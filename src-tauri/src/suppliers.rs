//! Suppliers, deliveries and what the shop owes them.
//!
//! A delivery is many lines received at once: each becomes a stock batch,
//! updates the product's cost (last cost) and links the product to the
//! supplier. Whatever isn't paid on the spot goes on the supplier's ledger;
//! payments take it off. Money paid out of the till is a negative cash
//! payment, so the drawer's expected cash goes down with it.
//!
//! A supplier can sell in US dollars (`suppliers.currency`): its deliveries
//! are invoiced and its account kept in USD. Product costs are still kept
//! in IQD, converted at the rate in use when the delivery arrives. Paying it
//! from the till takes dollars from the drawer, or dinars at today's rate.

use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, Transaction};

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::sales::round_money;
use crate::{notifications, shifts, stock};

fn trimmed(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierInput {
    pub name: String,
    pub contact_person: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    #[serde(default = "yes")]
    pub is_active: bool,
    /// IQD (default) or USD
    #[serde(default)]
    pub currency: Option<String>,
}

fn supplier_currency(s: &SupplierInput) -> Result<&str> {
    match s.currency.as_deref().unwrap_or("IQD") {
        c @ ("IQD" | "USD") => Ok(c),
        other => Err(Error::Invalid(format!("unknown currency {other}"))),
    }
}

/// The currency a supplier deals in (outside a transaction).
async fn currency_on(ctx: &Ctx, supplier_id: i64) -> Result<String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT currency FROM suppliers WHERE id = $1").bind(supplier_id).fetch_optional(&ctx.pool).await?;
    Ok(row.ok_or(Error::NotFound)?.0)
}

/// The currency a supplier deals in.
async fn currency_of(tx: &mut Transaction<'_, Sqlite>, supplier_id: i64) -> Result<String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT currency FROM suppliers WHERE id = $1").bind(supplier_id).fetch_optional(&mut **tx).await?;
    Ok(row.ok_or(Error::NotFound)?.0)
}

fn yes() -> bool {
    true
}

pub async fn create_supplier(ctx: &Ctx, s: SupplierInput) -> Result<i64> {
    let actor = auth::require(ctx, "suppliers.edit").await?;
    if s.name.trim().is_empty() {
        return Err(Error::Rule("supplier_name_required"));
    }
    let currency = supplier_currency(&s)?;
    let id = sqlx::query(
        "INSERT INTO suppliers (name, contact_person, phone, address, notes, is_active, created_at, currency) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(s.name.trim())
    .bind(trimmed(&s.contact_person))
    .bind(trimmed(&s.phone))
    .bind(trimmed(&s.address))
    .bind(trimmed(&s.notes))
    .bind(s.is_active)
    .bind(now())
    .bind(currency)
    .execute(&ctx.pool)
    .await?
    .last_insert_rowid();
    audit::log(ctx, Some(actor.user_id), "supplier.create", "suppliers", Some(id), &serde_json::json!({ "name": s.name.trim() })).await?;
    Ok(id)
}

#[derive(Debug, Deserialize)]
pub struct UpdateSupplierArgs {
    pub id: i64,
    pub supplier: SupplierInput,
}

pub async fn update_supplier(ctx: &Ctx, args: UpdateSupplierArgs) -> Result<()> {
    let actor = auth::require(ctx, "suppliers.edit").await?;
    let s = &args.supplier;
    if s.name.trim().is_empty() {
        return Err(Error::Rule("supplier_name_required"));
    }
    let currency = supplier_currency(s)?;
    let mut tx = ctx.pool.begin().await?;
    // The account can only change currency while nothing is owed either way
    if currency_of(&mut tx, args.id).await? != currency && balance(&mut tx, args.id).await?.abs() > 1e-9 {
        return Err(Error::Rule("currency_has_balance"));
    }
    let done = sqlx::query(
        "UPDATE suppliers SET name = $1, contact_person = $2, phone = $3, address = $4, notes = $5, is_active = $6, currency = $8 WHERE id = $7",
    )
    .bind(s.name.trim())
    .bind(trimmed(&s.contact_person))
    .bind(trimmed(&s.phone))
    .bind(trimmed(&s.address))
    .bind(trimmed(&s.notes))
    .bind(s.is_active)
    .bind(args.id)
    .bind(currency)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if done.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    audit::log(ctx, Some(actor.user_id), "supplier.update", "suppliers", Some(args.id), &serde_json::to_value(s)?).await
}

/// What the shop owes, in the supplier's currency.
pub async fn balance(tx: &mut Transaction<'_, Sqlite>, supplier_id: i64) -> Result<f64> {
    let (b,): (f64,) = sqlx::query_as(
        "SELECT CAST(COALESCE(SUM(amount), 0) AS REAL) FROM supplier_ledger
         WHERE supplier_id = $1 AND currency = (SELECT currency FROM suppliers WHERE id = $1)",
    )
        .bind(supplier_id)
        .fetch_one(&mut **tx)
        .await?;
    Ok(b)
}

/// How money changes hands: the account's currency, and for money out of
/// the till which drawer it leaves (`paid_in`) and the rate between them.
#[derive(Clone, Copy)]
struct Money<'a> {
    currency: &'a str,
    paid_in: &'a str,
    rate: Option<f64>,
}

const DINARS: Money = Money { currency: "IQD", paid_in: "IQD", rate: None };

/// Writes a ledger row, and for money out of the till the matching
/// negative cash payment. `amount` (in the account's currency): + the shop
/// owes more, − owes less.
#[allow(clippy::too_many_arguments)]
async fn ledger(
    tx: &mut Transaction<'_, Sqlite>,
    supplier_id: i64,
    kind: &str,
    amount: f64,
    delivery_id: Option<i64>,
    method: Option<&str>,
    note: Option<&str>,
    user_id: i64,
    shift_id: Option<i64>,
    at: &str,
    money: Money<'_>,
) -> Result<i64> {
    let id = sqlx::query(
        "INSERT INTO supplier_ledger (supplier_id, kind, amount, delivery_id, method, note, user_id, shift_id, created_at, currency, rate)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(supplier_id)
    .bind(kind)
    .bind(amount)
    .bind(delivery_id)
    .bind(method)
    .bind(note)
    .bind(user_id)
    .bind(shift_id)
    .bind(at)
    .bind(money.currency)
    .bind(money.rate)
    .execute(&mut **tx)
    .await?
    .last_insert_rowid();
    if method == Some("till") && amount < 0.0 {
        till_out(tx, -amount, Some(id), shift_id, at, money).await?;
    }
    Ok(id)
}

/// Money paid out of the drawer: dinars, or dollars (a negative 'usd' row
/// with its value in IQD).
async fn till_out(tx: &mut Transaction<'_, Sqlite>, amount: f64, ledger_id: Option<i64>, shift_id: Option<i64>, at: &str, money: Money<'_>) -> Result<()> {
    let rate = money.rate.unwrap_or(0.0);
    let (method, iqd, dollars) = match (money.currency, money.paid_in) {
        ("USD", "USD") => ("usd", crate::rates::to_iqd(amount, rate), Some(amount)),
        ("USD", _) => ("cash", crate::rates::to_iqd(amount, rate), None),
        _ => ("cash", amount, None),
    };
    sqlx::query(
        "INSERT INTO payments (sale_id, payment_method, amount, foreign_amount, rate, shift_id, supplier_ledger_id, payment_date)
         VALUES (NULL, $1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(method)
    .bind(-iqd)
    .bind(dollars.map(|d| -d))
    .bind(money.rate.filter(|_| money.currency == "USD"))
    .bind(shift_id)
    .bind(ledger_id)
    .bind(at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The currencies for a payment: a dollar account needs today's rate (the
/// one the screen showed) when money leaves the till or the rate is needed
/// for costs.
async fn money_for<'a>(ctx: &Ctx, currency: &'a str, paid_in: Option<&'a str>, shown_rate: Option<f64>, need_rate: bool) -> Result<Money<'a>> {
    let paid_in = match (currency, paid_in) {
        ("IQD", _) => "IQD",
        (_, Some(p @ ("IQD" | "USD"))) => p,
        (_, None) => "USD",
        (_, Some(other)) => return Err(Error::Invalid(format!("unknown currency {other}"))),
    };
    let rate = if currency == "USD" && need_rate { Some(crate::rates::rate_for_payment(ctx, shown_rate).await?) } else { None };
    Ok(Money { currency, paid_in, rate })
}

fn check_method(method: &str) -> Result<()> {
    if matches!(method, "till" | "cash" | "bank") {
        Ok(())
    } else {
        Err(Error::Invalid(format!("unknown payment method {method}")))
    }
}

// ------------------------------------------------------------ deliveries

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryLine {
    pub product_id: i64,
    /// In the pack (unit_id) or the product's own unit
    pub quantity: f64,
    /// Received by the pack: quantity and cost are per pack
    pub unit_id: Option<i64>,
    /// What one (pack or unit) cost
    pub cost: f64,
    pub expiry_date: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryArgs {
    pub supplier_id: Option<i64>,
    pub invoice_number: Option<String>,
    pub invoice_media_id: Option<i64>,
    pub note: Option<String>,
    pub lines: Vec<DeliveryLine>,
    /// Paid on the spot; the rest is owed to the supplier
    #[serde(default)]
    pub paid: f64,
    /// till | cash | bank (how `paid` was paid)
    pub paid_method: Option<String>,
    /// IQD or USD: what the delivery is invoiced in. A supplier's deliveries
    /// are in its own currency; this is for one without a supplier.
    #[serde(default)]
    pub currency: Option<String>,
    /// For a dollar delivery paid from the till: which drawer (IQD or USD)
    #[serde(default)]
    pub paid_in: Option<String>,
    /// The rate the screen showed, for a dollar delivery
    #[serde(default)]
    pub rate: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryResult {
    pub id: i64,
    /// In the delivery's currency
    pub total: f64,
    pub currency: String,
    /// What the shop owes this supplier now
    pub owed: f64,
}

pub async fn receive_delivery(ctx: &Ctx, args: DeliveryArgs) -> Result<DeliveryResult> {
    let actor = auth::require(ctx, "inventory.adjust_stock").await?;
    if args.supplier_id.is_some() && !actor.can("suppliers.edit") {
        return Err(Error::Forbidden("suppliers.edit".into()));
    }
    if args.lines.is_empty() {
        return Err(Error::Rule("delivery_empty"));
    }
    if !(args.paid >= 0.0) {
        return Err(Error::Rule("negative_amount"));
    }
    if args.paid > 0.0 {
        check_method(args.paid_method.as_deref().unwrap_or(""))?;
        if !actor.can("suppliers.pay") {
            return Err(Error::Forbidden("suppliers.pay".into()));
        }
    }
    let shift_id = shifts::shift_info(ctx, actor.user_id).await?.map(|s| s.id);
    let at = now();

    // Currency and rate first: the transaction below keeps its connection
    let currency = match args.supplier_id {
        Some(sup) => {
            let c = currency_on(ctx, sup).await?;
            if args.currency.as_deref().is_some_and(|a| a != c) {
                return Err(Error::Rule("supplier_currency"));
            }
            c
        }
        None => match args.currency.as_deref().unwrap_or("IQD") {
            c @ ("IQD" | "USD") => c.to_owned(),
            other => return Err(Error::Invalid(format!("unknown currency {other}"))),
        },
    };
    let money = money_for(ctx, &currency, args.paid_in.as_deref(), args.rate, true).await?;
    let paid = round_money(args.paid, &currency);
    let mut tx = ctx.pool.begin().await?;
    // What one product unit costs in IQD (margins are worked out in IQD)
    let in_iqd = |cost: f64| match money.rate {
        Some(r) => (cost * r).round() / 100.0,
        None => cost,
    };

    // Work the lines out first: quantity in product units, cost per unit
    struct Line {
        product_id: i64,
        qty: f64,
        /// Per product unit, in IQD
        cost: f64,
        /// Per product unit as invoiced, for a dollar delivery
        foreign: Option<f64>,
        unit: Option<(String, f64)>,
        expiry: Option<String>,
        tracked: bool,
    }
    let mut lines = Vec::new();
    let mut total = 0.0;
    for l in &args.lines {
        if !(l.quantity > 0.0) || !(l.cost >= 0.0) {
            return Err(Error::Rule("negative_amount"));
        }
        let row: Option<(String, String, bool)> = sqlx::query_as("SELECT name, unit, track_stock FROM products WHERE id = $1")
            .bind(l.product_id)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((name, unit, tracked)) = row else { return Err(Error::NotFound) };
        let pack = match l.unit_id {
            Some(uid) => {
                let p: Option<(String, f64)> = sqlx::query_as("SELECT name, CAST(factor AS REAL) FROM product_units WHERE id = $1 AND product_id = $2")
                    .bind(uid)
                    .bind(l.product_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                Some(p.ok_or(Error::NotFound)?)
            }
            None => None,
        };
        let factor = pack.as_ref().map_or(1.0, |p| p.1);
        let qty = l.quantity * factor;
        if crate::catalog::is_whole_unit(&unit) && qty.fract() != 0.0 {
            return Err(Error::RuleAbout("quantity_whole", name));
        }
        total += l.cost * l.quantity;
        let expiry = trimmed(&l.expiry_date).map(stock::parse_date).transpose()?;
        lines.push(Line { product_id: l.product_id, qty, cost: in_iqd(l.cost / factor), foreign: money.rate.map(|_| l.cost / factor), unit: pack, expiry, tracked });
    }
    let total = round_money(total, &currency);
    if paid > total + 1e-9 {
        return Err(Error::Rule("paid_too_much"));
    }
    if paid < total - 1e-9 && args.supplier_id.is_none() {
        // Nobody to owe: a delivery with no supplier is paid in full
        return Err(Error::Rule("supplier_required"));
    }

    let id = sqlx::query(
        "INSERT INTO deliveries (supplier_id, invoice_number, invoice_media_id, total, note, user_id, shift_id, received_at, currency, rate)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(args.supplier_id)
    .bind(trimmed(&args.invoice_number))
    .bind(args.invoice_media_id)
    .bind(total)
    .bind(trimmed(&args.note))
    .bind(actor.user_id)
    .bind(shift_id)
    .bind(&at)
    .bind(&currency)
    .bind(money.rate)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    for l in &lines {
        let batch = if l.tracked {
            sqlx::query("UPDATE products SET stock_quantity = stock_quantity + $1 WHERE id = $2")
                .bind(l.qty)
                .bind(l.product_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "INSERT INTO stock_adjustments (product_id, adjustment_type, quantity, reason, user_id, reference_id, created_at)
                 VALUES ($1, 'addition', $2, 'delivery', $3, $4, $5)",
            )
            .bind(l.product_id)
            .bind(l.qty)
            .bind(actor.user_id)
            .bind(id)
            .bind(&at)
            .execute(&mut *tx)
            .await?;
            Some(stock::insert_batch(&mut tx, l.product_id, l.qty, l.expiry.as_deref(), Some(l.cost), Some("delivery"), actor.user_id).await?)
        } else {
            None
        };
        // Last cost: what this delivery cost
        crate::pricing::record_changes(&mut tx, l.product_id, None, None, Some(l.cost), "delivery", actor.user_id).await?;
        sqlx::query("UPDATE products SET cost_price = $1 WHERE id = $2").bind(l.cost).bind(l.product_id).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO delivery_items (delivery_id, product_id, batch_id, quantity, cost, unit_name, unit_factor, expiry_date, cost_foreign)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(l.product_id)
        .bind(batch)
        .bind(l.qty)
        .bind(l.cost)
        .bind(l.unit.as_ref().map(|u| u.0.as_str()))
        .bind(l.unit.as_ref().map(|u| u.1))
        .bind(&l.expiry)
        .bind(l.foreign)
        .execute(&mut *tx)
        .await?;
        if let Some(sup) = args.supplier_id {
            // Remember who supplies it; the first supplier becomes the main one
            sqlx::query(
                "INSERT INTO product_suppliers (product_id, supplier_id, cost_price, is_primary)
                 VALUES ($1, $2, $3, NOT EXISTS (SELECT 1 FROM product_suppliers WHERE product_id = $1 AND is_primary = 1))
                 ON CONFLICT(product_id, supplier_id) DO UPDATE SET cost_price = excluded.cost_price",
            )
            .bind(l.product_id)
            .bind(sup)
            .bind(l.cost)
            .execute(&mut *tx)
            .await?;
        }
    }

    let mut owed = 0.0;
    if let Some(sup) = args.supplier_id {
        let note = trimmed(&args.invoice_number);
        ledger(&mut tx, sup, "delivery", total, Some(id), None, note, actor.user_id, shift_id, &at, money).await?;
        if paid > 0.0 {
            ledger(&mut tx, sup, "payment", -paid, Some(id), args.paid_method.as_deref(), note, actor.user_id, shift_id, &at, money).await?;
        }
        owed = balance(&mut tx, sup).await?;
    } else if paid > 0.0 && args.paid_method.as_deref() == Some("till") {
        // No supplier, paid from the till: still leaves the drawer
        till_out(&mut tx, paid, None, shift_id, &at, money).await?;
    }
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "delivery", "deliveries", Some(id), &serde_json::json!({ "supplier": args.supplier_id, "total": total, "paid": paid, "lines": lines.len(), "currency": currency, "rate": money.rate })).await?;
    notifications::refresh_quietly(ctx).await;
    Ok(DeliveryResult { id, total, currency, owed })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierPaymentArgs {
    pub supplier_id: i64,
    pub amount: f64,
    /// till | cash | bank
    pub method: String,
    pub note: Option<String>,
    /// A dollar supplier paid from the till: from the dollars (USD, default)
    /// or the dinars (IQD) at `rate`
    #[serde(default)]
    pub paid_in: Option<String>,
    #[serde(default)]
    pub rate: Option<f64>,
}

/// Paying a supplier what the shop owes. Never more than is owed.
pub async fn pay_supplier(ctx: &Ctx, args: SupplierPaymentArgs) -> Result<f64> {
    let actor = auth::require(ctx, "suppliers.pay").await?;
    check_method(&args.method)?;
    let shift_id = shifts::shift_info(ctx, actor.user_id).await?.map(|s| s.id);
    let at = now();
    let currency = currency_on(ctx, args.supplier_id).await?;
    let amount = round_money(args.amount, &currency);
    if !(amount > 0.0) {
        return Err(Error::Rule("negative_amount"));
    }
    // The rate only matters for money leaving the till
    let money = money_for(ctx, &currency, args.paid_in.as_deref(), args.rate, args.method == "till").await?;
    let mut tx = ctx.pool.begin().await?;
    let owed = balance(&mut tx, args.supplier_id).await?;
    if amount > owed + 1e-9 {
        return Err(Error::Rule("payment_exceeds_debt"));
    }
    ledger(&mut tx, args.supplier_id, "payment", -amount, None, Some(&args.method), trimmed(&args.note), actor.user_id, shift_id, &at, money).await?;
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "supplier.payment", "suppliers", Some(args.supplier_id), &serde_json::json!({ "amount": amount, "method": args.method, "currency": currency, "paidIn": money.paid_in, "rate": money.rate })).await?;
    Ok(owed - amount)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierAdjustArgs {
    pub supplier_id: i64,
    /// + the shop owes more (e.g. an old unpaid invoice), − owes less
    pub amount: f64,
    pub note: String,
}

pub async fn adjust_supplier_balance(ctx: &Ctx, args: SupplierAdjustArgs) -> Result<f64> {
    let actor = auth::require(ctx, "suppliers.pay").await?;
    let note = args.note.trim();
    if note.is_empty() {
        return Err(Error::Rule("note_required"));
    }
    let shift_id = shifts::shift_info(ctx, actor.user_id).await?.map(|s| s.id);
    let at = now();
    let mut tx = ctx.pool.begin().await?;
    let currency = currency_of(&mut tx, args.supplier_id).await?;
    let amount = round_money(args.amount, &currency);
    if amount == 0.0 || !amount.is_finite() {
        return Err(Error::Rule("negative_amount"));
    }
    let money = Money { currency: &currency, ..DINARS };
    ledger(&mut tx, args.supplier_id, "adjustment", amount, None, None, Some(note), actor.user_id, shift_id, &at, money).await?;
    let owed = balance(&mut tx, args.supplier_id).await?;
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "supplier.adjust", "suppliers", Some(args.supplier_id), &serde_json::json!({ "amount": amount, "note": note })).await?;
    Ok(owed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::catalog::{create_product, UnitInput};
    use crate::ctx::test_support;
    use crate::sales::tests::product;

    async fn one(ctx: &Ctx, sql: &str, id: i64) -> f64 {
        sqlx::query_as::<_, (f64,)>(&format!("SELECT CAST(({sql}) AS REAL)")).bind(id).fetch_one(&ctx.pool).await.unwrap().0
    }

    #[test]
    fn deliveries_stock_costs_and_what_is_owed() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let mut cola = product("Cola", "1", 0.0);
            cola.units = vec![UnitInput { name: "Carton".into(), factor: 24.0, price: 15000.0, ..Default::default() }];
            let cola = create_product(&ctx, cola).await.unwrap();
            let (carton,): (i64,) = sqlx::query_as("SELECT id FROM product_units").fetch_one(&ctx.pool).await.unwrap();
            let water = create_product(&ctx, product("Water", "2", 0.0)).await.unwrap();
            let sup = create_supplier(&ctx, SupplierInput { name: "Asia Drinks".into(), is_active: true, ..Default::default() }).await.unwrap();
            let drawer = crate::shifts::shift_summary(&ctx).await.unwrap().unwrap().expected_cash;

            // 2 cartons at 12,000 and 10 waters at 250: 26,500; 6,500 paid from the till
            let r = receive_delivery(&ctx, DeliveryArgs {
                supplier_id: Some(sup),
                invoice_number: Some("INV-7".into()),
                lines: vec![
                    DeliveryLine { product_id: cola, quantity: 2.0, unit_id: Some(carton), cost: 12000.0, expiry_date: Some("2027-01-31".into()) },
                    DeliveryLine { product_id: water, quantity: 10.0, cost: 250.0, ..Default::default() },
                ],
                paid: 6500.0,
                paid_method: Some("till".into()),
                ..Default::default()
            })
            .await
            .unwrap();
            assert_eq!((r.total, r.owed), (26500.0, 20000.0));
            assert_eq!(one(&ctx, "SELECT stock_quantity FROM products WHERE id = $1", cola).await, 48.0);
            assert_eq!(one(&ctx, "SELECT cost_price FROM products WHERE id = $1", cola).await, 500.0, "cost per can");
            assert_eq!(one(&ctx, "SELECT COUNT(*) FROM product_batches WHERE product_id = $1 AND expiry_date = '2027-01-31'", cola).await, 1.0);
            assert_eq!(one(&ctx, "SELECT is_primary FROM product_suppliers WHERE product_id = $1", cola).await, 1.0);
            let summary = crate::shifts::shift_summary(&ctx).await.unwrap().unwrap();
            assert_eq!(summary.expected_cash, drawer - 6500.0, "paid from the till");
            assert_eq!(summary.paid_to_suppliers, -6500.0);

            // Paying the rest by bank doesn't touch the drawer; can't overpay
            assert_eq!(pay_supplier(&ctx, SupplierPaymentArgs { supplier_id: sup, amount: 15000.0, method: "bank".into(), note: None, paid_in: None, rate: None }).await.unwrap(), 5000.0);
            let r = pay_supplier(&ctx, SupplierPaymentArgs { supplier_id: sup, amount: 9000.0, method: "cash".into(), note: None, paid_in: None, rate: None }).await;
            assert!(matches!(r, Err(Error::Rule("payment_exceeds_debt"))));
            assert_eq!(crate::shifts::shift_summary(&ctx).await.unwrap().unwrap().expected_cash, drawer - 6500.0);

            // No supplier: must be paid in full
            let r = receive_delivery(&ctx, DeliveryArgs { lines: vec![DeliveryLine { product_id: water, quantity: 1.0, cost: 250.0, ..Default::default() }], ..Default::default() }).await;
            assert!(matches!(r, Err(Error::Rule("supplier_required"))));
            // Half a carton of cans isn't a thing
            let r = receive_delivery(&ctx, DeliveryArgs {
                supplier_id: Some(sup),
                lines: vec![DeliveryLine { product_id: water, quantity: 0.5, cost: 250.0, ..Default::default() }],
                ..Default::default()
            })
            .await;
            assert!(matches!(r, Err(Error::RuleAbout("quantity_whole", _))));
        });
    }
}
