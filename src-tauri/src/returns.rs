//! Returns and refunds against a past sale. What's refunded is worked out
//! from the sale lines as sold (after discounts, plus tax), never from the
//! screen; the WebView sends only which lines and how many.

use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, Transaction};

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::sales::{round_money, shop_currency};
use crate::{notifications, shifts, stock};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnLine {
    pub sale_item_id: i64,
    pub quantity: f64,
    /// Put it back on the shelf (false for damaged or opened goods)
    pub restock: bool,
}

/// A line returned without a receipt: refunded at today's price.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenReturnLine {
    pub product_id: i64,
    pub quantity: f64,
    pub restock: bool,
}

/// What comes back. With `sale_id`, `lines` name lines of that sale; without
/// one (no receipt), `products` name products.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnItems {
    pub sale_id: Option<i64>,
    #[serde(default)]
    pub lines: Vec<ReturnLine>,
    #[serde(default)]
    pub products: Vec<OpenReturnLine>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnArgs {
    #[serde(flatten)]
    pub items: ReturnItems,
    /// cash | card
    pub refund_method: String,
    pub override_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnResult {
    pub id: i64,
    pub refund: f64,
}

/// One line ready to write: amounts already worked out.
struct Prepared {
    sale_item_id: Option<i64>,
    product_id: i64,
    name: String,
    qty: f64,
    cost: f64,
    /// Before tax
    subtotal: f64,
    /// With tax
    refund: f64,
    restock: bool,
}

fn check_qty(name: &str, unit: &str, qty: f64) -> Result<()> {
    if !(qty > 0.0) {
        return Err(Error::Rule("negative_amount"));
    }
    if matches!(unit, "piece" | "pack" | "box") && qty.fract() != 0.0 {
        return Err(Error::RuleAbout("quantity_whole", name.to_owned()));
    }
    Ok(())
}

async fn prepare_sale_lines(tx: &mut Transaction<'_, Sqlite>, sale_id: i64, lines: &[ReturnLine], currency: &str) -> Result<Vec<Prepared>> {
    let status: Option<(String,)> = sqlx::query_as("SELECT payment_status FROM sales WHERE id = $1")
        .bind(sale_id)
        .fetch_optional(&mut **tx)
        .await?;
    if status.is_none() {
        return Err(Error::NotFound);
    }
    let mut out = Vec::new();
    for l in lines.iter().filter(|l| l.quantity != 0.0) {
        let row: Option<(i64, String, f64, f64, f64, f64, String, f64, f64)> = sqlx::query_as(
            "SELECT si.product_id, si.product_name, CAST(si.quantity AS REAL), CAST(si.subtotal AS REAL),
                    CAST(COALESCE(si.tax_rate, 0) AS REAL), CAST(si.cost_price AS REAL), COALESCE(p.unit, 'piece'),
                    CAST(COALESCE((SELECT SUM(quantity) FROM return_items WHERE sale_item_id = si.id), 0) AS REAL),
                    CAST(COALESCE((SELECT SUM(subtotal) FROM return_items WHERE sale_item_id = si.id), 0) AS REAL)
             FROM sale_items si LEFT JOIN products p ON p.id = si.product_id
             WHERE si.id = $1 AND si.sale_id = $2",
        )
        .bind(l.sale_item_id)
        .bind(sale_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((product_id, name, sold, line_subtotal, tax_rate, cost, unit, returned, returned_subtotal)) = row else {
            return Err(Error::NotFound);
        };
        check_qty(&name, &unit, l.quantity)?;
        if l.quantity > sold - returned + 1e-9 {
            return Err(Error::RuleAbout("return_exceeds_sold", name));
        }
        // The last of a line gets exactly what's left of it, so rounding
        // never refunds more than was paid
        let subtotal = if (sold - returned - l.quantity).abs() < 1e-9 {
            line_subtotal - returned_subtotal
        } else {
            round_money(line_subtotal / sold * l.quantity, currency)
        };
        let refund = subtotal + round_money(subtotal * tax_rate / 100.0, currency);
        out.push(Prepared { sale_item_id: Some(l.sale_item_id), product_id, name, qty: l.quantity, cost, subtotal, refund, restock: l.restock });
    }
    Ok(out)
}

/// No receipt: today's price and tax rate, no discounts to unpick.
async fn prepare_open_lines(tx: &mut Transaction<'_, Sqlite>, lines: &[OpenReturnLine], currency: &str) -> Result<Vec<Prepared>> {
    let (tax_rate,): (f64,) = sqlx::query_as(
        "SELECT COALESCE((SELECT CAST(value AS REAL) FROM settings WHERE key = 'tax_rate'), 0)",
    )
    .fetch_one(&mut **tx)
    .await?;
    let mut out = Vec::new();
    for l in lines.iter().filter(|l| l.quantity != 0.0) {
        let row: Option<(String, f64, f64, String)> = sqlx::query_as(
            "SELECT name, CAST(selling_price AS REAL), CAST(cost_price AS REAL), unit FROM products WHERE id = $1",
        )
        .bind(l.product_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((name, price, cost, unit)) = row else {
            return Err(Error::NotFound);
        };
        check_qty(&name, &unit, l.quantity)?;
        let subtotal = round_money(price * l.quantity, currency);
        let refund = subtotal + round_money(subtotal * tax_rate / 100.0, currency);
        out.push(Prepared { sale_item_id: None, product_id: l.product_id, name, qty: l.quantity, cost, subtotal, refund, restock: l.restock });
    }
    Ok(out)
}

/// Checks the permission (or a manager's approval) and works out the lines.
pub struct PendingReturn {
    sale_id: Option<i64>,
    lines: Vec<Prepared>,
    reason: Option<String>,
    pub approved_by: Option<i64>,
    pub total: f64,
}

pub async fn prepare(
    ctx: &Ctx,
    tx: &mut Transaction<'_, Sqlite>,
    actor: &auth::Actor,
    items: &ReturnItems,
    override_token: Option<&str>,
    currency: &str,
) -> Result<PendingReturn> {
    let approved_by = auth::require_or_override(ctx, actor, "sales.refund", override_token).await?;
    let lines = match items.sale_id {
        Some(sale_id) => prepare_sale_lines(tx, sale_id, &items.lines, currency).await?,
        None => prepare_open_lines(tx, &items.products, currency).await?,
    };
    if lines.is_empty() {
        return Err(Error::Rule("empty_return"));
    }
    let total = lines.iter().map(|l| l.refund).sum();
    let reason = items.reason.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
    Ok(PendingReturn { sale_id: items.sale_id, lines, reason, approved_by, total })
}

/// Writes the return, its lines and any restocking. Payments are the
/// caller's (a plain refund, or an exchange). Returns the return's id.
pub async fn write(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PendingReturn,
    user_id: i64,
    shift_id: Option<i64>,
    method: &str,
    at: &str,
) -> Result<i64> {
    let id = sqlx::query(
        "INSERT INTO returns (original_sale_id, return_date, user_id, total_refund, reason, shift_id, refund_method, approved_by, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $2)",
    )
    .bind(p.sale_id)
    .bind(at)
    .bind(user_id)
    .bind(p.total)
    .bind(&p.reason)
    .bind(shift_id)
    .bind(method)
    .bind(p.approved_by)
    .execute(&mut **tx)
    .await?
    .last_insert_rowid();

    for l in &p.lines {
        sqlx::query(
            "INSERT INTO return_items (return_id, sale_item_id, product_id, product_name, quantity, cost_price, subtotal, refund_amount, restocked)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(l.sale_item_id)
        .bind(l.product_id)
        .bind(&l.name)
        .bind(l.qty)
        .bind(l.cost)
        .bind(l.subtotal)
        .bind(l.refund)
        .bind(l.restock)
        .execute(&mut **tx)
        .await?;
        // Nothing to put back for products that don't count stock
        let (tracked,): (bool,) = sqlx::query_as("SELECT track_stock FROM products WHERE id = $1")
            .bind(l.product_id)
            .fetch_one(&mut **tx)
            .await?;
        if l.restock && tracked {
            sqlx::query("UPDATE products SET stock_quantity = stock_quantity + $1 WHERE id = $2")
                .bind(l.qty)
                .bind(l.product_id)
                .execute(&mut **tx)
                .await?;
            stock::insert_batch(tx, l.product_id, l.qty, None, None, Some("return"), user_id).await?;
            sqlx::query(
                "INSERT INTO stock_adjustments (product_id, adjustment_type, quantity, reason, user_id, reference_id, created_at)
                 VALUES ($1, 'return', $2, 'return', $3, $4, $5)",
            )
            .bind(l.product_id)
            .bind(l.qty)
            .bind(user_id)
            .bind(id)
            .bind(at)
            .execute(&mut **tx)
            .await?;
        }
    }

    if let Some(sale_id) = p.sale_id {
        let (left,): (f64,) = sqlx::query_as(
            "SELECT CAST(COALESCE(SUM(si.quantity - COALESCE((SELECT SUM(ri.quantity) FROM return_items ri WHERE ri.sale_item_id = si.id), 0)), 0) AS REAL)
             FROM sale_items si WHERE si.sale_id = $1",
        )
        .bind(sale_id)
        .fetch_one(&mut **tx)
        .await?;
        sqlx::query("UPDATE sales SET payment_status = $1 WHERE id = $2")
            .bind(if left <= 1e-9 { "refunded" } else { "partially_refunded" })
            .bind(sale_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(id)
}

/// Money going back out for a return (negative amount).
pub async fn pay_out(tx: &mut Transaction<'_, Sqlite>, sale_id: Option<i64>, return_id: i64, method: &str, amount: f64, shift_id: Option<i64>, at: &str) -> Result<()> {
    if amount == 0.0 {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO payments (sale_id, payment_method, amount, shift_id, return_id, payment_date)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(sale_id)
    .bind(method)
    .bind(-amount)
    .bind(shift_id)
    .bind(return_id)
    .bind(at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// A plain return: items back, money back.
pub async fn create_return(ctx: &Ctx, args: ReturnArgs) -> Result<ReturnResult> {
    let actor = auth::require(ctx, "sales.sell").await?;
    if !matches!(args.refund_method.as_str(), "cash" | "card") {
        return Err(Error::Invalid(format!("unknown refund method {}", args.refund_method)));
    }
    let currency = shop_currency(ctx).await?;
    let shift_id = shifts::shift_info(ctx, actor.user_id).await?.map(|s| s.id);
    let at = now();

    let mut tx = ctx.pool.begin().await?;
    let pending = prepare(ctx, &mut tx, &actor, &args.items, args.override_token.as_deref(), &currency).await?;
    let id = write(&mut tx, &pending, actor.user_id, shift_id, &args.refund_method, &at).await?;
    pay_out(&mut tx, pending.sale_id, id, &args.refund_method, pending.total, shift_id, &at).await?;
    tx.commit().await?;

    audit::log(
        ctx,
        Some(actor.user_id),
        "refund",
        "returns",
        Some(id),
        &serde_json::json!({ "sale": pending.sale_id, "refund": pending.total, "approvedBy": pending.approved_by }),
    )
    .await?;
    notifications::refresh_quietly(ctx).await;
    Ok(ReturnResult { id, refund: pending.total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::catalog::create_product;
    use crate::ctx::test_support;
    use crate::sales::tests::{product, sell};
    use crate::sales::{checkout, CheckoutArgs, CheckoutLine};

    async fn one(ctx: &Ctx, sql: &str, id: i64) -> f64 {
        sqlx::query_as::<_, (f64,)>(&format!("SELECT CAST(({sql}) AS REAL)")).bind(id).fetch_one(&ctx.pool).await.unwrap().0
    }

    #[test]
    fn discounts_are_spread_over_lines_and_refunds_follow_them() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            crate::settings::set_setting(&ctx, crate::settings::SetSettingArgs { key: "tax_rate".into(), value: "10".into() })
                .await
                .unwrap();
            // 750 each
            let a = create_product(&ctx, product("Cola", "1", 10.0)).await.unwrap();
            let b = create_product(&ctx, product("Chips", "2", 10.0)).await.unwrap();
            let sale = checkout(&ctx, CheckoutArgs {
                lines: vec![
                    CheckoutLine { product_id: a, quantity: 4.0, discount: 0.0, ..Default::default() },
                    CheckoutLine { product_id: b, quantity: 2.0, discount: 500.0, ..Default::default() },
                ],
                cart_discount: 1000.0,
                payment_method: "cash".into(),
                tendered: None,
                override_token: None,
                exchange: None,
                refund_override_token: None,
                open_price_override_token: None,
            })
            .await
            .unwrap();
            // 4500 gross, 1500 off, tax on 3000
            assert_eq!((sale.subtotal, sale.discount, sale.tax, sale.total), (4500.0, 1500.0, 300.0, 3300.0));
            let net: f64 = one(&ctx, "SELECT SUM(subtotal) FROM sale_items WHERE sale_id = $1", sale.id).await;
            assert_eq!(net, 3000.0, "line subtotals carry every discount");

            let (cola_line,): (i64,) = sqlx::query_as("SELECT id FROM sale_items WHERE sale_id = $1 AND product_id = $2")
                .bind(sale.id)
                .bind(a)
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            let line_net = one(&ctx, "SELECT subtotal FROM sale_items WHERE id = $1", cola_line).await;
            let ret = |qty: f64, restock: bool| ReturnArgs {
                items: ReturnItems { sale_id: Some(sale.id), lines: vec![ReturnLine { sale_item_id: cola_line, quantity: qty, restock }], ..Default::default() },
                refund_method: "cash".into(),
                override_token: None,
            };
            let first = create_return(&ctx, ret(1.0, true)).await.unwrap();
            // 2250 net over 4 → 563 each (dinar rounding), plus 10% tax
            assert_eq!(line_net, 2250.0);
            assert_eq!(first.refund, 563.0 + 56.0);
            assert_eq!(one(&ctx, "SELECT CAST(stock_quantity AS REAL) FROM products WHERE id = $1", a).await, 7.0);
            let r = create_return(&ctx, ret(4.0, false)).await;
            assert!(matches!(r, Err(Error::RuleAbout("return_exceeds_sold", _))));
            create_return(&ctx, ret(3.0, false)).await.unwrap();
            let refunded = one(&ctx, "SELECT SUM(subtotal) FROM return_items WHERE sale_item_id = $1", cola_line).await;
            assert_eq!(refunded, line_net, "the whole line refunds exactly what it sold for");
            assert_eq!(one(&ctx, "SELECT CAST(stock_quantity AS REAL) FROM products WHERE id = $1", a).await, 7.0, "not restocked");
            let (status,): (String,) = sqlx::query_as("SELECT payment_status FROM sales WHERE id = $1").bind(sale.id).fetch_one(&ctx.pool).await.unwrap();
            assert_eq!(status, "partially_refunded");
        });
    }

    #[test]
    fn cash_drawer_expects_float_plus_cash_sales_minus_refunds() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            crate::settings::set_setting(&ctx, crate::settings::SetSettingArgs { key: "opening_float".into(), value: "10000".into() })
                .await
                .unwrap();
            // The owner's shift started at setup, before the float was set
            sqlx::query("UPDATE shifts SET cash_drawer_id = NULL").execute(&ctx.pool).await.unwrap();
            sqlx::query("DELETE FROM cash_drawers").execute(&ctx.pool).await.unwrap();
            let id = create_product(&ctx, product("Cola", "1", 10.0)).await.unwrap();
            let s1 = checkout(&ctx, sell(id, 2.0, "cash")).await.unwrap();
            checkout(&ctx, sell(id, 1.0, "card")).await.unwrap();
            let (line,): (i64,) = sqlx::query_as("SELECT id FROM sale_items WHERE sale_id = $1").bind(s1.id).fetch_one(&ctx.pool).await.unwrap();
            create_return(&ctx, ReturnArgs {
                items: ReturnItems {
                    sale_id: Some(s1.id),
                    lines: vec![ReturnLine { sale_item_id: line, quantity: 1.0, restock: true }],
                    reason: Some("wrong flavour".into()),
                    ..Default::default()
                },
                refund_method: "cash".into(),
                override_token: None,
            })
            .await
            .unwrap();
            let sum = crate::shifts::shift_summary(&ctx).await.unwrap().unwrap();
            assert_eq!((sum.opening_float, sum.cash_sales, sum.card_sales, sum.cash_refunds), (10000.0, 1500.0, 750.0, -750.0));
            assert_eq!(sum.expected_cash, 10750.0);
            assert_eq!(sum.sales, 2);

            crate::auth::end_shift(&ctx, crate::auth::EndShiftArgs { counted_cash: Some(10500.0), note: None }).await.unwrap();
            let (diff,): (f64,) = sqlx::query_as("SELECT CAST(difference AS REAL) FROM cash_drawers").fetch_one(&ctx.pool).await.unwrap();
            assert_eq!(diff, -250.0);
        });
    }

    #[test]
    fn discounts_and_refunds_need_permission_or_a_manager_pin() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let owner = 1;
            let id = create_product(&ctx, product("Cola", "1", 10.0)).await.unwrap();
            // A till role that may only sell
            let role = sqlx::query("INSERT INTO roles (name, is_system) VALUES ('till', 0)").execute(&ctx.pool).await.unwrap().last_insert_rowid();
            sqlx::query("INSERT INTO role_permissions (role_id, permission_key) VALUES ($1, 'sales.sell')").bind(role).execute(&ctx.pool).await.unwrap();
            let cashier = sqlx::query("INSERT INTO users (username, password_hash, full_name, role_id, is_active) VALUES ('till', 'x', 'Till', $1, 1)")
                .bind(role)
                .execute(&ctx.pool)
                .await
                .unwrap()
                .last_insert_rowid();
            ctx.session.set(Some(crate::ctx::SessionKey { user_id: cashier }));

            let mut args = sell(id, 1.0, "cash");
            args.cart_discount = 100.0;
            let r = checkout(&ctx, args).await;
            assert!(matches!(r, Err(Error::Forbidden(p)) if p == "sales.discount"));

            let grant = crate::auth::authorize_override(&ctx, crate::auth::OverrideArgs { pin: "1234".into(), permission: "sales.discount".into() })
                .await
                .unwrap();
            let mut args = sell(id, 1.0, "cash");
            args.cart_discount = 100.0;
            args.override_token = Some(grant.token.clone());
            let sale = checkout(&ctx, args).await.unwrap();
            assert_eq!(sale.total, 650.0);
            let (approver,): (Option<i64>,) = sqlx::query_as("SELECT approved_by FROM sales WHERE id = $1").bind(sale.id).fetch_one(&ctx.pool).await.unwrap();
            assert_eq!(approver, Some(owner));

            // Single use
            let mut args = sell(id, 1.0, "cash");
            args.cart_discount = 100.0;
            args.override_token = Some(grant.token);
            assert!(checkout(&ctx, args).await.is_err());
            assert!(crate::auth::authorize_override(&ctx, crate::auth::OverrideArgs { pin: "999999".into(), permission: "sales.refund".into() })
                .await
                .is_err());
        });
    }

    #[test]
    fn exchanges_net_the_return_against_the_sale() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let cola = create_product(&ctx, product("Cola", "1", 10.0)).await.unwrap(); // 750
            let mut big = product("Juice", "2", 10.0);
            big.selling_price = 2000.0;
            let juice = create_product(&ctx, big).await.unwrap();
            let first = checkout(&ctx, sell(cola, 2.0, "cash")).await.unwrap();
            let (line,): (i64,) = sqlx::query_as("SELECT id FROM sale_items WHERE sale_id = $1").bind(first.id).fetch_one(&ctx.pool).await.unwrap();

            // Two colas (1500) back for a juice (2000): 500 to pay
            let mut args = sell(juice, 1.0, "cash");
            args.exchange = Some(ReturnItems { sale_id: Some(first.id), lines: vec![ReturnLine { sale_item_id: line, quantity: 2.0, restock: true }], ..Default::default() });
            args.tendered = Some(1000.0);
            let ex = checkout(&ctx, args).await.unwrap();
            assert_eq!((ex.total, ex.returned, ex.refunded, ex.change), (2000.0, 1500.0, 0.0, Some(500.0)));

            // A juice (2000) back without a receipt for a cola (750): 1250 paid back by card
            let mut args = sell(cola, 1.0, "card");
            args.exchange = Some(ReturnItems { products: vec![OpenReturnLine { product_id: juice, quantity: 1.0, restock: true }], ..Default::default() });
            let ex2 = checkout(&ctx, args).await.unwrap();
            assert_eq!((ex2.total, ex2.returned, ex2.refunded), (750.0, 2000.0, 1250.0));

            // Exchange credit nets to zero; only real money is left
            let by: Vec<(String, f64)> = sqlx::query_as("SELECT payment_method, CAST(SUM(amount) AS REAL) FROM payments GROUP BY payment_method ORDER BY 1")
                .fetch_all(&ctx.pool)
                .await
                .unwrap();
            assert_eq!(by, vec![("card".into(), -1250.0), ("cash".into(), 2000.0), ("exchange".into(), 0.0)]);
            let (open_return,): (Option<i64>,) = sqlx::query_as("SELECT original_sale_id FROM returns ORDER BY id DESC LIMIT 1").fetch_one(&ctx.pool).await.unwrap();
            assert_eq!(open_return, None);
            assert_eq!(one(&ctx, "SELECT stock_quantity FROM products WHERE id = $1", cola).await, 9.0);
            assert_eq!(one(&ctx, "SELECT stock_quantity FROM products WHERE id = $1", juice).await, 10.0);
        });
    }

    #[test]
    fn returns_without_a_receipt_use_todays_price() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let cola = create_product(&ctx, product("Cola", "1", 5.0)).await.unwrap();
            let r = create_return(&ctx, ReturnArgs {
                items: ReturnItems { products: vec![OpenReturnLine { product_id: cola, quantity: 2.0, restock: false }], ..Default::default() },
                refund_method: "cash".into(),
                override_token: None,
            })
            .await
            .unwrap();
            assert_eq!(r.refund, 1500.0);
            let (sale, amount): (Option<i64>, f64) = sqlx::query_as("SELECT sale_id, CAST(amount AS REAL) FROM payments WHERE return_id = $1")
                .bind(r.id)
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            assert_eq!((sale, amount), (None, -1500.0));
            assert_eq!(one(&ctx, "SELECT stock_quantity FROM products WHERE id = $1", cola).await, 5.0, "not restocked");
        });
    }

    #[test]
    fn expired_stock_can_be_blocked_at_the_till() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let milk = create_product(&ctx, product("Milk", "1", 0.0)).await.unwrap();
            for (qty, expiry) in [(2.0, "2020-01-01"), (3.0, "2099-01-01")] {
                crate::stock::receive_stock(&ctx, crate::stock::ReceiveArgs { product_id: milk, quantity: qty, expiry_date: Some(expiry.into()), cost_price: None, note: None })
                    .await
                    .unwrap();
            }
            crate::settings::set_setting(&ctx, crate::settings::SetSettingArgs { key: "block_expired".into(), value: "1".into() }).await.unwrap();
            let r = checkout(&ctx, sell(milk, 4.0, "cash")).await;
            assert!(matches!(r, Err(Error::RuleAbout("expired_stock", _))));
            checkout(&ctx, sell(milk, 3.0, "cash")).await.unwrap();
            let expired_left = one(&ctx, "SELECT quantity_remaining FROM product_batches WHERE product_id = $1 AND expiry_date = '2020-01-01'", milk).await;
            assert_eq!(expired_left, 2.0, "fresh stock was sold, the expired batch left alone");
        });
    }

    #[test]
    fn opening_cash_is_counted_once() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            use crate::shifts::{count_opening_cash, shift_summary, OpeningCountArgs};
            assert!(!shift_summary(&ctx).await.unwrap().unwrap().opening_counted);
            count_opening_cash(&ctx, OpeningCountArgs { amount: 25000.0 }).await.unwrap();
            let sum = shift_summary(&ctx).await.unwrap().unwrap();
            assert!(sum.opening_counted);
            assert_eq!(sum.expected_cash, 25000.0);
            assert!(matches!(count_opening_cash(&ctx, OpeningCountArgs { amount: 1.0 }).await, Err(Error::Rule("opening_already_counted"))));
        });
    }
}
