//! Loyalty points. On unless the shop turns them off (`loyalty_enabled`).
//!
//! - **Earning**: a sale with a customer earns one point for every
//!   `loyalty_earn_per` of money actually paid (cash or card). What's left on
//!   account and what's paid with points earn nothing.
//! - **Spending**: points pay for part or all of a sale, each worth
//!   `loyalty_point_value`. It's a `points` payment, so the sale's payments
//!   still add up to its total.
//! - **Returns**: points that paid for a sale go back as points first; points
//!   the sale earned are taken back in proportion (never below zero).
//!
//! `customers.loyalty_points` is the balance; `loyalty_ledger` is every
//! change to it.

use serde::Deserialize;
use sqlx::{Sqlite, Transaction};

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};

pub struct Config {
    /// Money paid for one point
    pub earn_per: f64,
    /// What one point is worth when spent
    pub point_value: f64,
}

async fn setting(tx: &mut Transaction<'_, Sqlite>, key: &str) -> Result<Option<String>> {
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT value FROM settings WHERE key = $1")
        .bind(key)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(row.and_then(|(v,)| v))
}

/// None when the shop has turned points off.
pub async fn config(tx: &mut Transaction<'_, Sqlite>) -> Result<Option<Config>> {
    if setting(tx, "loyalty_enabled").await?.as_deref() == Some("0") {
        return Ok(None);
    }
    let num = |v: Option<String>, d: f64| v.and_then(|s| s.parse::<f64>().ok()).filter(|n| *n > 0.0).unwrap_or(d);
    Ok(Some(Config {
        earn_per: num(setting(tx, "loyalty_earn_per").await?, 1000.0),
        point_value: num(setting(tx, "loyalty_point_value").await?, 10.0),
    }))
}

pub async fn balance(tx: &mut Transaction<'_, Sqlite>, customer_id: i64) -> Result<i64> {
    let row: Option<(Option<i64>,)> = sqlx::query_as("SELECT loyalty_points FROM customers WHERE id = $1")
        .bind(customer_id)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(row.ok_or(Error::NotFound)?.0.unwrap_or(0))
}

pub struct Change<'a> {
    pub customer_id: i64,
    pub kind: &'a str,
    pub points: i64,
    pub sale_id: Option<i64>,
    pub return_id: Option<i64>,
    pub note: Option<&'a str>,
    pub user_id: i64,
    pub at: &'a str,
}

/// Adds a ledger row and moves the balance with it.
pub async fn write(tx: &mut Transaction<'_, Sqlite>, c: Change<'_>) -> Result<()> {
    if c.points == 0 {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO loyalty_ledger (customer_id, kind, points, sale_id, return_id, note, user_id, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(c.customer_id)
    .bind(c.kind)
    .bind(c.points)
    .bind(c.sale_id)
    .bind(c.return_id)
    .bind(c.note)
    .bind(c.user_id)
    .bind(c.at)
    .execute(&mut **tx)
    .await?;
    sqlx::query("UPDATE customers SET loyalty_points = COALESCE(loyalty_points, 0) + $1 WHERE id = $2")
        .bind(c.points)
        .bind(c.customer_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Points a payment of `paid` earns.
pub fn earned(cfg: &Config, paid: f64) -> i64 {
    if paid <= 0.0 {
        0
    } else {
        // A hair over, so 1,000 paid at 1,000 a point is 1 point, not 0.999…
        ((paid + 1e-6) / cfg.earn_per).floor() as i64
    }
}

/// What returning `refund` of a sale does to its points: how much of the
/// refund goes back as points (money value, and the points), and how many
/// earned points are taken back.
pub struct ReturnPoints {
    pub refund_value: f64,
    pub refund_points: i64,
    pub take_back: i64,
}

/// Works out and writes the points side of a return. `refund` is the
/// return's whole value; the caller pays out `refund - refund_value` in money.
/// `points_refund` false (an exchange): only earned points are taken back.
pub async fn on_return(
    tx: &mut Transaction<'_, Sqlite>,
    sale_id: i64,
    return_id: i64,
    refund: f64,
    points_refund: bool,
    user_id: i64,
    at: &str,
) -> Result<ReturnPoints> {
    let none = ReturnPoints { refund_value: 0.0, refund_points: 0, take_back: 0 };
    let row: Option<(Option<i64>, f64)> = sqlx::query_as("SELECT customer_id, total_amount FROM sales WHERE id = $1")
        .bind(sale_id)
        .fetch_optional(&mut **tx)
        .await?;
    let Some((Some(customer_id), total)) = row else {
        return Ok(none);
    };
    // Points that paid for the sale, and how much of that has gone back
    let (paid_value, refunded_value): (f64, f64) = sqlx::query_as(
        "SELECT CAST(COALESCE(SUM(CASE WHEN amount > 0 THEN amount END), 0) AS REAL),
                CAST(COALESCE(-SUM(CASE WHEN amount < 0 THEN amount END), 0) AS REAL)
         FROM payments WHERE sale_id = $1 AND payment_method = 'points'",
    )
    .bind(sale_id)
    .fetch_one(&mut **tx)
    .await?;
    let (spent, earned_total, taken): (i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(-SUM(CASE WHEN kind = 'redeem' THEN points END), 0),
                COALESCE(SUM(CASE WHEN kind = 'earn' THEN points END), 0),
                COALESCE(-SUM(CASE WHEN kind = 'refund' AND points < 0 THEN points END), 0)
         FROM loyalty_ledger WHERE sale_id = $1",
    )
    .bind(sale_id)
    .fetch_one(&mut **tx)
    .await?;

    let mut out = none;
    if points_refund && paid_value > refunded_value + 1e-9 && spent > 0 {
        out.refund_value = refund.min(paid_value - refunded_value);
        out.refund_points = ((out.refund_value / paid_value) * spent as f64).round() as i64;
        sqlx::query(
            "INSERT INTO payments (sale_id, payment_method, amount, return_id, payment_date) VALUES ($1, 'points', $2, $3, $4)",
        )
        .bind(sale_id)
        .bind(-out.refund_value)
        .bind(return_id)
        .bind(at)
        .execute(&mut **tx)
        .await?;
        write(tx, Change { customer_id, kind: "refund", points: out.refund_points, sale_id: Some(sale_id), return_id: Some(return_id), note: None, user_id, at }).await?;
    }
    if earned_total > taken && total > 0.0 {
        let share = ((earned_total as f64) * (refund / total).min(1.0)).round() as i64;
        let have = balance(tx, customer_id).await?.max(0);
        out.take_back = share.min(earned_total - taken).min(have);
        write(tx, Change { customer_id, kind: "refund", points: -out.take_back, sale_id: Some(sale_id), return_id: Some(return_id), note: None, user_id, at }).await?;
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdjustPointsArgs {
    pub customer_id: i64,
    /// + given, − taken away
    pub points: i64,
    pub note: String,
}

/// Points given or taken by hand. Always with a reason. Returns the new balance.
pub async fn adjust_loyalty_points(ctx: &Ctx, args: AdjustPointsArgs) -> Result<i64> {
    let actor = auth::require(ctx, "customers.edit").await?;
    let note = args.note.trim();
    if note.is_empty() {
        return Err(Error::Rule("note_required"));
    }
    if args.points == 0 {
        return Err(Error::Rule("points_amount"));
    }
    let at = now();
    let mut tx = ctx.pool.begin().await?;
    if config(&mut tx).await?.is_none() {
        return Err(Error::Rule("loyalty_off"));
    }
    let have = balance(&mut tx, args.customer_id).await?;
    if have + args.points < 0 {
        return Err(Error::Rule("not_enough_points"));
    }
    write(&mut tx, Change { customer_id: args.customer_id, kind: "adjustment", points: args.points, sale_id: None, return_id: None, note: Some(note), user_id: actor.user_id, at: &at }).await?;
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "loyalty.adjust", "customers", Some(args.customer_id), &serde_json::json!({ "points": args.points, "note": note })).await?;
    Ok(have + args.points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::catalog::create_product;
    use crate::ctx::test_support;
    use crate::customers::{create_customer, CustomerInput};
    use crate::returns::{create_return, ReturnArgs, ReturnItems, ReturnLine};
    use crate::sales::tests::{cards_on, product, sell};
    use crate::sales::checkout;

    async fn points_of(ctx: &Ctx, id: i64) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT loyalty_points FROM customers WHERE id = $1").bind(id).fetch_one(&ctx.pool).await.unwrap().0
    }

    async fn paid(ctx: &Ctx, sale: i64, method: &str) -> f64 {
        sqlx::query_as::<_, (f64,)>("SELECT CAST(COALESCE(SUM(amount), 0) AS REAL) FROM payments WHERE sale_id = $1 AND payment_method = $2")
            .bind(sale)
            .bind(method)
            .fetch_one(&ctx.pool)
            .await
            .unwrap()
            .0
    }

    #[test]
    fn earn_spend_split_and_return() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            // 750 a can; 1 point per 1,000 paid, worth 10 each (the defaults)
            let cola = create_product(&ctx, product("Cola", "1", 100.0)).await.unwrap();
            let lana = create_customer(&ctx, CustomerInput { name: "Lana".into(), is_active: true, ..Default::default() }).await.unwrap();

            // 40 cans = 30,000 → 30 points
            let mut args = sell(cola, 40.0, "cash");
            args.customer_id = Some(lana);
            let first = checkout(&ctx, args).await.unwrap();
            assert_eq!((first.points_earned, first.points_balance), (30, Some(30)));

            // Spend 25 points (250) on 2 cans: 1,250 left to pay, earning 1
            let mut args = sell(cola, 2.0, "cash");
            args.customer_id = Some(lana);
            args.points = 25;
            let second = checkout(&ctx, args).await.unwrap();
            assert_eq!(second.points_value, 250.0);
            assert_eq!(paid(&ctx, second.id, "points").await, 250.0);
            assert_eq!(paid(&ctx, second.id, "cash").await, 1250.0);
            assert_eq!(points_of(&ctx, lana).await, 6);

            // More than they have, or more than the sale is worth
            let mut args = sell(cola, 1.0, "cash");
            args.customer_id = Some(lana);
            args.points = 7;
            assert!(matches!(checkout(&ctx, args).await, Err(Error::Rule("not_enough_points"))));
            let mut args = sell(cola, 1.0, "cash");
            args.points = 1;
            assert!(matches!(checkout(&ctx, args).await, Err(Error::Rule("customer_required"))));

            // Returning both cans of the second sale: the 250 paid in points
            // comes back as 25 points, the 1 point earned goes, 1,250 in cash
            let (line,): (i64,) = sqlx::query_as("SELECT id FROM sale_items WHERE sale_id = $1").bind(second.id).fetch_one(&ctx.pool).await.unwrap();
            let r = create_return(&ctx, ReturnArgs {
                items: ReturnItems { sale_id: Some(second.id), lines: vec![ReturnLine { sale_item_id: line, quantity: 2.0, restock: true }], ..Default::default() },
                refund_method: "cash".into(),
                override_token: None,
            })
            .await
            .unwrap();
            assert_eq!((r.points_value, r.points_back, r.points_taken), (250.0, 25, 1));
            assert_eq!(paid(&ctx, second.id, "cash").await, 0.0);
            assert_eq!(points_of(&ctx, lana).await, 30);

            // Split: card off by default, then 1,000 by card and 500 cash
            let mut args = sell(cola, 2.0, "cash");
            args.card_amount = 1000.0;
            assert!(matches!(checkout(&ctx, args).await, Err(Error::Rule("card_payments_off"))));
            cards_on(&ctx).await;
            let mut args = sell(cola, 2.0, "cash");
            args.card_amount = 1000.0;
            args.tendered = Some(1000.0);
            let split = checkout(&ctx, args).await.unwrap();
            assert_eq!((paid(&ctx, split.id, "card").await, paid(&ctx, split.id, "cash").await), (1000.0, 500.0));
            assert_eq!(split.change, Some(500.0));
            let mut args = sell(cola, 2.0, "cash");
            args.card_amount = 2000.0;
            assert!(matches!(checkout(&ctx, args).await, Err(Error::Rule("split_too_large"))));

            // Switched off: nothing earned, nothing spent
            sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('loyalty_enabled', '0')").execute(&ctx.pool).await.unwrap();
            let mut args = sell(cola, 4.0, "cash");
            args.customer_id = Some(lana);
            let off = checkout(&ctx, args).await.unwrap();
            assert_eq!((off.points_earned, off.points_balance), (0, None));
            let mut args = sell(cola, 1.0, "cash");
            args.customer_id = Some(lana);
            args.points = 5;
            assert!(matches!(checkout(&ctx, args).await, Err(Error::Rule("loyalty_off"))));
        });
    }

    #[test]
    fn earning_rounds_down_but_not_below_a_whole_point() {
        let cfg = Config { earn_per: 1000.0, point_value: 10.0 };
        assert_eq!(earned(&cfg, 999.0), 0);
        assert_eq!(earned(&cfg, 1000.0), 1);
        assert_eq!(earned(&cfg, 25_750.0), 25);
        assert_eq!(earned(&cfg, -500.0), 0);
    }
}
