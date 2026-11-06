//! In-app alerts: low/out of stock, expiring/expired batches, recurring
//! expenses waiting to be confirmed, backups failing or overdue.
//!
//! `refresh` recomputes every condition from the data and reconciles the
//! table: a new condition opens a row, a condition that no longer holds
//! closes its row (`resolved_at`), and open rows get their numbers updated.
//! It runs after every stock change, at startup and hourly, so nothing
//! depends on triggers firing in the right order.

use std::collections::{HashMap, HashSet};

use chrono::{Duration, Local};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::Result;

const BACKUP_OVERDUE_DAYS: i64 = 3;

type Key = (String, Option<i64>, Option<i64>, Option<i64>); // kind, product, batch, expense

struct Condition {
    key: Key,
    payload: Value,
}

async fn setting(ctx: &Ctx, key: &str, default: &str) -> Result<String> {
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT value FROM settings WHERE key = $1")
        .bind(key)
        .fetch_optional(&ctx.pool)
        .await?;
    Ok(row.and_then(|(v,)| v).unwrap_or_else(|| default.to_owned()))
}

async fn conditions(ctx: &Ctx) -> Result<Vec<Condition>> {
    let mut out = Vec::new();

    let products: Vec<(i64, String, f64, f64)> = sqlx::query_as(
        "SELECT id, name, CAST(stock_quantity AS REAL), CAST(min_stock_level AS REAL)
         FROM products WHERE is_active = 1 AND track_stock = 1 AND (stock_quantity <= 0 OR stock_quantity <= min_stock_level)",
    )
    .fetch_all(&ctx.pool)
    .await?;
    for (id, name, stock, min) in products {
        let kind = if stock <= 0.0 { "stock_out" } else { "stock_low" };
        out.push(Condition {
            key: (kind.into(), Some(id), None, None),
            payload: json!({ "name": name, "stock": stock, "min": min }),
        });
    }

    let warn_days: i64 = setting(ctx, "expiry_warn_days", "7").await?.parse().unwrap_or(7);
    let today = Local::now().date_naive();
    let horizon = (today + Duration::days(warn_days)).format("%Y-%m-%d").to_string();
    let today_s = today.format("%Y-%m-%d").to_string();
    let batches: Vec<(i64, i64, String, String, f64)> = sqlx::query_as(
        "SELECT b.id, b.product_id, p.name, b.expiry_date, b.quantity_remaining
         FROM product_batches b JOIN products p ON p.id = b.product_id
         WHERE p.track_stock = 1 AND b.quantity_remaining > 0 AND b.expiry_date IS NOT NULL AND b.expiry_date <= $1",
    )
    .bind(&horizon)
    .fetch_all(&ctx.pool)
    .await?;
    for (batch, product, name, expiry, qty) in batches {
        // Sellable through its expiry day; expired from the day after
        let kind = if expiry < today_s { "expired" } else { "expiring" };
        out.push(Condition {
            key: (kind.into(), Some(product), Some(batch), None),
            payload: json!({ "name": name, "expiry": expiry, "quantity": qty }),
        });
    }

    let drafts: Vec<(i64, String, f64, String)> = sqlx::query_as(
        "SELECT e.id, t.name, e.amount, e.spent_on
         FROM expenses e JOIN expense_types t ON t.id = e.type_id WHERE e.status = 'draft'",
    )
    .fetch_all(&ctx.pool)
    .await?;
    for (id, name, amount, on) in drafts {
        out.push(Condition {
            key: ("expense_due".into(), None, None, Some(id)),
            payload: json!({ "name": name, "amount": amount, "date": on }),
        });
    }

    // Backups: the last attempt failed, or there's been no good one for a
    // while (automatic backups off, or failing every day)
    let last_error = setting(ctx, "backup_last_error", "").await?;
    let last_ok = setting(ctx, "backup_last_ok_at", "").await?;
    if !last_error.is_empty() {
        out.push(Condition {
            key: ("backup_failed".into(), None, None, None),
            payload: json!({ "error": last_error, "lastOk": last_ok }),
        });
    }
    let overdue = match chrono::DateTime::parse_from_rfc3339(&last_ok) {
        Ok(t) => chrono::Utc::now().signed_duration_since(t) > Duration::days(BACKUP_OVERDUE_DAYS),
        // Never backed up: only worth an alert if nothing will do it soon
        Err(_) => setting(ctx, "backup_auto", "1").await? == "0" && !crate::auth::needs_setup(ctx).await?,
    };
    if overdue {
        out.push(Condition {
            key: ("backup_overdue".into(), None, None, None),
            payload: json!({ "lastOk": last_ok, "days": BACKUP_OVERDUE_DAYS }),
        });
    }
    Ok(out)
}

/// Reconciles the table with current conditions. Returns the newly opened
/// rows as (kind, payload).
pub async fn refresh(ctx: &Ctx) -> Result<Vec<(String, Value)>> {
    let wanted = conditions(ctx).await?;
    let open: Vec<(i64, String, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT id, kind, product_id, batch_id, expense_id FROM notifications WHERE resolved_at IS NULL",
    )
    .fetch_all(&ctx.pool)
    .await?;
    let open: HashMap<Key, i64> = open
        .into_iter()
        .map(|(id, kind, p, b, e)| ((kind, p, b, e), id))
        .collect();
    let wanted_keys: HashSet<&Key> = wanted.iter().map(|c| &c.key).collect();
    let at = now();

    let mut tx = ctx.pool.begin().await?;
    for (key, id) in &open {
        if !wanted_keys.contains(key) {
            sqlx::query("UPDATE notifications SET resolved_at = $1 WHERE id = $2")
                .bind(&at)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
    }
    let mut created = Vec::new();
    for c in &wanted {
        let (kind, product, batch, expense) = &c.key;
        match open.get(&c.key) {
            Some(id) => {
                sqlx::query("UPDATE notifications SET payload = $1 WHERE id = $2")
                    .bind(c.payload.to_string())
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
            None => {
                sqlx::query(
                    "INSERT INTO notifications (kind, product_id, batch_id, expense_id, payload, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(kind)
                .bind(product)
                .bind(batch)
                .bind(expense)
                .bind(c.payload.to_string())
                .bind(&at)
                .execute(&mut *tx)
                .await?;
                created.push((kind.clone(), c.payload.clone()));
            }
        }
    }
    tx.commit().await?;

    if !created.is_empty() {
        notify_os(ctx, &created).await?;
    }
    Ok(created)
}

/// Refresh after a write; an alert problem must never fail the sale or
/// stock change that triggered it.
pub async fn refresh_quietly(ctx: &Ctx) {
    if let Err(e) = refresh(ctx).await {
        eprintln!("[notifications] refresh failed: {e}");
    }
}

/// System notifications for the urgent kinds, in the UI language.
async fn notify_os(ctx: &Ctx, created: &[(String, Value)]) -> Result<()> {
    let Some(notify) = &ctx.notifier else { return Ok(()) };
    if setting(ctx, "os_notifications", "1").await? != "1" {
        return Ok(());
    }
    let lang = setting(ctx, "language", "ku").await?;
    for (kind, payload) in created {
        let name = payload["name"].as_str().unwrap_or_default();
        // The error is a code or an English message; point to Settings instead
        let name = if kind == "backup_failed" {
            match lang.as_str() {
                "ku" => "ڕێکخستنەکان ← باکئەپ ببینە",
                "ar" => "راجع الإعدادات ← النسخ الاحتياطي",
                _ => "See Settings → Backups",
            }
        } else {
            name
        };
        let title = match (kind.as_str(), lang.as_str()) {
            ("stock_out", "ku") => "کاڵا نەماوە",
            ("stock_out", "ar") => "نفد المخزون",
            ("stock_out", _) => "Out of stock",
            ("expired", "ku") => "کاڵا بەسەرچووە",
            ("expired", "ar") => "منتج منتهي الصلاحية",
            ("expired", _) => "Expired stock",
            ("backup_failed", "ku") => "باکئەپ سەرکەوتوو نەبوو",
            ("backup_failed", "ar") => "فشل النسخ الاحتياطي",
            ("backup_failed", _) => "Backup failed",
            _ => continue,
        };
        notify(title, name);
    }
    Ok(())
}

// ------------------------------------------------------------ commands

/// Anyone signed in may ask for a recount (e.g. when opening the panel).
pub async fn refresh_notifications(ctx: &Ctx) -> Result<()> {
    auth::current(ctx).await?;
    refresh(ctx).await.map(|_| ())
}

#[derive(Debug, Deserialize)]
pub struct MarkReadArgs {
    /// None marks every open notification read
    pub ids: Option<Vec<i64>>,
}

pub async fn mark_notifications_read(ctx: &Ctx, args: MarkReadArgs) -> Result<()> {
    auth::current(ctx).await?;
    match args.ids {
        None => {
            sqlx::query("UPDATE notifications SET read_at = $1 WHERE read_at IS NULL AND resolved_at IS NULL")
                .bind(now())
                .execute(&ctx.pool)
                .await?;
        }
        Some(ids) => {
            for id in ids {
                sqlx::query("UPDATE notifications SET read_at = COALESCE(read_at, $1) WHERE id = $2")
                    .bind(now())
                    .bind(id)
                    .execute(&ctx.pool)
                    .await?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct IdArgs {
    pub id: i64,
}

/// Hides an alert until its condition clears and comes back.
pub async fn dismiss_notification(ctx: &Ctx, args: IdArgs) -> Result<()> {
    auth::current(ctx).await?;
    sqlx::query("UPDATE notifications SET dismissed_at = $1, read_at = COALESCE(read_at, $1) WHERE id = $2")
        .bind(now())
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::catalog::create_product;
    use crate::ctx::test_support;
    use crate::sales::{checkout, tests::sell as sale_of};
    use crate::stock::{receive_stock, ReceiveArgs};

    async fn open(ctx: &Ctx) -> Vec<(String, Option<i64>)> {
        sqlx::query_as("SELECT kind, product_id FROM notifications WHERE resolved_at IS NULL ORDER BY kind")
            .fetch_all(&ctx.pool)
            .await
            .unwrap()
    }

    #[test]
    fn stock_alerts_follow_the_count() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let mut p = crate::sales::tests::product("Cola", "1", 3.0);
            p.min_stock = 2.0;
            let id = create_product(&ctx, p).await.unwrap();
            refresh(&ctx).await.unwrap();
            assert!(open(&ctx).await.is_empty());

            let sell = |q: f64| checkout(&ctx, sale_of(id, q, "cash"));
            sell(1.0).await.unwrap();
            assert_eq!(open(&ctx).await, vec![("stock_low".into(), Some(id))]);
            sell(2.0).await.unwrap();
            assert_eq!(open(&ctx).await, vec![("stock_out".into(), Some(id))], "low resolves when it becomes out");

            receive_stock(&ctx, ReceiveArgs { product_id: id, quantity: 10.0, expiry_date: None, cost_price: None, note: None })
                .await
                .unwrap();
            assert!(open(&ctx).await.is_empty(), "restocking resolves it");
            let (history,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE resolved_at IS NOT NULL")
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            assert_eq!(history, 2);
        });
    }

    #[test]
    fn expiry_alerts_for_remaining_batches_only() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let mut p = crate::sales::tests::product("Milk", "1", 0.0);
            p.min_stock = 0.0;
            let id = create_product(&ctx, p).await.unwrap();
            let day = |n: i64| (Local::now().date_naive() + Duration::days(n)).format("%Y-%m-%d").to_string();
            for (qty, d) in [(2.0, day(-1)), (3.0, day(3)), (4.0, day(30))] {
                receive_stock(&ctx, ReceiveArgs { product_id: id, quantity: qty, expiry_date: Some(d), cost_price: None, note: None })
                    .await
                    .unwrap();
            }
            let kinds: Vec<String> = open(&ctx).await.into_iter().map(|(k, _)| k).collect();
            assert_eq!(kinds, vec!["expired", "expiring"], "the 30-day batch is outside the 7-day window");

            // Selling 2 uses up the expired batch first, so its alert clears
            checkout(&ctx, sale_of(id, 2.0, "cash"))
                .await
                .unwrap();
            let kinds: Vec<String> = open(&ctx).await.into_iter().map(|(k, _)| k).collect();
            assert_eq!(kinds, vec!["expiring"]);
        });
    }
}
