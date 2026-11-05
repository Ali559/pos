//! Stock movements outside of sales: receiving batches (with optional
//! expiry) and removing stock (damaged, expired, count corrections).
//!
//! `products.stock_quantity` is the count; `product_batches` tracks what's
//! left of each delivery so expiry alerts point at stock that still exists.
//! Anything that takes stock out consumes batches soonest-expiry first.

use chrono::NaiveDate;
use serde::Deserialize;
use sqlx::{Sqlite, Transaction};

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::notifications;

/// Today on the till's clock, as YYYY-MM-DD (expiry dates are local).
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// How much of the product sits in batches that expired before `today`.
pub async fn expired_quantity(tx: &mut Transaction<'_, Sqlite>, product_id: i64, today: &str) -> Result<f64> {
    let (n,): (f64,) = sqlx::query_as(
        "SELECT CAST(COALESCE(SUM(quantity_remaining), 0) AS REAL) FROM product_batches
         WHERE product_id = $1 AND quantity_remaining > 0 AND expiry_date < $2",
    )
    .bind(product_id)
    .bind(today)
    .fetch_one(&mut **tx)
    .await?;
    Ok(n)
}

pub fn parse_date(s: &str) -> Result<String> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| Error::Invalid(format!("not a YYYY-MM-DD date: {s}")))
}

/// Takes `qty` out of the product's batches, soonest expiry first (undated
/// last). Stock that was never batched just isn't tracked, so running out
/// of batches is fine. With `fresh_from` (a YYYY-MM-DD date), batches that
/// expired before it are left alone.
pub async fn consume_batches(tx: &mut Transaction<'_, Sqlite>, product_id: i64, qty: f64, fresh_from: Option<&str>) -> Result<()> {
    let batches: Vec<(i64, f64)> = sqlx::query_as(
        "SELECT id, quantity_remaining FROM product_batches
         WHERE product_id = $1 AND quantity_remaining > 0
           AND ($2 IS NULL OR expiry_date IS NULL OR expiry_date >= $2)
         ORDER BY expiry_date IS NULL, expiry_date, received_at, id",
    )
    .bind(product_id)
    .bind(fresh_from)
    .fetch_all(&mut **tx)
    .await?;
    let mut left = qty;
    for (id, remaining) in batches {
        if left <= 0.0 {
            break;
        }
        let take = left.min(remaining);
        sqlx::query("UPDATE product_batches SET quantity_remaining = quantity_remaining - $1 WHERE id = $2")
            .bind(take)
            .bind(id)
            .execute(&mut **tx)
            .await?;
        left -= take;
    }
    Ok(())
}

/// Adds a batch row; used by receive_stock and by product creation.
pub async fn insert_batch(
    tx: &mut Transaction<'_, Sqlite>,
    product_id: i64,
    qty: f64,
    expiry: Option<&str>,
    cost: Option<f64>,
    note: Option<&str>,
    user_id: i64,
) -> Result<i64> {
    Ok(sqlx::query(
        "INSERT INTO product_batches (product_id, quantity_received, quantity_remaining, expiry_date,
            cost_price, note, received_at, received_by)
         VALUES ($1, $2, $2, $3, $4, $5, $6, $7)",
    )
    .bind(product_id)
    .bind(qty)
    .bind(expiry)
    .bind(cost)
    .bind(note)
    .bind(now())
    .bind(user_id)
    .execute(&mut **tx)
    .await?
    .last_insert_rowid())
}

fn trimmed(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiveArgs {
    pub product_id: i64,
    pub quantity: f64,
    pub expiry_date: Option<String>,
    /// Updates the product's cost price when given
    pub cost_price: Option<f64>,
    pub note: Option<String>,
}

pub async fn receive_stock(ctx: &Ctx, args: ReceiveArgs) -> Result<()> {
    let actor = auth::require(ctx, "inventory.adjust_stock").await?;
    if args.quantity <= 0.0 || args.cost_price.is_some_and(|c| c < 0.0) {
        return Err(Error::Rule("negative_amount"));
    }
    let expiry = trimmed(&args.expiry_date).map(parse_date).transpose()?;
    let mut tx = ctx.pool.begin().await?;
    let done = sqlx::query("UPDATE products SET stock_quantity = stock_quantity + $1 WHERE id = $2")
        .bind(args.quantity)
        .bind(args.product_id)
        .execute(&mut *tx)
        .await?;
    if done.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    if let Some(cost) = args.cost_price {
        sqlx::query("UPDATE products SET cost_price = $1 WHERE id = $2")
            .bind(cost)
            .bind(args.product_id)
            .execute(&mut *tx)
            .await?;
    }
    insert_batch(&mut tx, args.product_id, args.quantity, expiry.as_deref(), args.cost_price, trimmed(&args.note), actor.user_id).await?;
    sqlx::query(
        "INSERT INTO stock_adjustments (product_id, adjustment_type, quantity, reason, user_id, created_at)
         VALUES ($1, 'addition', $2, $3, $4, $5)",
    )
    .bind(args.product_id)
    .bind(args.quantity)
    .bind(trimmed(&args.note).unwrap_or("received"))
    .bind(actor.user_id)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "stock.receive", "products", Some(args.product_id), &serde_json::json!({ "quantity": args.quantity, "expiry": expiry })).await?;
    notifications::refresh_quietly(ctx).await;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveArgs {
    pub product_id: i64,
    pub quantity: f64,
    /// damaged | expired | correction | other
    pub reason: String,
    /// Take it from this batch (e.g. throwing out an expired one)
    pub batch_id: Option<i64>,
    pub note: Option<String>,
}

pub async fn remove_stock(ctx: &Ctx, args: RemoveArgs) -> Result<()> {
    let actor = auth::require(ctx, "inventory.adjust_stock").await?;
    if !matches!(args.reason.as_str(), "damaged" | "expired" | "correction" | "other") {
        return Err(Error::Invalid(format!("unknown reason {}", args.reason)));
    }
    if args.quantity <= 0.0 {
        return Err(Error::Rule("negative_amount"));
    }
    let mut tx = ctx.pool.begin().await?;
    let (name, stock): (String, f64) = sqlx::query_as("SELECT name, CAST(stock_quantity AS REAL) FROM products WHERE id = $1")
        .bind(args.product_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::NotFound)?;
    if args.quantity > stock {
        return Err(Error::RuleAbout("insufficient_stock", name));
    }
    match args.batch_id {
        Some(batch) => {
            let done = sqlx::query(
                "UPDATE product_batches SET quantity_remaining = MAX(0, quantity_remaining - $1)
                 WHERE id = $2 AND product_id = $3",
            )
            .bind(args.quantity)
            .bind(batch)
            .bind(args.product_id)
            .execute(&mut *tx)
            .await?;
            if done.rows_affected() == 0 {
                return Err(Error::NotFound);
            }
        }
        None => consume_batches(&mut tx, args.product_id, args.quantity, None).await?,
    }
    sqlx::query("UPDATE products SET stock_quantity = stock_quantity - $1 WHERE id = $2")
        .bind(args.quantity)
        .bind(args.product_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO stock_adjustments (product_id, adjustment_type, quantity, reason, user_id, created_at)
         VALUES ($1, 'subtraction', $2, $3, $4, $5)",
    )
    .bind(args.product_id)
    .bind(-args.quantity)
    .bind(match trimmed(&args.note) {
        Some(note) => format!("{}: {note}", args.reason),
        None => args.reason.clone(),
    })
    .bind(actor.user_id)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "stock.remove", "products", Some(args.product_id), &serde_json::json!({ "quantity": args.quantity, "reason": args.reason })).await?;
    notifications::refresh_quietly(ctx).await;
    Ok(())
}
