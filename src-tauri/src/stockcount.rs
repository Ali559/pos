//! Stock counts: walk a shelf or a category, enter (or scan) what's there,
//! then apply the differences in one go.
//!
//! Each counted item remembers what the system said at the moment it was
//! counted (`expected`). Applying moves stock by `counted − expected`, so a
//! sale made while the count was open isn't counted twice. A count can be
//! left open and continued later.

use serde::{Deserialize, Serialize};

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::{notifications, stock};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartCountArgs {
    pub name: Option<String>,
    /// Only this category (and its subcategories)
    pub category_id: Option<i64>,
}

pub async fn start_stock_count(ctx: &Ctx, args: StartCountArgs) -> Result<i64> {
    let actor = auth::require(ctx, "inventory.count").await?;
    let name = args.name.as_deref().map(str::trim).filter(|s| !s.is_empty());
    Ok(sqlx::query("INSERT INTO stock_counts (name, category_id, user_id, started_at) VALUES ($1, $2, $3, $4)")
        .bind(name)
        .bind(args.category_id)
        .bind(actor.user_id)
        .bind(now())
        .execute(&ctx.pool)
        .await?
        .last_insert_rowid())
}

async fn open_count(ctx: &Ctx, count_id: i64) -> Result<()> {
    let row: Option<(String,)> = sqlx::query_as("SELECT status FROM stock_counts WHERE id = $1").bind(count_id).fetch_optional(&ctx.pool).await?;
    match row {
        Some((s,)) if s == "open" => Ok(()),
        Some(_) => Err(Error::Rule("count_closed")),
        None => Err(Error::NotFound),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordCountArgs {
    pub count_id: i64,
    pub product_id: i64,
    /// What's on the shelf, in the product's own unit
    pub counted: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counted {
    pub expected: f64,
    pub counted: f64,
}

/// Records (or corrects) one product's count. Counting it again starts
/// from the system's figure now.
pub async fn record_count(ctx: &Ctx, args: RecordCountArgs) -> Result<Counted> {
    auth::require(ctx, "inventory.count").await?;
    open_count(ctx, args.count_id).await?;
    if !(args.counted >= 0.0) {
        return Err(Error::Rule("negative_amount"));
    }
    let row: Option<(String, String, f64)> = sqlx::query_as("SELECT name, unit, CAST(stock_quantity AS REAL) FROM products WHERE id = $1")
        .bind(args.product_id)
        .fetch_optional(&ctx.pool)
        .await?;
    let Some((name, unit, expected)) = row else { return Err(Error::NotFound) };
    if crate::catalog::is_whole_unit(&unit) && args.counted.fract() != 0.0 {
        return Err(Error::RuleAbout("quantity_whole", name));
    }
    sqlx::query(
        "INSERT INTO stock_count_items (count_id, product_id, expected, counted, counted_at) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT(count_id, product_id) DO UPDATE SET expected = excluded.expected, counted = excluded.counted, counted_at = excluded.counted_at",
    )
    .bind(args.count_id)
    .bind(args.product_id)
    .bind(expected)
    .bind(args.counted)
    .bind(now())
    .execute(&ctx.pool)
    .await?;
    Ok(Counted { expected, counted: args.counted })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountItemArgs {
    pub count_id: i64,
    pub product_id: i64,
}

pub async fn remove_count_item(ctx: &Ctx, args: CountItemArgs) -> Result<()> {
    auth::require(ctx, "inventory.count").await?;
    open_count(ctx, args.count_id).await?;
    sqlx::query("DELETE FROM stock_count_items WHERE count_id = $1 AND product_id = $2")
        .bind(args.count_id)
        .bind(args.product_id)
        .execute(&ctx.pool)
        .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountIdArgs {
    pub count_id: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    /// Products whose stock changed
    pub changed: i64,
    /// Total units found (+) and missing (−)
    pub gained: f64,
    pub lost: f64,
}

/// Applies every difference and closes the count.
pub async fn apply_stock_count(ctx: &Ctx, args: CountIdArgs) -> Result<Applied> {
    let actor = auth::require(ctx, "inventory.adjust_stock").await?;
    open_count(ctx, args.count_id).await?;
    let at = now();
    let reason = format!("count #{}", args.count_id);
    let mut tx = ctx.pool.begin().await?;
    let items: Vec<(i64, f64, f64, bool)> = sqlx::query_as(
        "SELECT i.product_id, CAST(i.expected AS REAL), CAST(i.counted AS REAL), p.track_stock
         FROM stock_count_items i JOIN products p ON p.id = i.product_id WHERE i.count_id = $1",
    )
    .bind(args.count_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut out = Applied { changed: 0, gained: 0.0, lost: 0.0 };
    for (product_id, expected, counted, tracked) in items {
        let delta = counted - expected;
        if !tracked || delta.abs() < 1e-9 {
            continue;
        }
        sqlx::query("UPDATE products SET stock_quantity = MAX(0, stock_quantity + $1) WHERE id = $2")
            .bind(delta)
            .bind(product_id)
            .execute(&mut *tx)
            .await?;
        if delta < 0.0 {
            stock::consume_batches(&mut tx, product_id, -delta, None).await?;
            out.lost += -delta;
        } else {
            stock::insert_batch(&mut tx, product_id, delta, None, None, Some(&reason), actor.user_id).await?;
            out.gained += delta;
        }
        sqlx::query(
            "INSERT INTO stock_adjustments (product_id, adjustment_type, quantity, reason, user_id, reference_id, created_at)
             VALUES ($1, 'correction', $2, $3, $4, $5, $6)",
        )
        .bind(product_id)
        .bind(delta)
        .bind(&reason)
        .bind(actor.user_id)
        .bind(args.count_id)
        .bind(&at)
        .execute(&mut *tx)
        .await?;
        out.changed += 1;
    }
    sqlx::query("UPDATE stock_counts SET status = 'applied', finished_at = $1, finished_by = $2 WHERE id = $3")
        .bind(&at)
        .bind(actor.user_id)
        .bind(args.count_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "stock.count", "stock_counts", Some(args.count_id), &serde_json::json!({ "changed": out.changed, "gained": out.gained, "lost": out.lost })).await?;
    notifications::refresh_quietly(ctx).await;
    Ok(out)
}

pub async fn cancel_stock_count(ctx: &Ctx, args: CountIdArgs) -> Result<()> {
    let actor = auth::require(ctx, "inventory.count").await?;
    open_count(ctx, args.count_id).await?;
    sqlx::query("UPDATE stock_counts SET status = 'cancelled', finished_at = $1, finished_by = $2 WHERE id = $3")
        .bind(now())
        .bind(actor.user_id)
        .bind(args.count_id)
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
    use crate::sales::checkout;
    use crate::sales::tests::{product, sell};

    async fn stock_of(ctx: &Ctx, id: i64) -> f64 {
        sqlx::query_as::<_, (f64,)>("SELECT CAST(stock_quantity AS REAL) FROM products WHERE id = $1").bind(id).fetch_one(&ctx.pool).await.unwrap().0
    }

    #[test]
    fn counts_apply_the_difference_even_with_sales_in_between() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let cola = create_product(&ctx, product("Cola", "1", 20.0)).await.unwrap();
            let water = create_product(&ctx, product("Water", "2", 10.0)).await.unwrap();
            let count = start_stock_count(&ctx, StartCountArgs { name: Some("Drinks shelf".into()), category_id: None }).await.unwrap();

            // 18 cola on the shelf (2 missing), 12 water (2 found)
            record_count(&ctx, RecordCountArgs { count_id: count, product_id: cola, counted: 18.0 }).await.unwrap();
            record_count(&ctx, RecordCountArgs { count_id: count, product_id: water, counted: 12.0 }).await.unwrap();
            let r = record_count(&ctx, RecordCountArgs { count_id: count, product_id: water, counted: 1.5 }).await;
            assert!(matches!(r, Err(Error::RuleAbout("quantity_whole", _))));

            // A sale after counting: 3 cola leave the shelf and the system
            checkout(&ctx, sell(cola, 3.0, "cash")).await.unwrap();

            let applied = apply_stock_count(&ctx, CountIdArgs { count_id: count }).await.unwrap();
            assert_eq!((applied.changed, applied.gained, applied.lost), (2, 2.0, 2.0));
            assert_eq!(stock_of(&ctx, cola).await, 15.0, "18 counted, then 3 sold");
            assert_eq!(stock_of(&ctx, water).await, 12.0);

            // Closed: no more changes
            let r = record_count(&ctx, RecordCountArgs { count_id: count, product_id: cola, counted: 1.0 }).await;
            assert!(matches!(r, Err(Error::Rule("count_closed"))));
        });
    }
}
