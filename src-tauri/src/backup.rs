//! Backups: one `.zip` per backup holding a consistent snapshot of the
//! database (`VACUUM INTO`), every media file and a `backup.json` manifest.
//!
//! - The hourly background job takes one when the last good one is more
//!   than a day old; *Back up now* takes one on demand.
//! - Once a new backup is written *and* reads back cleanly, older regular
//!   backups beyond `backup_keep` (default 1: only the newest) are deleted.
//!   Safety copies taken before a restore (`pos-before-restore-…`) are
//!   never deleted this way.
//! - The folder is picked with a dialog opened here, so the WebView never
//!   names a path. Until one is picked, backups go next to the media folder.
//! - Restoring checks the file, takes a safety backup, stages the files and
//!   restarts; `apply_pending_restore` swaps them in at the next start,
//!   before the database is opened, and migrations bring an older backup up
//!   to date.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;

use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};

const PREFIX: &str = "pos-backup-";
const SAFETY_PREFIX: &str = "pos-before-restore-";
const MANIFEST: &str = "backup.json";
const DB_ENTRY: &str = "pos.db";
const PENDING_DIR: &str = "restore-pending";
const READY_MARK: &str = "READY";
/// Only the newest backup by default: once today's is done, older ones go
pub const DEFAULT_KEEP: u32 = 1;
/// A day between automatic backups
const AUTO_EVERY_SECS: i64 = 24 * 60 * 60;

/// One backup at a time (the background job and *Back up now* can collide)
static RUNNING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Auto,
    Manual,
    BeforeRestore,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Auto => "auto",
            Kind::Manual => "manual",
            Kind::BeforeRestore => "before_restore",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub app_version: String,
    /// Highest migration applied; None when the database wasn't migrated by
    /// the sql plugin (tests)
    pub schema_version: Option<i64>,
    pub shop_name: Option<String>,
    pub created_at: String,
    pub kind: String,
    pub sales: i64,
    pub products: i64,
    pub media_files: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub name: String,
    pub bytes: u64,
    /// From the manifest; None if the file couldn't be read
    pub manifest: Option<Manifest>,
}

// ------------------------------------------------------------ settings

async fn setting(ctx: &Ctx, key: &str) -> Result<Option<String>> {
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT value FROM settings WHERE key = $1")
        .bind(key)
        .fetch_optional(&ctx.pool)
        .await?;
    Ok(row.and_then(|(v,)| v).filter(|v| !v.is_empty()))
}

/// Settings Rust writes itself (status, folder); not open to `set_setting`.
async fn put(ctx: &Ctx, key: &str, value: Option<&str>) -> Result<()> {
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, $3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .bind(now())
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

/// `<app data>/backups`: the media folder's sibling.
pub fn default_dir(ctx: &Ctx) -> PathBuf {
    ctx.media_dir.parent().unwrap_or(&ctx.media_dir).join("backups")
}

pub async fn backup_dir(ctx: &Ctx) -> Result<PathBuf> {
    Ok(setting(ctx, "backup_dir").await?.map(PathBuf::from).unwrap_or_else(|| default_dir(ctx)))
}

async fn keep(ctx: &Ctx) -> Result<usize> {
    Ok(setting(ctx, "backup_keep").await?.and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_KEEP) as usize)
}

// ------------------------------------------------------------ creating

async fn manifest(ctx: &Ctx, kind: Kind) -> Result<Manifest> {
    let count = |table: &'static str| async move {
        let (n,): (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}")).fetch_one(&ctx.pool).await?;
        Ok::<i64, Error>(n)
    };
    let shop: Option<(String,)> = sqlx::query_as("SELECT name FROM shop_profile WHERE id = 1")
        .fetch_optional(&ctx.pool)
        .await?;
    Ok(Manifest {
        app_version: env!("CARGO_PKG_VERSION").into(),
        schema_version: schema_version(&ctx.pool).await?,
        shop_name: shop.map(|(n,)| n),
        created_at: now(),
        kind: kind.as_str().into(),
        sales: count("sales").await?,
        products: count("products").await?,
        media_files: count("media_files").await?,
    })
}

async fn schema_version(pool: &sqlx::SqlitePool) -> Result<Option<i64>> {
    let has: Option<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'")
            .fetch_optional(pool)
            .await?;
    if has.is_none() {
        return Ok(None);
    }
    let (v,): (Option<i64>,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
        .fetch_one(pool)
        .await?;
    Ok(v)
}

/// A name that doesn't exist yet in `dir`: `<prefix><local time>.zip`, with
/// `-2`, `-3`… if two land in the same second.
fn free_name(dir: &Path, prefix: &str) -> String {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let mut name = format!("{prefix}{stamp}.zip");
    let mut n = 2;
    while dir.join(&name).exists() {
        name = format!("{prefix}{stamp}-{n}.zip");
        n += 1;
    }
    name
}

/// `pos-backup-YYYYMMDD-HHMMSS[-N].zip`
fn is_regular(name: &str) -> bool {
    is_named(name, PREFIX)
}

fn is_backup(name: &str) -> bool {
    is_named(name, PREFIX) || is_named(name, SAFETY_PREFIX)
}

fn is_named(name: &str, prefix: &str) -> bool {
    let Some(rest) = name.strip_prefix(prefix).and_then(|r| r.strip_suffix(".zip")) else {
        return false;
    };
    let (stamp, suffix) = match rest.get(15..) {
        Some(s) => (&rest[..15], s),
        None => return false,
    };
    let b = stamp.as_bytes();
    b.len() == 15
        && b[8] == b'-'
        && b.iter().enumerate().all(|(i, c)| i == 8 || c.is_ascii_digit())
        && (suffix.is_empty() || suffix.strip_prefix('-').is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit())))
}

/// Writes the zip. The snapshot is taken into a temp file first, and the
/// zip is written under a hidden name and renamed when complete, so a
/// half-written backup never looks like a real one.
async fn write_backup(ctx: &Ctx, dir: &Path, kind: Kind) -> Result<BackupFile> {
    std::fs::create_dir_all(dir).map_err(|_| Error::Rule("backup_folder_unavailable"))?;
    let manifest = manifest(ctx, kind).await?;

    let snapshot = std::env::temp_dir().join(format!("pos-snapshot-{}.db", uuid::Uuid::new_v4()));
    sqlx::query("VACUUM INTO $1")
        .bind(snapshot.to_string_lossy().into_owned())
        .execute(&ctx.pool)
        .await?;

    let prefix = if kind == Kind::BeforeRestore { SAFETY_PREFIX } else { PREFIX };
    let name = free_name(dir, prefix);
    let partial = dir.join(format!(".{name}.partial"));
    let media_dir = ctx.media_dir.clone();
    let result = {
        let (snapshot, partial, manifest) = (snapshot.clone(), partial.clone(), manifest.clone());
        tauri::async_runtime::spawn_blocking(move || zip_up(&partial, &snapshot, &media_dir, &manifest))
            .await
            .map_err(|e| Error::Invalid(e.to_string()))?
    };
    let _ = std::fs::remove_file(&snapshot);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&partial);
        return Err(e);
    }
    let path = dir.join(&name);
    std::fs::rename(&partial, &path)?;
    Ok(BackupFile {
        bytes: std::fs::metadata(&path)?.len(),
        name,
        manifest: Some(manifest),
    })
}

fn zip_err(e: zip::result::ZipError) -> Error {
    match e {
        zip::result::ZipError::Io(e) => Error::Io(e),
        e => Error::Invalid(e.to_string()),
    }
}

fn zip_up(target: &Path, snapshot: &Path, media_dir: &Path, manifest: &Manifest) -> Result<()> {
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    let mut zip = zip::ZipWriter::new(File::create(target)?);
    let deflate = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).large_file(true);
    // WebP is already compressed
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file(MANIFEST, deflate).map_err(zip_err)?;
    zip.write_all(&serde_json::to_vec_pretty(manifest)?)?;

    zip.start_file(DB_ENTRY, deflate).map_err(zip_err)?;
    std::io::copy(&mut File::open(snapshot)?, &mut zip)?;

    if let Ok(entries) = std::fs::read_dir(media_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
            zip.start_file(format!("media/{name}"), stored).map_err(zip_err)?;
            std::io::copy(&mut File::open(&path)?, &mut zip)?;
        }
    }
    zip.finish().map_err(zip_err)?.sync_all()?;
    Ok(())
}

/// Deletes the oldest regular backups beyond `keep`. Names sort by time.
fn rotate(dir: &Path, keep: usize) -> Result<usize> {
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| is_regular(n))
        .collect();
    // Newest first: by time stamp, then by the -N of same-second backups
    let order = |n: &String| {
        let rest = &n[PREFIX.len()..n.len() - 4];
        (rest[..15].to_owned(), rest[15..].trim_start_matches('-').parse::<u32>().unwrap_or(1))
    };
    names.sort_by_key(|n| std::cmp::Reverse(order(n)));
    let mut removed = 0;
    for name in names.iter().skip(keep.max(1)) {
        if std::fs::remove_file(dir.join(name)).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Takes a backup and records the outcome in settings (for the status card
/// and the alerts). Errors are recorded and returned.
pub async fn run(ctx: &Ctx, kind: Kind) -> Result<BackupFile> {
    // A second request waits for the one in progress
    let _guard = RUNNING.lock().await;
    put(ctx, "backup_last_attempt_at", Some(&now())).await?;
    let dir = backup_dir(ctx).await?;
    // A chosen folder must already exist: creating it could quietly put the
    // backup on this computer's own disk when the USB stick is out
    let written = if setting(ctx, "backup_dir").await?.is_some() && !dir.is_dir() {
        Err(Error::Rule("backup_folder_unavailable"))
    } else {
        write_backup(ctx, &dir, kind).await
    };
    match written {
        Ok(file) => {
            put(ctx, "backup_last_ok_at", Some(&now())).await?;
            put(ctx, "backup_last_error", None).await?;
            // Old backups are only deleted once the new one is known to be good
            if kind != Kind::BeforeRestore {
                let check = std::env::temp_dir().join(format!("pos-backup-verify-{}", uuid::Uuid::new_v4()));
                let verified = unpack_and_check(&dir.join(&file.name), &check).await;
                let _ = std::fs::remove_dir_all(&check);
                match verified {
                    Ok(_) => {
                        if let Err(e) = rotate(&dir, keep(ctx).await?) {
                            eprintln!("[backup] removing old backups: {e}");
                        }
                    }
                    Err(e) => eprintln!("[backup] new backup failed its check, old ones kept: {e}"),
                }
            }
            Ok(file)
        }
        Err(e) => {
            // A rule's code (translated by the UI), else the message
            let text = match &e {
                Error::Rule(code) => (*code).to_owned(),
                e => e.to_string(),
            };
            put(ctx, "backup_last_error", Some(&text)).await?;
            Err(e)
        }
    }
}

/// Background job: a backup if automatic backups are on and the last good
/// one is more than a day old. Nothing before first-run setup.
pub async fn run_if_due(ctx: &Ctx) -> Result<()> {
    if auth::needs_setup(ctx).await? || setting(ctx, "backup_auto").await?.as_deref() == Some("0") {
        return Ok(());
    }
    let last = setting(ctx, "backup_last_ok_at").await?;
    let due = match last.as_deref().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()) {
        Some(t) => chrono::Utc::now().signed_duration_since(t).num_seconds() >= AUTO_EVERY_SECS,
        None => true,
    };
    if due {
        run(ctx, Kind::Auto).await?;
    }
    Ok(())
}

// ------------------------------------------------------------ reading

fn read_manifest(path: &Path) -> Result<Manifest> {
    let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(|_| Error::Rule("backup_invalid"))?;
    let mut text = String::new();
    zip.by_name(MANIFEST)
        .map_err(|_| Error::Rule("backup_invalid"))?
        .read_to_string(&mut text)?;
    serde_json::from_str(&text).map_err(|_| Error::Rule("backup_invalid"))
}

pub fn list_in(dir: &Path) -> Vec<BackupFile> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<BackupFile> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if !is_backup(&name) {
                return None;
            }
            Some(BackupFile {
                bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                manifest: read_manifest(&e.path()).ok(),
                name,
            })
        })
        .collect();
    // Newest first, whichever prefix
    out.sort_by(|a, b| {
        let at = |f: &BackupFile| f.manifest.as_ref().map(|m| m.created_at.clone()).unwrap_or_default();
        at(b).cmp(&at(a)).then_with(|| b.name.cmp(&a.name))
    });
    out
}

/// Entry names a backup may contain; anything else is refused, so a crafted
/// zip can't write outside the staging folder.
fn safe_entry(name: &str) -> Option<&str> {
    if name == DB_ENTRY || name == MANIFEST {
        return Some(name);
    }
    let file = name.strip_prefix("media/")?;
    let ok = !file.is_empty()
        && !file.starts_with('.')
        && file.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    ok.then_some(name)
}

/// Unpacks a backup into `into` (created fresh) and checks it: a manifest,
/// a database that passes `quick_check`, has the app's tables, and isn't
/// from a newer version of the app. Returns the manifest.
async fn unpack_and_check(path: &Path, into: &Path) -> Result<Manifest> {
    let manifest = read_manifest(path)?;
    {
        let (path, into) = (path.to_owned(), into.to_owned());
        tauri::async_runtime::spawn_blocking(move || unpack(&path, &into))
            .await
            .map_err(|e| Error::Invalid(e.to_string()))??;
    }
    let db = into.join(DB_ENTRY);
    if !db.is_file() {
        return Err(Error::Rule("backup_invalid"));
    }
    let pool = sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&db).read_only(true))
        .await
        .map_err(|_| Error::Rule("backup_invalid"))?;
    let checked = async {
        let (ok,): (String,) = sqlx::query_as("PRAGMA quick_check").fetch_one(&pool).await?;
        if ok != "ok" {
            return Err(Error::Rule("backup_invalid"));
        }
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('shop_profile', 'products', 'sales', 'users')",
        )
        .fetch_all(&pool)
        .await?;
        if tables.len() != 4 {
            return Err(Error::Rule("backup_invalid"));
        }
        if schema_version(&pool).await?.unwrap_or(0) > crate::db::migrations().len() as i64 {
            return Err(Error::Rule("backup_too_new"));
        }
        Ok(())
    }
    .await;
    pool.close().await;
    checked?;
    Ok(manifest)
}

fn unpack(path: &Path, into: &Path) -> Result<()> {
    if into.exists() {
        std::fs::remove_dir_all(into)?;
    }
    std::fs::create_dir_all(into.join("media"))?;
    let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(|_| Error::Rule("backup_invalid"))?;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|_| Error::Rule("backup_invalid"))?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().map_err(|_| Error::Rule("backup_invalid"))?.into_owned();
        let Some(name) = safe_entry(&name) else {
            return Err(Error::Rule("backup_invalid"));
        };
        let mut out = File::create(into.join(name))?;
        std::io::copy(&mut file, &mut out)?;
    }
    Ok(())
}

/// Moves a file or folder, copying when a rename can't cross disks.
fn move_path(from: &Path, to: &Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)?.flatten() {
            move_path(&entry.path(), &to.join(entry.file_name()))?;
        }
        std::fs::remove_dir_all(from)
    } else {
        std::fs::copy(from, to)?;
        std::fs::remove_file(from)
    }
}

/// Swaps a staged restore in. Called at startup before the sql plugin opens
/// the database. `db` is the live database file, `media` the media folder.
pub fn apply_pending(staging: &Path, db: &Path, media: &Path) -> std::io::Result<bool> {
    if !staging.join(READY_MARK).is_file() {
        // A half-staged restore (crash while unpacking) is thrown away
        if staging.exists() {
            std::fs::remove_dir_all(staging)?;
        }
        return Ok(false);
    }
    // The old write-ahead log belongs to the old database: it must go too
    for ext in ["-wal", "-shm", "-journal"] {
        let side = PathBuf::from(format!("{}{ext}", db.display()));
        if side.exists() {
            std::fs::remove_file(side)?;
        }
    }
    if let Some(parent) = db.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(db);
    move_path(&staging.join(DB_ENTRY), db)?;

    let old_media = media.with_file_name(format!("media.old-{}", uuid::Uuid::new_v4()));
    if media.exists() {
        std::fs::rename(media, &old_media)?;
    }
    move_path(&staging.join("media"), media)?;
    let _ = std::fs::remove_dir_all(&old_media);
    std::fs::remove_dir_all(staging)?;
    Ok(true)
}

fn staging_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(PENDING_DIR))
}

/// A tiny plugin registered before the sql plugin, so a staged restore is in
/// place before the database is opened and migrated.
pub fn restore_plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("pos-restore")
        .setup(|app, _| {
            let staging = staging_dir(app)?;
            let db = crate::db::db_path(app)?;
            let media = app.path().app_data_dir()?.join("media");
            match apply_pending(&staging, &db, &media) {
                Ok(true) => eprintln!("[backup] restored a backup"),
                Ok(false) => {}
                // Leave the current data alone; the staged copy stays for a retry
                Err(e) => eprintln!("[backup] restore failed: {e}"),
            }
            Ok(())
        })
        .build()
}

// ------------------------------------------------------------ commands

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    pub dir: String,
    pub is_default_dir: bool,
    /// False when the chosen folder is missing (e.g. the USB stick is out)
    pub dir_available: bool,
    pub auto: bool,
    pub keep: u32,
    pub last_ok_at: Option<String>,
    pub last_attempt_at: Option<String>,
    pub last_error: Option<String>,
    pub files: Vec<BackupFile>,
}

pub async fn backup_status(ctx: &Ctx) -> Result<BackupStatus> {
    auth::require(ctx, "settings.system").await?;
    let custom = setting(ctx, "backup_dir").await?;
    let dir = backup_dir(ctx).await?;
    Ok(BackupStatus {
        dir_available: custom.is_none() || dir.is_dir(),
        is_default_dir: custom.is_none(),
        dir: dir.to_string_lossy().into_owned(),
        auto: setting(ctx, "backup_auto").await?.as_deref() != Some("0"),
        keep: keep(ctx).await? as u32,
        last_ok_at: setting(ctx, "backup_last_ok_at").await?,
        last_attempt_at: setting(ctx, "backup_last_attempt_at").await?,
        last_error: setting(ctx, "backup_last_error").await?,
        files: list_in(&dir),
    })
}

pub async fn backup_now(ctx: &Ctx) -> Result<BackupFile> {
    let actor = auth::require(ctx, "settings.system").await?;
    let result = run(ctx, Kind::Manual).await;
    crate::notifications::refresh_quietly(ctx).await;
    let file = result?;
    crate::audit::log(ctx, Some(actor.user_id), "backup", "", None, &serde_json::json!({ "file": file.name })).await?;
    Ok(file)
}

#[derive(Debug, Deserialize)]
pub struct FolderArgs {
    /// Back to the default folder instead of picking one
    #[serde(default)]
    pub reset: bool,
}

/// Opens a folder dialog and stores the choice. Returns false if cancelled.
#[tauri::command]
pub async fn choose_backup_folder(app: AppHandle, args: FolderArgs) -> Result<bool> {
    // Files on this computer feed the main computer's database: do it there
    if crate::remote::client(&app).is_some() {
        return Err(crate::error::Error::Rule("main_only"));
    }
    let ctx = Ctx::from_app(&app).await?;
    let actor = auth::require(&ctx, "settings.system").await?;
    let dir = if args.reset {
        None
    } else {
        let Some(path) = app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok()) else {
            return Ok(false);
        };
        // Must be writable now, not at 3 a.m. when the first backup runs
        let probe = path.join(format!(".pos-write-test-{}", uuid::Uuid::new_v4()));
        std::fs::write(&probe, b"").map_err(|_| Error::Rule("backup_folder_unavailable"))?;
        let _ = std::fs::remove_file(probe);
        Some(path.to_string_lossy().into_owned())
    };
    put(&ctx, "backup_dir", dir.as_deref()).await?;
    crate::audit::log(&ctx, Some(actor.user_id), "backup.folder", "settings", None, &serde_json::json!({ "dir": dir })).await?;
    Ok(true)
}

/// A checked backup waiting for the user to confirm the restore.
#[derive(Default)]
pub struct RestoreCandidate(Mutex<Option<(String, PathBuf)>>);

#[derive(Debug, Deserialize)]
pub struct InspectArgs {
    /// A file in the backup folder; None opens a file dialog
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspected {
    /// Pass to `restore_backup` to go ahead
    pub token: String,
    pub name: String,
    pub manifest: Manifest,
}

/// Checks a backup and remembers it for `restore_backup`. None if the file
/// dialog was cancelled.
#[tauri::command]
pub async fn inspect_backup(app: AppHandle, args: InspectArgs) -> Result<Option<Inspected>> {
    // Files on this computer feed the main computer's database: do it there
    if crate::remote::client(&app).is_some() {
        return Err(crate::error::Error::Rule("main_only"));
    }
    let ctx = Ctx::from_app(&app).await?;
    auth::require(&ctx, "settings.system").await?;
    let path = match args.name {
        Some(name) => {
            if !is_backup(&name) {
                return Err(Error::NotFound);
            }
            backup_dir(&ctx).await?.join(name)
        }
        None => {
            let picked = app.dialog().file().add_filter("Backup", &["zip"]).blocking_pick_file();
            match picked.and_then(|p| p.into_path().ok()) {
                Some(p) => p,
                None => return Ok(None),
            }
        }
    };
    if !path.is_file() {
        return Err(Error::NotFound);
    }
    let check = std::env::temp_dir().join(format!("pos-restore-check-{}", uuid::Uuid::new_v4()));
    let manifest = unpack_and_check(&path, &check).await;
    let _ = std::fs::remove_dir_all(&check);
    let manifest = manifest?;
    let token = uuid::Uuid::new_v4().to_string();
    *app.state::<RestoreCandidate>().0.lock().unwrap() = Some((token.clone(), path.clone()));
    Ok(Some(Inspected {
        token,
        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        manifest,
    }))
}

#[derive(Debug, Deserialize)]
pub struct RestoreArgs {
    pub token: String,
}

/// Takes a safety backup, stages the chosen backup and restarts the app.
#[tauri::command]
pub async fn restore_backup(app: AppHandle, args: RestoreArgs) -> Result<()> {
    // Files on this computer feed the main computer's database: do it there
    if crate::remote::client(&app).is_some() {
        return Err(crate::error::Error::Rule("main_only"));
    }
    let ctx = Ctx::from_app(&app).await?;
    let actor = auth::require(&ctx, "settings.system").await?;
    let path = {
        let state = app.state::<RestoreCandidate>();
        let mut slot = state.0.lock().unwrap();
        match slot.take() {
            Some((token, path)) if token == args.token => path,
            other => {
                *slot = other;
                return Err(Error::Rule("backup_not_checked"));
            }
        }
    };
    let staging = staging_dir(&app)?;
    let manifest = unpack_and_check(&path, &staging).await?;
    if let Err(e) = run(&ctx, Kind::BeforeRestore).await {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    crate::audit::log(
        &ctx,
        Some(actor.user_id),
        "backup.restore",
        "",
        None,
        &serde_json::json!({ "file": path.file_name().map(|n| n.to_string_lossy().into_owned()), "created_at": manifest.created_at }),
    )
    .await?;
    std::fs::write(staging.join(READY_MARK), b"")?;
    ctx.pool.close().await;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::ctx::test_support;

    #[test]
    fn names() {
        assert!(is_regular("pos-backup-20261106-143000.zip"));
        assert!(is_regular("pos-backup-20261106-143000-2.zip"));
        assert!(!is_regular("pos-before-restore-20261106-143000.zip"));
        assert!(is_backup("pos-before-restore-20261106-143000.zip"));
        for bad in ["pos-backup-2026116-143000.zip", "pos-backup-20261106-143000-.zip", "pos-backup-20261106-143000.zip.bak", "../pos-backup-20261106-143000.zip", "notes.zip"] {
            assert!(!is_backup(bad), "{bad}");
        }
        assert_eq!(safe_entry("media/ab-12.webp"), Some("media/ab-12.webp"));
        for bad in ["media/../pos.db", "../pos.db", "media/a/b.webp", "media/.hidden", "other.txt", "/pos.db"] {
            assert_eq!(safe_entry(bad), None, "{bad}");
        }
    }

    #[test]
    fn rotation_keeps_the_newest_regular_backups() {
        let dir = std::env::temp_dir().join(format!("pos-rot-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        for d in 1..=5 {
            std::fs::write(dir.join(format!("pos-backup-2026110{d}-120000.zip")), b"").unwrap();
        }
        std::fs::write(dir.join("pos-before-restore-20261101-120000.zip"), b"").unwrap();
        std::fs::write(dir.join("holiday.zip"), b"").unwrap();
        // Same second: the -2 one is the newer
        std::fs::write(dir.join("pos-backup-20261105-120000-2.zip"), b"").unwrap();
        assert_eq!(rotate(&dir, 2).unwrap(), 4);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
        left.sort();
        assert_eq!(left, ["holiday.zip", "pos-backup-20261105-120000-2.zip", "pos-backup-20261105-120000.zip", "pos-before-restore-20261101-120000.zip"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn backup_round_trips_and_restores() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx_with_read_only_pool().await;
            setup_owner(&ctx).await;
            let media = ctx.media_dir.join("media");
            std::fs::create_dir_all(&media).unwrap();
            let ctx = Ctx { media_dir: media.clone(), ..ctx };
            std::fs::write(media.join("logo.webp"), b"RIFF-logo").unwrap();

            let file = run(&ctx, Kind::Manual).await.unwrap();
            let dir = backup_dir(&ctx).await.unwrap();
            assert_eq!(dir, default_dir(&ctx));
            assert!(is_regular(&file.name));
            let m = file.manifest.as_ref().unwrap();
            assert_eq!(m.shop_name.as_deref(), Some("دووکان"));

            // Listed, with no partial files left behind
            let listed = list_in(&dir);
            assert_eq!(listed.len(), 1);
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);

            // Data changes after the backup…
            sqlx::query("UPDATE shop_profile SET name = 'Changed'").execute(&ctx.pool).await.unwrap();
            std::fs::write(media.join("new.webp"), b"RIFF-new").unwrap();

            // …a restore brings back the backup's database and media
            let staging = ctx.media_dir.parent().unwrap().join(PENDING_DIR);
            unpack_and_check(&dir.join(&file.name), &staging).await.unwrap();
            std::fs::write(staging.join(READY_MARK), b"").unwrap();
            let live = ctx.media_dir.parent().unwrap().join("restored.db");
            std::fs::write(&live, b"old").unwrap();
            std::fs::write(format!("{}-wal", live.display()), b"old wal").unwrap();
            assert!(apply_pending(&staging, &live, &media).unwrap());
            assert!(!staging.exists());
            assert!(!PathBuf::from(format!("{}-wal", live.display())).exists());
            assert!(media.join("logo.webp").is_file());
            assert!(!media.join("new.webp").exists());
            let pool = sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&live)).await.unwrap();
            let (name,): (String,) = sqlx::query_as("SELECT name FROM shop_profile").fetch_one(&pool).await.unwrap();
            assert_eq!(name, "دووکان");

            // Nothing staged → nothing happens
            assert!(!apply_pending(&staging, &live, &media).unwrap());
        });
    }

    #[test]
    fn refuses_files_that_are_not_backups() {
        tauri::async_runtime::block_on(async {
            let dir = std::env::temp_dir().join(format!("pos-bad-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let junk = dir.join("junk.zip");
            std::fs::write(&junk, b"not a zip").unwrap();
            let r = unpack_and_check(&junk, &dir.join("out")).await;
            assert!(matches!(r, Err(Error::Rule("backup_invalid"))));

            // A zip with a manifest but an entry trying to escape
            let evil = dir.join("evil.zip");
            let mut z = zip::ZipWriter::new(File::create(&evil).unwrap());
            z.start_file(MANIFEST, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(serde_json::to_string(&Manifest::default()).unwrap().as_bytes()).unwrap();
            z.start_file("../escape.txt", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
            let r = unpack_and_check(&evil, &dir.join("out2")).await;
            assert!(matches!(r, Err(Error::Rule("backup_invalid"))));
            assert!(!dir.join("escape.txt").exists());
            std::fs::remove_dir_all(dir).unwrap();
        });
    }

    #[test]
    fn due_only_when_set_up_and_a_day_old() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx_with_read_only_pool().await;
            // Backups go next to the media folder: keep them in this test's own dir
            let ctx = Ctx { media_dir: ctx.media_dir.join("media"), ..ctx };
            run_if_due(&ctx).await.unwrap();
            assert!(setting(&ctx, "backup_last_ok_at").await.unwrap().is_none(), "no backup before setup");

            setup_owner(&ctx).await;
            run_if_due(&ctx).await.unwrap();
            let first = setting(&ctx, "backup_last_ok_at").await.unwrap().unwrap();
            run_if_due(&ctx).await.unwrap();
            assert_eq!(list_in(&backup_dir(&ctx).await.unwrap()).len(), 1, "not due again within a day");

            let before = list_in(&backup_dir(&ctx).await.unwrap())[0].name.clone();
            put(&ctx, "backup_last_ok_at", Some("2020-01-01T00:00:00Z")).await.unwrap();
            run_if_due(&ctx).await.unwrap();
            assert_ne!(setting(&ctx, "backup_last_ok_at").await.unwrap().unwrap(), "2020-01-01T00:00:00Z");
            let _ = first;
            // Once the new one is done and checked, the old one is gone
            let after = list_in(&backup_dir(&ctx).await.unwrap());
            assert_eq!(after.len(), 1);
            assert_ne!(after[0].name, before);
        });
    }
}
