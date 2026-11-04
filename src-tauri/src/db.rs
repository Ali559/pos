use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_sql::{DbInstances, DbPool, Migration, MigrationKind};

use crate::error::{Error, Result};

/// The one database the app uses. The front end loads the same URL, and
/// `tauri.conf.json` preloads it so migrations run at startup.
/// The file lives in the app config dir (plugin-sql's choice).
pub const DB_URL: &str = "sqlite:pos.db";

pub fn migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "init",
            sql: include_str!("../migrations/0001_init.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "shop_media_i18n",
            sql: include_str!("../migrations/0002_shop_media_i18n.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "people",
            sql: include_str!("../migrations/0003_people.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "catalog",
            sql: include_str!("../migrations/0004_catalog.sql"),
            kind: MigrationKind::Up,
        },
    ]
}

/// Same file as `DB_URL` (plugin-sql puts it in the app config dir).
pub fn db_path<R: Runtime>(app: &AppHandle<R>) -> Result<std::path::PathBuf> {
    Ok(app.path().app_config_dir()?.join("pos.db"))
}

/// A second pool on the same file, opened read-only, for WebView reads.
pub struct ReadPool(pub SqlitePool);

/// Called from Tauri's setup hook, which isn't inside the async runtime, so
/// the pool (which spawns background tasks) is built on that runtime. The
/// sql plugin has already created and migrated the file by then.
pub fn open_read_pool<R: Runtime>(app: &AppHandle<R>) -> Result<ReadPool> {
    let opts = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(db_path(app)?)
        .read_only(true);
    let pool = tauri::async_runtime::block_on(SqlitePool::connect_with(opts))?;
    Ok(ReadPool(pool))
}

/// The SQLite pool owned by tauri-plugin-sql, shared with Rust commands so
/// both sides see the same connection and migrations.
pub async fn pool<R: Runtime>(app: &AppHandle<R>) -> Result<SqlitePool> {
    let instances = app.state::<DbInstances>();
    let instances = instances.0.read().await;
    match instances.get(DB_URL) {
        Some(DbPool::Sqlite(pool)) => Ok(pool.clone()),
        _ => Err(Error::DbNotReady),
    }
}

/// Loads the sample catalog. Debug builds only; refuses if products exist.
pub async fn seed_demo_data(ctx: &crate::ctx::Ctx) -> Result<()> {
    if !cfg!(debug_assertions) {
        return Err(Error::Rule("dev_only"));
    }
    crate::auth::require(ctx, "settings.system").await?;
    let pool = &ctx.pool;
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM products")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Err(Error::Invalid("the catalog already has products".into()));
    }
    sqlx::raw_sql(include_str!("../seeds/demo.sql"))
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_and_seed_apply_in_order() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            for m in migrations() {
                sqlx::raw_sql(m.sql).execute(&pool).await.unwrap();
            }
            sqlx::raw_sql(include_str!("../seeds/demo.sql"))
                .execute(&pool)
                .await
                .unwrap();

            let (language,): (String,) =
                sqlx::query_as("SELECT value FROM settings WHERE key = 'language'")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(language, "ku");

            let (products,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM products")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(products, 15);
        });
    }
}
