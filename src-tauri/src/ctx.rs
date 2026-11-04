use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, Runtime};

use crate::db;
use crate::error::Result;
use crate::media;

/// Who is signed in on this till. One session per app instance.
#[derive(Debug, Clone, Copy)]
pub struct SessionKey {
    pub user_id: i64,
}

/// A manager's PIN approval for one action the cashier can't do alone.
/// Single use and short-lived; see `auth::authorize_override`.
#[derive(Debug, Clone)]
pub struct Override {
    pub token: String,
    pub permission: String,
    pub approver_id: i64,
    pub expires: std::time::Instant,
}

#[derive(Default)]
pub struct SessionState {
    key: Mutex<Option<SessionKey>>,
    overrides: Mutex<Vec<Override>>,
}

impl SessionState {
    pub fn get(&self) -> Option<SessionKey> {
        *self.key.lock().unwrap()
    }
    /// Changing who is signed in drops any pending approvals
    pub fn set(&self, key: Option<SessionKey>) {
        *self.key.lock().unwrap() = key;
        self.overrides.lock().unwrap().clear();
    }
    pub fn add_override(&self, o: Override) {
        let mut list = self.overrides.lock().unwrap();
        list.retain(|x| x.expires > std::time::Instant::now());
        list.push(o);
    }
    /// Removes and returns the approver if the token is valid for `permission`.
    pub fn take_override(&self, token: &str, permission: &str) -> Option<i64> {
        let mut list = self.overrides.lock().unwrap();
        let now = std::time::Instant::now();
        let i = list.iter().position(|o| o.token == token && o.permission == permission && o.expires > now)?;
        Some(list.remove(i).approver_id)
    }
}

/// Shows a system notification (title, body). Absent in tests.
pub type Notifier = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Everything a command needs, without Tauri types, so command logic can be
/// unit tested against an in-memory database and reused by the dev bridge.
pub struct Ctx {
    /// Read-write pool shared with tauri-plugin-sql (migrations live there)
    pub pool: SqlitePool,
    /// Read-only connection for the `select` command
    pub read_pool: SqlitePool,
    pub session: Arc<SessionState>,
    pub media_dir: PathBuf,
    pub notifier: Option<Notifier>,
}

impl Ctx {
    pub async fn from_app<R: Runtime>(app: &AppHandle<R>) -> Result<Self> {
        Ok(Self {
            pool: db::pool(app).await?,
            read_pool: app.state::<db::ReadPool>().0.clone(),
            session: app.state::<Arc<SessionState>>().inner().clone(),
            media_dir: media::media_dir(app)?,
            notifier: Some(os_notifier(app.clone())),
        })
    }
}

fn os_notifier<R: Runtime>(app: AppHandle<R>) -> Notifier {
    use tauri_plugin_notification::NotificationExt;
    Arc::new(move |title: &str, body: &str| {
        if let Err(e) = app.notification().builder().title(title).body(body).show() {
            eprintln!("[notify] {e}");
        }
    })
}

/// Current time as an RFC 3339 UTC string ("2026-11-04T15:00:00Z"), the
/// format used for every time column written from Rust.
pub fn now() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    /// A context over a fresh in-memory database with every migration applied.
    pub async fn ctx() -> Ctx {
        // One connection: each in-memory connection is its own database
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for m in db::migrations() {
            sqlx::raw_sql(m.sql).execute(&pool).await.unwrap();
        }
        let media_dir = std::env::temp_dir().join(format!("pos-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&media_dir).unwrap();
        Ctx {
            read_pool: pool.clone(),
            pool,
            session: Arc::new(SessionState::default()),
            media_dir,
            notifier: None,
        }
    }

    /// Like `ctx()`, but on a temp file so a genuinely read-only second
    /// connection can be opened on it.
    pub async fn ctx_with_read_only_pool() -> Ctx {
        use sqlx::sqlite::SqliteConnectOptions;
        let dir = std::env::temp_dir().join(format!("pos-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("pos.db");
        let pool = SqlitePool::connect_with(SqliteConnectOptions::new().filename(&file).create_if_missing(true))
            .await
            .unwrap();
        for m in db::migrations() {
            sqlx::raw_sql(m.sql).execute(&pool).await.unwrap();
        }
        let read_pool = SqlitePool::connect_with(SqliteConnectOptions::new().filename(&file).read_only(true))
            .await
            .unwrap();
        Ctx {
            pool,
            read_pool,
            session: Arc::new(SessionState::default()),
            media_dir: dir,
            notifier: None,
        }
    }
}
