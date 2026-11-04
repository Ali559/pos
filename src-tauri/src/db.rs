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
        Migration {
            version: 5,
            description: "money",
            sql: include_str!("../migrations/0005_money.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "single_language",
            sql: include_str!("../migrations/0006_single_language.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 7,
            description: "register",
            sql: include_str!("../migrations/0007_register.sql"),
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
    let lang = crate::settings::get(ctx, "language", "ku").await?;
    crate::seeds::insert_demo_catalog(pool, &lang).await?;
    seed_demo_history(ctx).await?;
    Ok(())
}

/// ~60 days of sales by three people, their shifts, and a few expenses, so
/// analytics has something to show in development. Deterministic.
async fn seed_demo_history(ctx: &crate::ctx::Ctx) -> Result<()> {
    use chrono::{Datelike, Duration, Local, TimeZone};
    let pool = &ctx.pool;
    let owner = ctx.session.get().map(|k| k.user_id).unwrap_or(1);

    // Two sample cashiers with unusable passwords (random, never shown)
    let mut staff = vec![owner];
    for (username, name) in [("demo.rebin", "Rebin Ahmed"), ("demo.shno", "Shno Karim")] {
        sqlx::query(
            "INSERT OR IGNORE INTO users (username, password_hash, full_name, role_id) VALUES ($1, $2, $3, 3)",
        )
        .bind(username)
        .bind(crate::auth::hash_secret(&uuid::Uuid::new_v4().to_string())?)
        .bind(name)
        .execute(pool)
        .await?;
        let (id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE username = $1")
            .bind(username)
            .fetch_one(pool)
            .await?;
        staff.push(id);
    }

    let products: Vec<(i64, String, f64, f64)> = sqlx::query_as(
        "SELECT id, name, CAST(selling_price AS REAL), CAST(cost_price AS REAL) FROM products ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    if products.is_empty() {
        return Ok(());
    }

    // Small LCG so the history is the same every time
    let mut seed: u64 = 42;
    let mut rand = move |n: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) % n
    };
    let fmt = |t: chrono::DateTime<chrono::Utc>| t.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let mut tx = pool.begin().await?;
    let today = Local::now().date_naive();
    for day in (1..=60).rev() {
        let date = today - Duration::days(day);
        let weekend = matches!(date.format("%a").to_string().as_str(), "Fri" | "Sat");
        for (i, &user) in staff.iter().enumerate() {
            // Each person works a fixed shift: 8–16, 12–20, 16–23
            let (start_h, end_h) = [(8, 16), (12, 20), (16, 23)][i % 3];
            let local = |h: u32, m: u32| Local.from_local_datetime(&date.and_hms_opt(h, m, 0).unwrap()).single().unwrap().to_utc();
            let shift = sqlx::query("INSERT INTO shifts (user_id, clocked_in_at, clocked_out_at) VALUES ($1, $2, $3)")
                .bind(user)
                .bind(fmt(local(start_h, 0)))
                .bind(fmt(local(end_h, 0)))
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();
            let count = 6 + rand(10) + if weekend { 6 } else { 0 };
            for _ in 0..count {
                let at = local(start_h + rand((end_h - start_h) as u64) as u32, rand(60) as u32);
                let mut subtotal = 0.0;
                let mut lines = Vec::new();
                for _ in 0..1 + rand(4) {
                    let (pid, name, price, cost) = &products[rand(products.len() as u64) as usize];
                    let qty = (1 + rand(3)) as f64;
                    subtotal += price * qty;
                    lines.push((*pid, name.clone(), qty, *price, *cost));
                }
                let sale = sqlx::query(
                    "INSERT INTO sales (receipt_number, sale_date, user_id, shift_id, subtotal, tax_amount, total_amount, created_at)
                     VALUES ($1, $2, $3, $4, $5, 0, $5, $2)",
                )
                .bind(format!("tmp-{}", uuid::Uuid::new_v4()))
                .bind(fmt(at))
                .bind(user)
                .bind(shift)
                .bind(subtotal)
                .execute(&mut *tx)
                .await?
                .last_insert_rowid();
                sqlx::query("UPDATE sales SET receipt_number = $1 WHERE id = $2")
                    .bind(format!("R-{sale:06}"))
                    .bind(sale)
                    .execute(&mut *tx)
                    .await?;
                for (pid, name, qty, price, cost) in lines {
                    sqlx::query(
                        "INSERT INTO sale_items (sale_id, product_id, product_name, quantity, unit_price, cost_price, subtotal, created_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                    )
                    .bind(sale)
                    .bind(pid)
                    .bind(name)
                    .bind(qty)
                    .bind(price)
                    .bind(cost)
                    .bind(price * qty)
                    .bind(fmt(at))
                    .execute(&mut *tx)
                    .await?;
                }
                let method = if rand(10) < 7 { "cash" } else { "card" };
                sqlx::query("INSERT INTO payments (sale_id, payment_method, amount, payment_date) VALUES ($1, $2, $3, $4)")
                    .bind(sale)
                    .bind(method)
                    .bind(subtotal)
                    .bind(fmt(at))
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }
    // The sale_items trigger took stock down; history shouldn't empty the
    // shelves, so give back what the sample sales took
    sqlx::query(
        "UPDATE products SET stock_quantity = stock_quantity
            + (SELECT COALESCE(SUM(quantity), 0) FROM sale_items si WHERE si.product_id = products.id)",
    )
    .execute(&mut *tx)
    .await?;

    let month = |n: u32| (today.with_day0(0).unwrap() - chrono::Months::new(n)).format("%Y-%m-%d").to_string();
    for (type_id, amount, on) in [
        (1, 750_000.0, month(1)), (1, 750_000.0, month(0)),
        (2, 120_000.0, month(1)), (2, 135_000.0, month(0)),
        (3, 90_000.0, month(1)), (3, 110_000.0, month(0)),
        (5, 45_000.0, (today - Duration::days(9)).format("%Y-%m-%d").to_string()),
    ] {
        sqlx::query("INSERT INTO expenses (type_id, amount, spent_on, created_by, created_at) VALUES ($1, $2, $3, $4, $5)")
            .bind(type_id)
            .bind(amount)
            .bind(on)
            .bind(owner)
            .bind(crate::ctx::now())
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    crate::notifications::refresh_quietly(ctx).await;
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
            crate::seeds::insert_demo_catalog(&pool, "ku").await.unwrap();

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
