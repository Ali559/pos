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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutArgs {
    pub lines: Vec<CheckoutLine>,
    /// cash | card
    pub payment_method: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleResult {
    pub id: i64,
    pub receipt_number: String,
    pub subtotal: f64,
    pub tax: f64,
    pub total: f64,
}

/// Dinar amounts have no useful fraction; other currencies keep cents.
fn round_money(amount: f64, currency: &str) -> f64 {
    let factor = if currency == "IQD" { 1.0 } else { 100.0 };
    (amount * factor).round() / factor
}

pub async fn checkout(ctx: &Ctx, args: CheckoutArgs) -> Result<SaleResult> {
    let actor = auth::require(ctx, "sales.sell").await?;
    if !matches!(args.payment_method.as_str(), "cash" | "card") {
        return Err(Error::Invalid(format!("unknown payment method {}", args.payment_method)));
    }
    // Merge repeated lines for the same product
    let mut wanted: BTreeMap<i64, f64> = BTreeMap::new();
    for line in &args.lines {
        if !(line.quantity > 0.0) {
            return Err(Error::Rule("negative_amount"));
        }
        *wanted.entry(line.product_id).or_default() += line.quantity;
    }
    if wanted.is_empty() {
        return Err(Error::Rule("empty_sale"));
    }

    let (tax_rate,): (f64,) = sqlx::query_as(
        "SELECT COALESCE((SELECT CAST(value AS REAL) FROM settings WHERE key = 'tax_rate'), 0)",
    )
    .fetch_one(&ctx.pool)
    .await?;
    let (currency,): (String,) =
        sqlx::query_as("SELECT COALESCE((SELECT currency FROM shop_profile WHERE id = 1), 'IQD')")
            .fetch_one(&ctx.pool)
            .await?;
    let shift_id = shifts::shift_info(ctx, actor.user_id).await?.map(|s| s.id);

    let mut tx = ctx.pool.begin().await?;
    let mut lines = Vec::new();
    let mut subtotal = 0.0;
    for (&product_id, &qty) in &wanted {
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
        subtotal += price * qty;
        lines.push((product_id, name, qty, price, cost));
    }
    let subtotal = round_money(subtotal, &currency);
    let tax = round_money(subtotal * tax_rate / 100.0, &currency);
    let total = subtotal + tax;
    let at = now();

    let placeholder = format!("tmp-{}", uuid::Uuid::new_v4());
    let id = sqlx::query(
        "INSERT INTO sales (receipt_number, sale_date, user_id, shift_id, subtotal, tax_amount, total_amount, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $2)",
    )
    .bind(&placeholder)
    .bind(&at)
    .bind(actor.user_id)
    .bind(shift_id)
    .bind(subtotal)
    .bind(tax)
    .bind(total)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    let receipt_number = format!("R-{id:06}");
    sqlx::query("UPDATE sales SET receipt_number = $1 WHERE id = $2")
        .bind(&receipt_number)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    for (product_id, name, qty, price, cost) in &lines {
        // The after_sale_item_insert trigger takes the stock down and logs it
        sqlx::query(
            "INSERT INTO sale_items (sale_id, product_id, product_name, quantity, unit_price, cost_price, tax_rate, subtotal, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(product_id)
        .bind(name)
        .bind(qty)
        .bind(price)
        .bind(cost)
        .bind(tax_rate)
        .bind(price * qty)
        .bind(&at)
        .execute(&mut *tx)
        .await?;
        stock::consume_batches(&mut tx, *product_id, *qty).await?;
    }
    sqlx::query("INSERT INTO payments (sale_id, payment_method, amount, payment_date) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(&args.payment_method)
        .bind(total)
        .bind(&at)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    audit::log(ctx, Some(actor.user_id), "sale", "sales", Some(id), &serde_json::json!({ "total": total, "items": lines.len() })).await?;
    notifications::refresh_quietly(ctx).await;
    Ok(SaleResult {
        id,
        receipt_number,
        subtotal,
        tax,
        total,
    })
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
            name_ku: None,
            name_ar: None,
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
                lines: vec![CheckoutLine { product_id: id, quantity: 2.0 }, CheckoutLine { product_id: id, quantity: 1.0 }],
                payment_method: "cash".into(),
            })
            .await
            .unwrap();
            assert_eq!(sale.receipt_number, format!("R-{:06}", sale.id));
            assert_eq!((sale.subtotal, sale.tax, sale.total), (2250.0, 225.0, 2475.0));
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
            let r = checkout(&ctx, CheckoutArgs { lines: vec![CheckoutLine { product_id: id, quantity: 3.0 }], payment_method: "card".into() }).await;
            assert!(matches!(r, Err(Error::RuleAbout("insufficient_stock", n)) if n == "Cola"));
            let r = checkout(&ctx, CheckoutArgs { lines: vec![CheckoutLine { product_id: id, quantity: 0.5 }], payment_method: "card".into() }).await;
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
            checkout(&ctx, CheckoutArgs { lines: vec![CheckoutLine { product_id: id, quantity: 7.0 }], payment_method: "cash".into() })
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
