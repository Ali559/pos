//! Customers and their debt (قەرز).
//!
//! A balance is the sum of the customer's ledger; positive means they owe
//! the shop. Rows are only ever added:
//! - `sale`: part or all of a sale left on account (checkout writes it)
//! - `payment`: money paid back, with a matching payment row with no sale
//!   so cash repayments are counted in the drawer
//! - `refund`: a return from an on-account sale taken off the debt
//! - `adjustment`: an opening balance carried over from the paper notebook,
//!   or a correction, always with a note

use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::audit;
use crate::auth::{self, Actor};
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::sales::{round_money, shop_currency};
use crate::shifts;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomerInput {
    pub name: String,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    /// Most they may owe; None for no limit
    pub credit_limit: Option<f64>,
    #[serde(default = "yes")]
    pub is_active: bool,
}

fn yes() -> bool {
    true
}

fn trimmed(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Digits only (and a leading +), so "0750 123 4567" and "07501234567" match.
pub fn normalize_phone(phone: &str) -> String {
    phone.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect()
}

/// Creating a customer at the till is part of selling on account, so either
/// permission will do; editing needs customers.edit.
async fn may_create(ctx: &Ctx) -> Result<Actor> {
    let actor = auth::current(ctx).await?;
    if actor.can("customers.edit") || actor.can("customers.credit") {
        Ok(actor)
    } else {
        Err(Error::Forbidden("customers.edit".into()))
    }
}

async fn validate(ctx: &Ctx, c: &CustomerInput, id: Option<i64>) -> Result<Option<String>> {
    if c.name.trim().is_empty() {
        return Err(Error::Rule("customer_name_required"));
    }
    if c.credit_limit.is_some_and(|l| !(l >= 0.0)) {
        return Err(Error::Rule("negative_amount"));
    }
    let phone = trimmed(&c.phone).map(normalize_phone).filter(|p| !p.is_empty());
    if let Some(p) = &phone {
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM customers WHERE phone = $1 AND id IS NOT $2")
            .bind(p)
            .bind(id)
            .fetch_one(&ctx.pool)
            .await?;
        if n > 0 {
            return Err(Error::Rule("phone_taken"));
        }
    }
    Ok(phone)
}

pub async fn create_customer(ctx: &Ctx, c: CustomerInput) -> Result<i64> {
    let actor = may_create(ctx).await?;
    let phone = validate(ctx, &c, None).await?;
    // Only an editor sets a credit limit; a cashier's new customer has none
    let limit = if actor.can("customers.edit") { c.credit_limit } else { None };
    let id = sqlx::query(
        "INSERT INTO customers (name, phone, address, notes, credit_limit, is_active, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $7)",
    )
    .bind(c.name.trim())
    .bind(&phone)
    .bind(trimmed(&c.address))
    .bind(trimmed(&c.notes))
    .bind(limit)
    .bind(c.is_active)
    .bind(now())
    .execute(&ctx.pool)
    .await?
    .last_insert_rowid();
    audit::log(ctx, Some(actor.user_id), "customer.create", "customers", Some(id), &serde_json::json!({ "name": c.name.trim() })).await?;
    Ok(id)
}

#[derive(Debug, Deserialize)]
pub struct UpdateCustomerArgs {
    pub id: i64,
    pub customer: CustomerInput,
}

pub async fn update_customer(ctx: &Ctx, args: UpdateCustomerArgs) -> Result<()> {
    let actor = auth::require(ctx, "customers.edit").await?;
    let c = &args.customer;
    let phone = validate(ctx, c, Some(args.id)).await?;
    let done = sqlx::query(
        "UPDATE customers SET name = $1, phone = $2, address = $3, notes = $4, credit_limit = $5, is_active = $6, updated_at = $7
         WHERE id = $8",
    )
    .bind(c.name.trim())
    .bind(&phone)
    .bind(trimmed(&c.address))
    .bind(trimmed(&c.notes))
    .bind(c.credit_limit)
    .bind(c.is_active)
    .bind(now())
    .bind(args.id)
    .execute(&ctx.pool)
    .await?;
    if done.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    audit::log(ctx, Some(actor.user_id), "customer.update", "customers", Some(args.id), &serde_json::to_value(c)?).await
}

// ------------------------------------------------------------ the ledger

pub async fn balance(pool: &SqlitePool, customer_id: i64) -> Result<f64> {
    let (b,): (f64,) = sqlx::query_as("SELECT CAST(COALESCE(SUM(amount), 0) AS REAL) FROM customer_ledger WHERE customer_id = $1")
        .bind(customer_id)
        .fetch_one(pool)
        .await?;
    Ok(b)
}

pub struct Entry<'a> {
    pub customer_id: i64,
    pub kind: &'a str,
    pub amount: f64,
    pub sale_id: Option<i64>,
    pub return_id: Option<i64>,
    pub method: Option<&'a str>,
    pub note: Option<&'a str>,
    pub user_id: i64,
    pub shift_id: Option<i64>,
    pub at: &'a str,
}

pub async fn write_entry(tx: &mut Transaction<'_, Sqlite>, e: Entry<'_>) -> Result<i64> {
    Ok(sqlx::query(
        "INSERT INTO customer_ledger (customer_id, kind, amount, sale_id, return_id, method, note, user_id, shift_id, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(e.customer_id)
    .bind(e.kind)
    .bind(e.amount)
    .bind(e.sale_id)
    .bind(e.return_id)
    .bind(e.method)
    .bind(e.note)
    .bind(e.user_id)
    .bind(e.shift_id)
    .bind(e.at)
    .execute(&mut **tx)
    .await?
    .last_insert_rowid())
}

/// Checks a sale may leave `amount` on the customer's account: the cashier
/// may sell on account, the customer exists and is active, and the debt
/// stays within their limit unless a manager approved going over
/// (`customers.over_limit`). Runs inside checkout's transaction.
pub async fn check_on_account(
    ctx: &Ctx,
    tx: &mut Transaction<'_, Sqlite>,
    actor: &Actor,
    customer_id: i64,
    amount: f64,
    override_token: Option<&str>,
) -> Result<Option<i64>> {
    if !actor.can("customers.credit") {
        return Err(Error::Forbidden("customers.credit".into()));
    }
    let row: Option<(String, bool, Option<f64>, f64)> = sqlx::query_as(
        "SELECT name, is_active, credit_limit,
                CAST(COALESCE((SELECT SUM(amount) FROM customer_ledger WHERE customer_id = c.id), 0) AS REAL)
         FROM customers c WHERE id = $1",
    )
    .bind(customer_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((name, active, limit, owed)) = row else {
        return Err(Error::NotFound);
    };
    if !active {
        return Err(Error::RuleAbout("customer_inactive", name));
    }
    match limit {
        Some(limit) if owed + amount > limit + 1e-9 => auth::require_or_override(ctx, actor, "customers.over_limit", override_token)
            .await
            .map_err(|e| match e {
                Error::Forbidden(_) => Error::RuleAbout("credit_limit", name),
                e => e,
            }),
        _ => Ok(None),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentArgs {
    pub customer_id: i64,
    pub amount: f64,
    /// cash | card
    pub method: String,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentResult {
    pub id: i64,
    /// What they still owe
    pub balance: f64,
}

/// Money paid towards the debt. Never more than is owed.
pub async fn record_customer_payment(ctx: &Ctx, args: PaymentArgs) -> Result<PaymentResult> {
    let actor = auth::require(ctx, "customers.credit").await?;
    if !matches!(args.method.as_str(), "cash" | "card") {
        return Err(Error::Invalid(format!("unknown payment method {}", args.method)));
    }
    let currency = shop_currency(ctx).await?;
    let amount = round_money(args.amount, &currency);
    if !(amount > 0.0) {
        return Err(Error::Rule("negative_amount"));
    }
    let owed = balance(&ctx.pool, args.customer_id).await?;
    if amount > owed + 1e-9 {
        return Err(Error::Rule("payment_exceeds_debt"));
    }
    let shift_id = shifts::shift_info(ctx, actor.user_id).await?.map(|s| s.id);
    let at = now();
    let mut tx = ctx.pool.begin().await?;
    let id = write_entry(&mut tx, Entry {
        customer_id: args.customer_id,
        kind: "payment",
        amount: -amount,
        sale_id: None,
        return_id: None,
        method: Some(&args.method),
        note: trimmed(&args.note),
        user_id: actor.user_id,
        shift_id,
        at: &at,
    })
    .await?;
    sqlx::query("INSERT INTO payments (sale_id, payment_method, amount, shift_id, ledger_id, payment_date) VALUES (NULL, $1, $2, $3, $4, $5)")
        .bind(&args.method)
        .bind(amount)
        .bind(shift_id)
        .bind(id)
        .bind(&at)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "customer.payment", "customers", Some(args.customer_id), &serde_json::json!({ "amount": amount, "method": args.method })).await?;
    Ok(PaymentResult { id, balance: owed - amount })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdjustArgs {
    pub customer_id: i64,
    /// + they owe more (e.g. the old notebook's balance), − they owe less
    pub amount: f64,
    pub note: String,
}

/// Opening balances and corrections. Always with a reason.
pub async fn adjust_customer_balance(ctx: &Ctx, args: AdjustArgs) -> Result<f64> {
    let actor = auth::require(ctx, "customers.edit").await?;
    let currency = shop_currency(ctx).await?;
    let amount = round_money(args.amount, &currency);
    if amount == 0.0 || !amount.is_finite() {
        return Err(Error::Rule("negative_amount"));
    }
    let note = args.note.trim();
    if note.is_empty() {
        return Err(Error::Rule("note_required"));
    }
    let found: Option<(i64,)> = sqlx::query_as("SELECT id FROM customers WHERE id = $1").bind(args.customer_id).fetch_optional(&ctx.pool).await?;
    found.ok_or(Error::NotFound)?;
    let shift_id = shifts::shift_info(ctx, actor.user_id).await?.map(|s| s.id);
    let at = now();
    let mut tx = ctx.pool.begin().await?;
    write_entry(&mut tx, Entry {
        customer_id: args.customer_id,
        kind: "adjustment",
        amount,
        sale_id: None,
        return_id: None,
        method: None,
        note: Some(note),
        user_id: actor.user_id,
        shift_id,
        at: &at,
    })
    .await?;
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "customer.adjust", "customers", Some(args.customer_id), &serde_json::json!({ "amount": amount, "note": note })).await?;
    balance(&ctx.pool, args.customer_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::catalog::create_product;
    use crate::ctx::test_support;
    use crate::returns::{create_return, ReturnArgs, ReturnItems, ReturnLine};
    use crate::sales::tests::{product, sell};
    use crate::sales::checkout;

    fn customer(name: &str, phone: &str, limit: Option<f64>) -> CustomerInput {
        CustomerInput { name: name.into(), phone: Some(phone.into()), credit_limit: limit, is_active: true, ..Default::default() }
    }

    async fn cash_in_drawer(ctx: &Ctx) -> f64 {
        crate::shifts::shift_summary(ctx).await.unwrap().unwrap().expected_cash
    }

    #[test]
    fn debt_from_sales_payments_refunds_and_adjustments() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let cola = create_product(&ctx, product("Cola", "1", 20.0)).await.unwrap(); // 750 each
            let karwan = create_customer(&ctx, customer("Karwan", "0750 123 4567", Some(3000.0))).await.unwrap();
            // Same phone written differently is the same customer
            let r = create_customer(&ctx, customer("Other", "07501234567", None)).await;
            assert!(matches!(r, Err(Error::Rule("phone_taken"))));
            let drawer = cash_in_drawer(&ctx).await;

            // 4 × 750 = 3000: 1000 paid in cash now, 2000 on account
            let mut args = sell(cola, 4.0, "cash");
            args.customer_id = Some(karwan);
            args.on_account = 2000.0;
            let sale = checkout(&ctx, args).await.unwrap();
            assert_eq!(balance(&ctx.pool, karwan).await.unwrap(), 2000.0);
            assert_eq!(cash_in_drawer(&ctx).await, drawer + 1000.0, "only the cash part is in the drawer");
            let (paid,): (f64,) = sqlx::query_as("SELECT CAST(SUM(amount) AS REAL) FROM payments WHERE sale_id = $1").bind(sale.id).fetch_one(&ctx.pool).await.unwrap();
            assert_eq!(paid, 3000.0, "the sale's payments still add up to its total");

            // Over the 3000 limit: refused without a manager…
            let mut args = sell(cola, 2.0, "cash");
            args.customer_id = Some(karwan);
            args.on_account = 1500.0;
            // …the owner holds customers.over_limit, so check the rule directly
            let cashier = Actor { user_id: 1, role_id: 3, is_owner: false, permissions: ["customers.credit".to_owned()].into() };
            let mut tx = ctx.pool.begin().await.unwrap();
            let r = check_on_account(&ctx, &mut tx, &cashier, karwan, 1500.0, None).await;
            assert!(matches!(r, Err(Error::RuleAbout("credit_limit", _))));
            tx.rollback().await.unwrap();
            checkout(&ctx, args).await.unwrap();
            assert_eq!(balance(&ctx.pool, karwan).await.unwrap(), 3500.0);

            // On account needs a customer
            let mut args = sell(cola, 1.0, "cash");
            args.on_account = 750.0;
            assert!(matches!(checkout(&ctx, args).await, Err(Error::Rule("customer_required"))));

            // A cash repayment lands in the drawer; can't pay more than owed
            let before = cash_in_drawer(&ctx).await;
            let r = record_customer_payment(&ctx, PaymentArgs { customer_id: karwan, amount: 1000.0, method: "cash".into(), note: None }).await.unwrap();
            assert_eq!(r.balance, 2500.0);
            assert_eq!(cash_in_drawer(&ctx).await, before + 1000.0);
            let r = record_customer_payment(&ctx, PaymentArgs { customer_id: karwan, amount: 9999.0, method: "cash".into(), note: None }).await;
            assert!(matches!(r, Err(Error::Rule("payment_exceeds_debt"))));

            // Returning 2 cans from the first sale onto the account: −1500
            let (line,): (i64,) = sqlx::query_as("SELECT id FROM sale_items WHERE sale_id = $1").bind(sale.id).fetch_one(&ctx.pool).await.unwrap();
            let before = cash_in_drawer(&ctx).await;
            create_return(&ctx, ReturnArgs {
                items: ReturnItems { sale_id: Some(sale.id), lines: vec![ReturnLine { sale_item_id: line, quantity: 2.0, restock: true }], ..Default::default() },
                refund_method: "credit".into(),
                override_token: None,
            })
            .await
            .unwrap();
            assert_eq!(balance(&ctx.pool, karwan).await.unwrap(), 1000.0);
            assert_eq!(cash_in_drawer(&ctx).await, before, "nothing left the drawer");

            // The old notebook's balance, with a reason
            let r = adjust_customer_balance(&ctx, AdjustArgs { customer_id: karwan, amount: 5000.0, note: " ".into() }).await;
            assert!(matches!(r, Err(Error::Rule("note_required"))));
            let b = adjust_customer_balance(&ctx, AdjustArgs { customer_id: karwan, amount: 5000.0, note: "notebook".into() }).await.unwrap();
            assert_eq!(b, 6000.0);
        });
    }
}
