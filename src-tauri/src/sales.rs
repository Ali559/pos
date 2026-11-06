//! Checkout. Prices, costs and stock come from the database, never from the
//! screen: the WebView sends only product ids, quantities and how the
//! customer paid.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::{notifications, returns, shifts, stock};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutLine {
    pub product_id: i64,
    /// In the pack (unit_id) or the product's own unit. Ignored for scale
    /// labels, which carry their own quantity.
    pub quantity: f64,
    /// Off this line, as an amount
    #[serde(default)]
    pub discount: f64,
    /// Sold by the pack: a row of product_units
    #[serde(default)]
    pub unit_id: Option<i64>,
    /// A scale label; decoded here again for the weight or price
    #[serde(default)]
    pub scale_barcode: Option<String>,
    /// Price per unit typed at the till (open-price products only)
    #[serde(default)]
    pub open_price: Option<f64>,
    /// What an open-price line is ("batteries"); shown on the receipt
    #[serde(default)]
    pub note: Option<String>,
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
    /// Items coming back in the same transaction (an exchange): their value
    /// pays for the sale first
    #[serde(default)]
    pub exchange: Option<returns::ReturnItems>,
    /// Manager approval for the exchange's refund
    #[serde(default)]
    pub refund_override_token: Option<String>,
    /// Manager approval for typed prices
    #[serde(default)]
    pub open_price_override_token: Option<String>,
    /// Who's buying (optional; needed to leave anything on account)
    #[serde(default)]
    pub customer_id: Option<i64>,
    /// Left on the customer's account (قەرز); the rest is paid with
    /// `payment_method`
    #[serde(default)]
    pub on_account: f64,
    /// Manager approval for going over the customer's credit limit
    #[serde(default)]
    pub over_limit_override_token: Option<String>,
    /// retail (default) | wholesale
    #[serde(default)]
    pub price_level: Option<String>,
    /// Manager approval for a wholesale sale
    #[serde(default)]
    pub wholesale_override_token: Option<String>,
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
    /// Exchange: value of what came back, and what was paid back on top
    pub returned: f64,
    pub refunded: f64,
    /// Left on the customer's account
    pub on_account: f64,
    /// Taken off by promotions (part of `discount`)
    pub promotions: f64,
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
    /// In the product's own unit (what stock counts)
    qty: f64,
    /// What the line costs before discounts
    gross: f64,
    cost: f64,
    discount: f64,
    /// Sold by the pack: its name and size, as a snapshot
    unit: Option<(String, f64)>,
    tracked: bool,
    /// Taken off by a promotion (kept apart from `discount`, the manual one)
    promo: f64,
    promotion: Option<(i64, String)>,
    tax_exempt: bool,
}

pub async fn checkout(ctx: &Ctx, args: CheckoutArgs) -> Result<SaleResult> {
    let actor = auth::require(ctx, "sales.sell").await?;
    if !matches!(args.payment_method.as_str(), "cash" | "card") {
        return Err(Error::Invalid(format!("unknown payment method {}", args.payment_method)));
    }
    if args.lines.is_empty() {
        return Err(Error::Rule("empty_sale"));
    }
    for line in &args.lines {
        if !(line.quantity > 0.0) || !(line.discount >= 0.0) || line.open_price.is_some_and(|p| !(p >= 0.0)) {
            return Err(Error::Rule("negative_amount"));
        }
    }
    if !(args.cart_discount >= 0.0) || args.tendered.is_some_and(|t| !(t >= 0.0)) || !(args.on_account >= 0.0) {
        return Err(Error::Rule("negative_amount"));
    }
    if args.on_account > 0.0 && args.customer_id.is_none() {
        return Err(Error::Rule("customer_required"));
    }

    let (tax_rate,): (f64,) = sqlx::query_as(
        "SELECT COALESCE((SELECT CAST(value AS REAL) FROM settings WHERE key = 'tax_rate'), 0)",
    )
    .fetch_one(&ctx.pool)
    .await?;
    let currency = shop_currency(ctx).await?;
    let shift_id = shifts::shift_info(ctx, actor.user_id).await?.map(|s| s.id);
    let today = stock::today();
    let block_expired = crate::settings::get(ctx, "block_expired", "0").await? == "1";
    let scale_cfg = crate::scale::config(ctx).await?;

    // Repeated plain lines (same product, same pack) become one sale line;
    // scale labels and typed prices stay apart
    let mut merged: Vec<CheckoutLine> = Vec::new();
    for line in &args.lines {
        let plain = line.scale_barcode.is_none() && line.open_price.is_none();
        match merged.iter_mut().find(|m| {
            plain && m.scale_barcode.is_none() && m.open_price.is_none() && m.product_id == line.product_id && m.unit_id == line.unit_id
        }) {
            Some(m) => {
                m.quantity += line.quantity;
                m.discount += line.discount;
            }
            None => merged.push(CheckoutLine {
                product_id: line.product_id,
                quantity: line.quantity,
                discount: line.discount,
                unit_id: line.unit_id,
                scale_barcode: line.scale_barcode.clone(),
                open_price: line.open_price,
                note: line.note.clone(),
            }),
        }
    }

    let level = args.price_level.as_deref().unwrap_or("retail");
    if !matches!(level, "retail" | "wholesale") {
        return Err(Error::Invalid(format!("unknown price level {level}")));
    }
    let wholesale_by = if level == "wholesale" {
        auth::require_or_override(ctx, &actor, "sales.wholesale", args.wholesale_override_token.as_deref()).await?
    } else {
        None
    };

    let mut tx = ctx.pool.begin().await?;
    let mut lines = Vec::new();
    let mut typed_price = false;
    for line in &merged {
        #[allow(clippy::type_complexity)]
        let row: Option<(String, f64, f64, bool, String, bool, bool, Option<String>, Option<f64>, bool)> = sqlx::query_as(
            "SELECT name, CAST(selling_price AS REAL), CAST(cost_price AS REAL), is_active, unit,
                    open_price, track_stock, plu, CAST(wholesale_price AS REAL), tax_exempt
             FROM products WHERE id = $1",
        )
        .bind(line.product_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((name, price, cost, active, unit, open_price, tracked, plu, wholesale, tax_exempt)) = row else {
            return Err(Error::NotFound);
        };
        let mut promo = 0.0;
        let mut promotion = None;
        if !active {
            return Err(Error::RuleAbout("product_unavailable", name));
        }
        let whole = crate::catalog::is_whole_unit(&unit);
        let (qty, gross, pack, name) = if let Some(unit_id) = line.unit_id {
            let pack: Option<(String, f64, f64)> = sqlx::query_as(
                "SELECT name, CAST(factor AS REAL), CAST(price AS REAL) FROM product_units WHERE id = $1 AND product_id = $2",
            )
            .bind(unit_id)
            .bind(line.product_id)
            .fetch_optional(&mut *tx)
            .await?;
            let Some((pack_name, factor, pack_price)) = pack else {
                return Err(Error::NotFound);
            };
            if line.quantity.fract() != 0.0 {
                return Err(Error::RuleAbout("quantity_whole", name));
            }
            (line.quantity * factor, pack_price * line.quantity, Some((pack_name, factor)), name)
        } else if let Some(label) = line.scale_barcode.as_deref() {
            let decoded = scale_cfg.as_ref().and_then(|cfg| crate::scale::decode(cfg, label).map(|d| (cfg, d)));
            let Some((cfg, (code, value))) = decoded else {
                return Err(Error::Rule("scale_label_invalid"));
            };
            if plu.as_deref() != Some(code.as_str()) {
                return Err(Error::Rule("scale_label_invalid"));
            }
            let (qty, amount) = crate::scale::line_from_label(cfg, value, price);
            let gross = amount.unwrap_or_else(|| round_money(price * qty, &currency));
            (qty, gross, None, name)
        } else if let Some(typed) = line.open_price {
            if !open_price {
                return Err(Error::RuleAbout("price_not_open", name));
            }
            typed_price = true;
            let note = line.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
            let name = note.map(|n| format!("{name}: {n}")).unwrap_or(name);
            (line.quantity, round_money(typed * line.quantity, &currency), None, name)
        } else {
            if open_price {
                return Err(Error::RuleAbout("price_required", name));
            }
            // Wholesale, or retail with a cheaper price from a quantity
            let each = crate::pricing::unit_price(&mut tx, line.product_id, price, wholesale, level, line.quantity).await?;
            let gross = if whole { each * line.quantity } else { round_money(each * line.quantity, &currency) };
            // Promotions are for retail sales of single items
            if level == "retail" {
                let promos = crate::pricing::promotions_for(&mut tx, line.product_id, &today).await?;
                if let Some((p, off)) = crate::pricing::best_promotion(&promos, line.quantity, each, &currency) {
                    promo = off.min(gross);
                    promotion = Some((p.id, p.name));
                }
            }
            (line.quantity, gross, None, name)
        };
        if !(qty > 0.0) {
            return Err(Error::Rule("negative_amount"));
        }
        if whole && qty.fract() != 0.0 {
            return Err(Error::RuleAbout("quantity_whole", name));
        }
        let discount = round_money(line.discount, &currency);
        if discount + promo > gross + 1e-9 {
            return Err(Error::RuleAbout("discount_too_large", name));
        }
        lines.push(Line { product_id: line.product_id, name, qty, gross, cost, discount, unit: pack, tracked, promo, promotion, tax_exempt });
    }
    if typed_price {
        auth::require_or_override(ctx, &actor, "sales.open_price", args.open_price_override_token.as_deref()).await?;
    }

    // Stock, per product: a single and a carton of the same thing add up
    let mut need: BTreeMap<i64, (f64, String)> = BTreeMap::new();
    for l in lines.iter().filter(|l| l.tracked) {
        let e = need.entry(l.product_id).or_insert((0.0, l.name.clone()));
        e.0 += l.qty;
    }
    for (&product_id, (qty, name)) in &need {
        let (on_hand,): (f64,) = sqlx::query_as("SELECT CAST(stock_quantity AS REAL) FROM products WHERE id = $1")
            .bind(product_id)
            .fetch_one(&mut *tx)
            .await?;
        if *qty > on_hand + 1e-9 {
            return Err(Error::RuleAbout("insufficient_stock", name.clone()));
        }
        if block_expired && *qty > on_hand - stock::expired_quantity(&mut tx, product_id, &today).await? + 1e-9 {
            return Err(Error::RuleAbout("expired_stock", name.clone()));
        }
    }

    // The cart discount is spread over the lines by what's left of each,
    // so every sale line carries its full discount (reports, returns)
    let cart_discount = round_money(args.cart_discount, &currency);
    let nets: Vec<f64> = lines.iter().map(|l| l.gross - l.discount - l.promo).collect();
    if cart_discount > nets.iter().sum::<f64>() {
        return Err(Error::Rule("discount_too_large"));
    }
    for (l, share) in lines.iter_mut().zip(allocate(cart_discount, &nets, &currency)) {
        l.discount += share;
    }
    // Only discounts someone gave need approval; promotions don't
    let manual: f64 = lines.iter().map(|l| l.discount).sum();
    let promotions: f64 = lines.iter().map(|l| l.promo).sum();
    let discount = manual + promotions;
    let approved_by = if manual > 0.0 {
        auth::require_or_override(ctx, &actor, "sales.discount", args.override_token.as_deref()).await?
    } else {
        None
    }
    .or(wholesale_by);

    let subtotal = round_money(lines.iter().map(|l| l.gross).sum(), &currency);
    // Tax-free products aren't taxed
    let taxable: f64 = lines.iter().filter(|l| !l.tax_exempt).map(|l| l.gross - l.discount - l.promo).sum();
    let tax = round_money(taxable * tax_rate / 100.0, &currency);
    let total = subtotal - discount + tax;

    // An exchange: what comes back pays for the sale first; any surplus is
    // paid back with the chosen method, any shortfall is paid with it
    let exchange = match &args.exchange {
        Some(items) => Some(returns::prepare(ctx, &mut tx, &actor, items, args.refund_override_token.as_deref(), &currency).await?),
        None => None,
    };
    let returned = exchange.as_ref().map_or(0.0, |e| e.total);
    let credit = returned.min(total);
    // What's left after the exchange: part on account, the rest paid now
    let on_account = round_money(args.on_account, &currency);
    if on_account > total - credit + 1e-9 {
        return Err(Error::Rule("on_account_too_large"));
    }
    if let Some(customer) = args.customer_id {
        if on_account > 0.0 {
            crate::customers::check_on_account(ctx, &mut tx, &actor, customer, on_account, args.over_limit_override_token.as_deref()).await?;
        } else {
            let found: Option<(i64,)> = sqlx::query_as("SELECT id FROM customers WHERE id = $1").bind(customer).fetch_optional(&mut *tx).await?;
            found.ok_or(Error::NotFound)?;
        }
    }
    let due = total - credit - on_account;
    let (tendered, change) = match (args.payment_method.as_str(), args.tendered) {
        ("cash", Some(t)) if due > 0.0 && t < due => return Err(Error::Rule("tendered_short")),
        ("cash", Some(t)) if due > 0.0 => (Some(t), Some(round_money(t - due, &currency))),
        _ => (None, None),
    };
    let at = now();

    let placeholder = format!("tmp-{}", uuid::Uuid::new_v4());
    let id = sqlx::query(
        "INSERT INTO sales (receipt_number, sale_date, user_id, shift_id, subtotal, discount_amount, tax_amount, total_amount, approved_by, customer_id, price_level, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $2)",
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
    .bind(args.customer_id)
    .bind(level)
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
            "INSERT INTO sale_items (sale_id, product_id, product_name, quantity, unit_price, cost_price, tax_rate, discount_amount, subtotal,
                unit_name, unit_factor, promotion_id, promotion_name, promo_discount, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        )
        .bind(id)
        .bind(l.product_id)
        .bind(&l.name)
        .bind(l.qty)
        .bind(l.gross / l.qty)
        .bind(l.cost)
        .bind(if l.tax_exempt { 0.0 } else { tax_rate })
        .bind(l.discount + l.promo)
        .bind(l.gross - l.discount - l.promo)
        .bind(l.unit.as_ref().map(|u| u.0.as_str()))
        .bind(l.unit.as_ref().map(|u| u.1))
        .bind(l.promotion.as_ref().map(|p| p.0))
        .bind(l.promotion.as_ref().map(|p| p.1.as_str()))
        .bind(l.promo)
        .bind(&at)
        .execute(&mut *tx)
        .await?;
        if l.tracked {
            stock::consume_batches(&mut tx, l.product_id, l.qty, block_expired.then_some(today.as_str())).await?;
        }
    }
    if on_account > 0.0 {
        let customer_id = args.customer_id.ok_or(Error::Rule("customer_required"))?;
        sqlx::query("INSERT INTO payments (sale_id, payment_method, amount, shift_id, payment_date) VALUES ($1, 'credit', $2, $3, $4)")
            .bind(id)
            .bind(on_account)
            .bind(shift_id)
            .bind(&at)
            .execute(&mut *tx)
            .await?;
        crate::customers::write_entry(&mut tx, crate::customers::Entry {
            customer_id,
            kind: "sale",
            amount: on_account,
            sale_id: Some(id),
            return_id: None,
            method: None,
            note: None,
            user_id: actor.user_id,
            shift_id,
            at: &at,
        })
        .await?;
    }
    if due > 0.0 || (exchange.is_none() && on_account == 0.0) {
        sqlx::query(
            "INSERT INTO payments (sale_id, payment_method, amount, tendered, change_given, shift_id, payment_date)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(&args.payment_method)
        .bind(due)
        .bind(tendered)
        .bind(change)
        .bind(shift_id)
        .bind(&at)
        .execute(&mut *tx)
        .await?;
    }
    let mut refunded = 0.0;
    if let Some(pending) = &exchange {
        let return_id = returns::write(&mut tx, pending, actor.user_id, shift_id, "exchange", &at).await?;
        // The credit moves from the return to the sale; nothing crosses the drawer
        sqlx::query(
            "INSERT INTO payments (sale_id, payment_method, amount, shift_id, return_id, payment_date)
             VALUES ($1, 'exchange', $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(credit)
        .bind(shift_id)
        .bind(return_id)
        .bind(&at)
        .execute(&mut *tx)
        .await?;
        let original = items_sale(&args);
        returns::pay_out(&mut tx, original, return_id, "exchange", credit, shift_id, &at).await?;
        refunded = returned - credit;
        returns::pay_out(&mut tx, original, return_id, &args.payment_method, refunded, shift_id, &at).await?;
    }
    tx.commit().await?;

    audit::log(
        ctx,
        Some(actor.user_id),
        "sale",
        "sales",
        Some(id),
        &serde_json::json!({ "total": total, "items": lines.len(), "discount": discount, "approvedBy": approved_by, "customer": args.customer_id, "onAccount": on_account }),
    )
    .await?;
    notifications::refresh_quietly(ctx).await;
    Ok(SaleResult { id, receipt_number, subtotal, discount, tax, total, tendered, change, returned, refunded, on_account, promotions })
}

fn items_sale(args: &CheckoutArgs) -> Option<i64> {
    args.exchange.as_ref().and_then(|e| e.sale_id)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldArgs {
    /// A name to find it by ("man in the blue coat"); optional
    pub label: Option<String>,
    /// The cart as the register keeps it. Prices are looked up again when
    /// it's charged, so this is only what was on it.
    pub cart: serde_json::Value,
}

/// Parks the current cart so the till can serve someone else.
pub async fn hold_sale(ctx: &Ctx, args: HoldArgs) -> Result<i64> {
    let actor = auth::require(ctx, "sales.sell").await?;
    if !args.cart.is_object() && !args.cart.is_array() {
        return Err(Error::Invalid("cart must be JSON".into()));
    }
    let label = args.label.as_deref().map(str::trim).filter(|s| !s.is_empty());
    Ok(sqlx::query("INSERT INTO held_sales (user_id, customer_name, sale_data, created_at) VALUES ($1, $2, $3, $4)")
        .bind(actor.user_id)
        .bind(label)
        .bind(args.cart.to_string())
        .bind(now())
        .execute(&ctx.pool)
        .await?
        .last_insert_rowid())
}

#[derive(Debug, Deserialize)]
pub struct HeldIdArgs {
    pub id: i64,
}

/// Takes a parked cart off the list and returns it (resume), or just drops it.
pub async fn take_held_sale(ctx: &Ctx, args: HeldIdArgs) -> Result<serde_json::Value> {
    auth::require(ctx, "sales.sell").await?;
    let row: Option<(String,)> = sqlx::query_as("DELETE FROM held_sales WHERE id = $1 RETURNING sale_data")
        .bind(args.id)
        .fetch_optional(&ctx.pool)
        .await?;
    let (data,) = row.ok_or(Error::NotFound)?;
    Ok(serde_json::from_str(&data)?)
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
            track_stock: true,
            ..Default::default()
        }
    }

    pub fn sell(product_id: i64, quantity: f64, method: &str) -> CheckoutArgs {
        CheckoutArgs {
            lines: vec![CheckoutLine { product_id, quantity, discount: 0.0, ..Default::default() }],
            cart_discount: 0.0,
            payment_method: method.into(),
            tendered: None,
            override_token: None,
            exchange: None,
            refund_override_token: None,
            open_price_override_token: None,
            customer_id: None,
            on_account: 0.0,
            over_limit_override_token: None,
            price_level: None,
            wholesale_override_token: None,
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

    fn ean13(first12: &str) -> String {
        let sum: u32 = first12.chars().enumerate().map(|(i, c)| c.to_digit(10).unwrap() * if i % 2 == 0 { 1 } else { 3 }).sum();
        format!("{first12}{}", (10 - sum % 10) % 10)
    }

    fn cart(lines: Vec<CheckoutLine>) -> CheckoutArgs {
        CheckoutArgs { lines, ..sell(0, 1.0, "card") }
    }

    #[test]
    fn packs_scale_labels_and_typed_prices() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;

            // A carton of 24 has its own barcode and price; stock counts cans
            let mut cola = product("Cola", "111", 30.0);
            cola.units = vec![crate::catalog::UnitInput { name: "Carton".into(), factor: 24.0, barcode: Some("C24".into()), price: 15000.0, ..Default::default() }];
            let cola = create_product(&ctx, cola).await.unwrap();
            let scan = crate::scale::resolve_scan(&ctx, crate::scale::ScanArgs { code: "C24".into() }).await.unwrap().unwrap();
            assert_eq!((scan.product_id, scan.quantity), (cola, 1.0));
            let carton = scan.unit_id.unwrap();
            let sale = checkout(&ctx, cart(vec![
                CheckoutLine { product_id: cola, quantity: 1.0, unit_id: Some(carton), ..Default::default() },
                CheckoutLine { product_id: cola, quantity: 2.0, ..Default::default() },
            ]))
            .await
            .unwrap();
            assert_eq!(sale.total, 15000.0 + 1500.0);
            assert_eq!(stock_of(&ctx, cola).await, 4.0);
            let (name, factor, qty, sub): (Option<String>, Option<f64>, f64, f64) = sqlx::query_as(
                "SELECT unit_name, unit_factor, quantity, subtotal FROM sale_items WHERE sale_id = $1 AND unit_name IS NOT NULL",
            )
            .bind(sale.id)
            .fetch_one(&ctx.pool)
            .await
            .unwrap();
            assert_eq!((name.as_deref(), factor, qty, sub), (Some("Carton"), Some(24.0), 24.0, 15000.0));
            // Singles and cartons share the stock: 4 cans can't make a carton
            let r = checkout(&ctx, cart(vec![CheckoutLine { product_id: cola, quantity: 1.0, unit_id: Some(carton), ..Default::default() }])).await;
            assert!(matches!(r, Err(Error::RuleAbout("insufficient_stock", _))));
            // A pack's barcode can't be used by another product
            let r = create_product(&ctx, product("Fake", "C24", 1.0)).await;
            assert!(matches!(r, Err(Error::Rule("barcode_taken"))));

            // Weighed: a scale label (20 + PLU 00042 + 01250 g) sells 1.25 kg
            let mut cheese = product("Cheese", "222", 5.0);
            cheese.unit = "kg".into();
            cheese.selling_price = 12000.0;
            cheese.plu = Some("00042".into());
            let cheese = create_product(&ctx, cheese).await.unwrap();
            let label = ean13("200004201250");
            let scan = crate::scale::resolve_scan(&ctx, crate::scale::ScanArgs { code: label.clone() }).await.unwrap().unwrap();
            assert_eq!((scan.product_id, scan.quantity, scan.amount), (cheese, 1.25, None));
            let sale = checkout(&ctx, cart(vec![CheckoutLine { product_id: cheese, quantity: 99.0, scale_barcode: Some(label), ..Default::default() }]))
                .await
                .unwrap();
            assert_eq!(sale.total, 15000.0, "the label's weight, not the screen's quantity");
            assert_eq!(stock_of(&ctx, cheese).await, 3.75);
            let other = ean13("200004301250");
            let r = checkout(&ctx, cart(vec![CheckoutLine { product_id: cheese, quantity: 1.0, scale_barcode: Some(other), ..Default::default() }])).await;
            assert!(matches!(r, Err(Error::Rule("scale_label_invalid"))));

            // Price labels charge exactly the printed price
            for (k, v) in [("scale_mode", "price"), ("scale_decimals", "0")] {
                crate::settings::set_setting(&ctx, crate::settings::SetSettingArgs { key: k.into(), value: v.into() }).await.unwrap();
            }
            let label = ean13("200004209000");
            let sale = checkout(&ctx, cart(vec![CheckoutLine { product_id: cheese, quantity: 1.0, scale_barcode: Some(label), ..Default::default() }]))
                .await
                .unwrap();
            assert_eq!(sale.total, 9000.0);
            assert_eq!(stock_of(&ctx, cheese).await, 3.0);

            // The misc item: price typed at the till, no stock counted
            crate::catalog::ensure_misc_product(&ctx).await.unwrap();
            crate::catalog::ensure_misc_product(&ctx).await.unwrap();
            let (misc,): (i64,) = sqlx::query_as("SELECT id FROM products WHERE sku = 'MISC'").fetch_one(&ctx.pool).await.unwrap();
            let r = checkout(&ctx, cart(vec![CheckoutLine { product_id: misc, quantity: 1.0, ..Default::default() }])).await;
            assert!(matches!(r, Err(Error::RuleAbout("price_required", _))));
            let sale = checkout(&ctx, cart(vec![CheckoutLine {
                product_id: misc,
                quantity: 2.0,
                open_price: Some(2500.0),
                note: Some("batteries".into()),
                ..Default::default()
            }]))
            .await
            .unwrap();
            assert_eq!(sale.total, 5000.0);
            assert_eq!(stock_of(&ctx, misc).await, 0.0);
            let (name,): (String,) = sqlx::query_as("SELECT product_name FROM sale_items WHERE sale_id = $1").bind(sale.id).fetch_one(&ctx.pool).await.unwrap();
            assert!(name.ends_with(": batteries"));
            // Typed prices only for open-price products
            let r = checkout(&ctx, cart(vec![CheckoutLine { product_id: cola, quantity: 1.0, open_price: Some(1.0), ..Default::default() }])).await;
            assert!(matches!(r, Err(Error::RuleAbout("price_not_open", _))));
        });
    }

    #[test]
    fn wholesale_quantity_prices_promotions_and_tax_free() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            crate::settings::set_setting(&ctx, crate::settings::SetSettingArgs { key: "tax_rate".into(), value: "10".into() }).await.unwrap();
            // Cola: 750 retail, 600 wholesale, 700 each from 6
            let mut cola = product("Cola", "1", 50.0);
            cola.wholesale_price = Some(600.0);
            cola.qty_prices = vec![crate::catalog::QtyPrice { min_qty: 6.0, price: 700.0 }];
            let cola = create_product(&ctx, cola).await.unwrap();
            // Bread is tax-free
            let mut bread = product("Bread", "2", 50.0);
            bread.selling_price = 1000.0;
            bread.tax_exempt = true;
            let bread = create_product(&ctx, bread).await.unwrap();

            // 6 cola at the quantity price + 1 bread: tax only on the cola
            let sale = checkout(&ctx, cart(vec![
                CheckoutLine { product_id: cola, quantity: 6.0, ..Default::default() },
                CheckoutLine { product_id: bread, quantity: 1.0, ..Default::default() },
            ]))
            .await
            .unwrap();
            assert_eq!((sale.subtotal, sale.tax, sale.total), (5200.0, 420.0, 5620.0));

            // Wholesale: 600 each, no quantity price, no promotions
            let mut ws = cart(vec![CheckoutLine { product_id: cola, quantity: 6.0, ..Default::default() }]);
            ws.price_level = Some("wholesale".into());
            assert_eq!(checkout(&ctx, ws).await.unwrap().subtotal, 3600.0);

            // Buy 2 get 1 on cola: 3 cans → one free, no discount permission needed
            crate::pricing::save_promotion(&ctx, crate::pricing::SavePromotionArgs {
                id: None,
                promotion: crate::pricing::PromotionInput {
                    name: "Cola 2+1".into(),
                    kind: "multibuy".into(),
                    buy_qty: Some(2.0),
                    free_qty: Some(1.0),
                    product_ids: vec![cola],
                    is_active: true,
                    ..Default::default()
                },
            })
            .await
            .unwrap();
            let sale = checkout(&ctx, cart(vec![CheckoutLine { product_id: cola, quantity: 3.0, ..Default::default() }])).await.unwrap();
            assert_eq!((sale.subtotal, sale.promotions, sale.discount), (2250.0, 750.0, 750.0));
            assert_eq!(sale.tax, 150.0, "tax on what's left after the promotion");
            let (name, promo): (Option<String>, f64) = sqlx::query_as("SELECT promotion_name, promo_discount FROM sale_items WHERE sale_id = $1")
                .bind(sale.id)
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            assert_eq!((name.as_deref(), promo), (Some("Cola 2+1"), 750.0));

            // Price changes are remembered
            let mut edit = product("Cola", "1", 0.0);
            edit.selling_price = 800.0;
            edit.wholesale_price = Some(600.0);
            crate::catalog::update_product(&ctx, crate::catalog::UpdateProductArgs { id: cola, product: edit }).await.unwrap();
            let (old, new): (f64, f64) = sqlx::query_as("SELECT old_value, new_value FROM price_history WHERE product_id = $1 AND field = 'price'")
                .bind(cola)
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            assert_eq!((old, new), (750.0, 800.0));
            let changed = crate::pricing::bulk_update_prices(&ctx, crate::pricing::BulkPriceArgs { product_ids: vec![cola, bread], mode: "percent".into(), value: 10.0, round_to: 250.0 }).await.unwrap();
            // +10 % at 250 steps: cola 800 → 880 → 1000; bread 1000 → 1100 → 1000 (unchanged)
            assert_eq!(changed, 1);
            let price = |id: i64| {
                let pool = ctx.pool.clone();
                async move { sqlx::query_as::<_, (f64,)>("SELECT CAST(selling_price AS REAL) FROM products WHERE id = $1").bind(id).fetch_one(&pool).await.unwrap().0 }
            };
            assert_eq!((price(cola).await, price(bread).await), (1000.0, 1000.0));
        });
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
                lines: vec![CheckoutLine { product_id: id, quantity: 2.0, discount: 0.0, ..Default::default() }, CheckoutLine { product_id: id, quantity: 1.0, discount: 0.0, ..Default::default() }],
                cart_discount: 0.0,
                payment_method: "cash".into(),
                tendered: Some(3000.0),
                override_token: None,
                exchange: None,
                refund_override_token: None,
                open_price_override_token: None,
                customer_id: None,
                on_account: 0.0,
                over_limit_override_token: None,
                price_level: None,
                wholesale_override_token: None,
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
