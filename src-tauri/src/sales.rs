//! Checkout. Prices, costs and stock come from the database, never from the
//! screen: the WebView sends only product ids, quantities and how the
//! customer paid.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::{notifications, shifts, stock};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutLine {
    pub product_id: i64,
    pub quantity: f64,
    /// Off this line, as an amount
    #[serde(default)]
    pub discount: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutArgs {
    pub lines: Vec<CheckoutLine>,
    /// Off the whole sale (after line discounts), as an amount
    #[serde(default)]
    pub cart_discount: f64,
    /// cash | card
    pub payment_method: String,
    /// Cash handed over; None means exact
    pub tendered: Option<f64>,
    /// From `authorize_override`, when the cashier can't discount alone
    pub override_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleResult {
    pub id: i64,
    pub receipt_number: String,
    pub subtotal: f64,
    pub discount: f64,
    pub tax: f64,
    pub total: f64,
    pub tendered: Option<f64>,
    pub change: Option<f64>,
}

/// Dinar amounts have no useful fraction; other currencies keep cents.
pub fn round_money(amount: f64, currency: &str) -> f64 {
    let factor = if currency == "IQD" { 1.0 } else { 100.0 };
    (amount * factor).round() / factor
}

pub async fn shop_currency(ctx: &Ctx) -> Result<String> {
    let (currency,): (String,) =
        sqlx::query_as("SELECT COALESCE((SELECT currency FROM shop_profile WHERE id = 1), 'IQD')")
            .fetch_one(&ctx.pool)
            .await?;
    Ok(currency)
}

/// Splits `amount` over `weights` in proportion, rounded, with the last
/// share taking the remainder so the parts add up exactly.
pub fn allocate(amount: f64, weights: &[f64], currency: &str) -> Vec<f64> {
    let total: f64 = weights.iter().sum();
    if amount == 0.0 || total <= 0.0 {
        return vec![0.0; weights.len()];
    }
    let mut out = Vec::with_capacity(weights.len());
    let mut given = 0.0;
    for (i, w) in weights.iter().enumerate() {
        let share = if i + 1 == weights.len() { amount - given } else { round_money(amount * w / total, currency) };
        given += share;
        out.push(share);
    }
    out
}

struct Line {
    product_id: i64,
    name: String,
    qty: f64,
    price: f64,
    cost: f64,
    discount: f64,
}

pub async fn checkout(ctx: &Ctx, args: CheckoutArgs) -> Result<SaleResult> {
    let actor = auth::require(ctx, "sales.sell").await?;
    if !matches!(args.payment_method.as_str(), "cash" | "card") {
        return Err(Error::Invalid(format!("unknown payment method {}", args.payment_method)));
    }
    // Merge repeated lines for the same product
    let mut wanted: BTreeMap<i64, (f64, f64)> = BTreeMap::new();
    for line in &args.lines {
        if !(line.quantity > 0.0) || !(line.discount >= 0.0) {
            return Err(Error::Rule("negative_amount"));
        }
        let e = wanted.entry(line.product_id).or_default();
        e.0 += line.quantity;
        e.1 += line.discount;
    }
    if wanted.is_empty() {
        return Err(Error::Rule("empty_sale"));
    }
    if !(args.cart_discount >= 0.0) || args.tendered.is_some_and(|t| !(t >= 0.0)) {
        return Err(Error::Rule("negative_amount"));
    }

    let (tax_rate,): (f64,) = sqlx::query_as(
        "SELECT COALESCE((SELECT CAST(value AS REAL) FROM settings WHERE key = 'tax_rate'), 0)",
    )
    .fetch_one(&ctx.pool)
    .await?;
    let currency = shop_currency(ctx).await?;
    let shift_id = shifts::shift_info(ctx, actor.user_id).await?.map(|s| s.id);

    let mut tx = ctx.pool.begin().await?;
    let mut lines = Vec::new();
    for (&product_id, &(qty, discount)) in &wanted {
        let row: Option<(String, f64, f64, f64, bool, String)> = sqlx::query_as(
            "SELECT name, CAST(selling_price AS REAL), CAST(cost_price AS REAL),
                    CAST(stock_quantity AS REAL), is_active, unit
             FROM products WHERE id = $1",
        )
        .bind(product_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((name, price, cost, on_hand, active, unit)) = row else {
            return Err(Error::NotFound);
        };
        if !active {
            return Err(Error::RuleAbout("product_unavailable", name));
        }
        if matches!(unit.as_str(), "piece" | "pack" | "box") && qty.fract() != 0.0 {
            return Err(Error::RuleAbout("quantity_whole", name));
        }
        if qty > on_hand {
            return Err(Error::RuleAbout("insufficient_stock", name));
        }
        let discount = round_money(discount, &currency);
        if discount > price * qty {
            return Err(Error::RuleAbout("discount_too_large", name));
        }
        lines.push(Line { product_id, name, qty, price, cost, discount });
    }

    // The cart discount is spread over the lines by what's left of each,
    // so every sale line carries its full discount (reports, returns)
    let cart_discount = round_money(args.cart_discount, &currency);
    let nets: Vec<f64> = lines.iter().map(|l| l.price * l.qty - l.discount).collect();
    if cart_discount > nets.iter().sum::<f64>() {
        return Err(Error::Rule("discount_too_large"));
    }
    for (l, share) in lines.iter_mut().zip(allocate(cart_discount, &nets, &currency)) {
        l.discount += share;
    }
    let discount: f64 = lines.iter().map(|l| l.discount).sum();
    let approved_by = if discount > 0.0 {
        auth::require_or_override(ctx, &actor, "sales.discount", args.override_token.as_deref()).await?
    } else {
        None
    };

    let subtotal = round_money(lines.iter().map(|l| l.price * l.qty).sum(), &currency);
    let tax = round_money((subtotal - discount) * tax_rate / 100.0, &currency);
    let total = subtotal - discount + tax;
    let (tendered, change) = match (args.payment_method.as_str(), args.tendered) {
        ("cash", Some(t)) if t < total => return Err(Error::Rule("tendered_short")),
        ("cash", Some(t)) => (Some(t), Some(round_money(t - total, &currency))),
        _ => (None, None),
    };
    let at = now();

    let placeholder = format!("tmp-{}", uuid::Uuid::new_v4());
    let id = sqlx::query(
        "INSERT INTO sales (receipt_number, sale_date, user_id, shift_id, subtotal, discount_amount, tax_amount, total_amount, approved_by, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $2)",
    )
    .bind(&placeholder)
    .bind(&at)
    .bind(actor.user_id)
    .bind(shift_id)
    .bind(subtotal)
    .bind(discount)
    .bind(tax)
    .bind(total)
    .bind(approved_by)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    let receipt_number = format!("R-{id:06}");
    sqlx::query("UPDATE sales SET receipt_number = $1 WHERE id = $2")
        .bind(&receipt_number)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    for l in &lines {
        // The after_sale_item_insert trigger takes the stock down and logs it
        sqlx::query(
            "INSERT INTO sale_items (sale_id, product_id, product_name, quantity, unit_price, cost_price, tax_rate, discount_amount, subtotal, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(id)
        .bind(l.product_id)
        .bind(&l.name)
        .bind(l.qty)
        .bind(l.price)
        .bind(l.cost)
        .bind(tax_rate)
        .bind(l.discount)
        .bind(l.price * l.qty - l.discount)
        .bind(&at)
        .execute(&mut *tx)
        .await?;
        stock::consume_batches(&mut tx, l.product_id, l.qty).await?;
    }
    sqlx::query(
        "INSERT INTO payments (sale_id, payment_method, amount, tendered, change_given, shift_id, payment_date)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(&args.payment_method)
    .bind(total)
    .bind(tendered)
    .bind(change)
    .bind(shift_id)
    .bind(&at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    audit::log(
        ctx,
        Some(actor.user_id),
        "sale",
        "sales",
        Some(id),
        &serde_json::json!({ "total": total, "items": lines.len(), "discount": discount, "approvedBy": approved_by }),
    )
    .await?;
    notifications::refresh_quietly(ctx).await;
    Ok(SaleResult { id, receipt_number, subtotal, discount, tax, total, tendered, change })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoidLineArgs {
    /// What came off the cart: product id and quantity
    pub lines: Vec<CheckoutLine>,
    pub override_token: Option<String>,
}

/// Taking a scanned item back off the cart. Nothing is stored yet, so this
/// only checks the permission (or a manager's approval) and leaves a trail.
pub async fn void_line(ctx: &Ctx, args: VoidLineArgs) -> Result<()> {
    let actor = auth::require(ctx, "sales.sell").await?;
    let approved_by = auth::require_or_override(ctx, &actor, "sales.void_line", args.override_token.as_deref()).await?;
    let lines: Vec<_> = args.lines.iter().map(|l| serde_json::json!({ "product": l.product_id, "quantity": l.quantity })).collect();
    audit::log(ctx, Some(actor.user_id), "sale.void_line", "products", None, &serde_json::json!({ "lines": lines, "approvedBy": approved_by })).await
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::catalog::{create_product, ProductInput};
    use crate::ctx::test_support;
    use crate::stock::{receive_stock, remove_stock, ReceiveArgs, RemoveArgs};

    pub fn product(name: &str, barcode: &str, stock: f64) -> ProductInput {
        ProductInput {
            name: name.into(),
            sku: None,
            category_id: None,
            barcode: Some(barcode.into()),
            extra_barcodes: vec![],
            cost_price: 500.0,
            selling_price: 750.0,
            min_stock: 2.0,
            unit: "piece".into(),
            icon_key: None,
            media_ids: vec![],
            is_active: true,
            initial_stock: Some(stock),
            initial_expiry: None,
        }
    }

    pub fn sell(product_id: i64, quantity: f64, method: &str) -> CheckoutArgs {
        CheckoutArgs {
            lines: vec![CheckoutLine { product_id, quantity, discount: 0.0 }],
            cart_discount: 0.0,
            payment_method: method.into(),
            tendered: None,
            override_token: None,
        }
    }

    async fn stock_of(ctx: &Ctx, id: i64) -> f64 {
        sqlx::query_as::<_, (f64,)>("SELECT CAST(stock_quantity AS REAL) FROM products WHERE id = $1")
            .bind(id)
            .fetch_one(&ctx.pool)
            .await
            .unwrap()
            .0
    }

    #[test]
    fn checkout_uses_database_prices_and_takes_stock() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            crate::settings::set_setting(&ctx, crate::settings::SetSettingArgs { key: "tax_rate".into(), value: "10".into() })
                .await
                .unwrap();
            let id = create_product(&ctx, product("Cola", "1", 10.0)).await.unwrap();
            let sale = checkout(&ctx, CheckoutArgs {
                lines: vec![CheckoutLine { product_id: id, quantity: 2.0, discount: 0.0 }, CheckoutLine { product_id: id, quantity: 1.0, discount: 0.0 }],
                cart_discount: 0.0,
                payment_method: "cash".into(),
                tendered: Some(3000.0),
                override_token: None,
            })
            .await
            .unwrap();
            assert_eq!(sale.receipt_number, format!("R-{:06}", sale.id));
            assert_eq!((sale.subtotal, sale.tax, sale.total), (2250.0, 225.0, 2475.0));
            assert_eq!(sale.change, Some(525.0));
            assert_eq!(stock_of(&ctx, id).await, 7.0);

            let (items, cost, shift): (i64, f64, Option<i64>) = sqlx::query_as(
                "SELECT COUNT(*), SUM(si.cost_price * si.quantity), s.shift_id
                 FROM sale_items si JOIN sales s ON s.id = si.sale_id WHERE s.id = $1",
            )
            .bind(sale.id)
            .fetch_one(&ctx.pool)
            .await
            .unwrap();
            assert_eq!((items, cost), (1, 1500.0), "repeated lines merge; cost is snapshotted");
            assert!(shift.is_some(), "sale is tied to the open shift");
        });
    }

    #[test]
    fn checkout_refuses_more_than_on_hand_and_fractions_of_pieces() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let id = create_product(&ctx, product("Cola", "1", 2.0)).await.unwrap();
            let r = checkout(&ctx, sell(id, 3.0, "card")).await;
            assert!(matches!(r, Err(Error::RuleAbout("insufficient_stock", n)) if n == "Cola"));
            let r = checkout(&ctx, sell(id, 0.5, "card")).await;
            assert!(matches!(r, Err(Error::RuleAbout("quantity_whole", _))));
            assert_eq!(stock_of(&ctx, id).await, 2.0, "nothing changed");
        });
    }

    #[test]
    fn batches_are_consumed_soonest_expiry_first() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let id = create_product(&ctx, product("Milk", "1", 0.0)).await.unwrap();
            for (qty, expiry) in [(5.0, "2026-12-20"), (5.0, "2026-12-10")] {
                receive_stock(&ctx, ReceiveArgs { product_id: id, quantity: qty, expiry_date: Some(expiry.into()), cost_price: None, note: None })
                    .await
                    .unwrap();
            }
            checkout(&ctx, sell(id, 7.0, "cash"))
                .await
                .unwrap();
            let left: Vec<(String, f64)> = sqlx::query_as(
                "SELECT expiry_date, quantity_remaining FROM product_batches WHERE product_id = $1 ORDER BY expiry_date",
            )
            .bind(id)
            .fetch_all(&ctx.pool)
            .await
            .unwrap();
            assert_eq!(left, vec![("2026-12-10".into(), 0.0), ("2026-12-20".into(), 3.0)]);

            remove_stock(&ctx, RemoveArgs { product_id: id, quantity: 1.0, reason: "damaged".into(), batch_id: None, note: None })
                .await
                .unwrap();
            assert_eq!(stock_of(&ctx, id).await, 2.0);
            let r = remove_stock(&ctx, RemoveArgs { product_id: id, quantity: 5.0, reason: "damaged".into(), batch_id: None, note: None }).await;
            assert!(matches!(r, Err(Error::RuleAbout("insufficient_stock", _))));
        });
    }
}
