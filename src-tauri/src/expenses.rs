//! Shop expenses and their types. A recurring expense is a template: each
//! week or month after its date it produces a draft for someone to confirm
//! (amounts like electricity change), shown as an `expense_due` alert.

use chrono::{Duration, Local, Months, NaiveDate};
use serde::Deserialize;

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::notifications;
use crate::stock::parse_date;

fn trimmed(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

// ------------------------------------------------------------------ types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseTypeInput {
    pub name_en: String,
    pub name_ku: String,
    pub name_ar: Option<String>,
    pub icon_key: String,
    pub color: Option<String>,
}

fn validate_type(t: &ExpenseTypeInput) -> Result<()> {
    if t.name_en.trim().is_empty() || t.name_ku.trim().is_empty() {
        return Err(Error::Rule("type_names_required"));
    }
    if !t.icon_key.contains(':') {
        return Err(Error::Rule("icon_required"));
    }
    Ok(())
}

pub async fn create_expense_type(ctx: &Ctx, t: ExpenseTypeInput) -> Result<i64> {
    let actor = auth::require(ctx, "expenses.edit").await?;
    validate_type(&t)?;
    let id = sqlx::query(
        "INSERT INTO expense_types (name_en, name_ku, name_ar, icon_key, color, display_order)
         VALUES ($1, $2, $3, $4, $5, (SELECT COALESCE(MAX(display_order), 0) + 1 FROM expense_types))",
    )
    .bind(t.name_en.trim())
    .bind(t.name_ku.trim())
    .bind(trimmed(&t.name_ar))
    .bind(t.icon_key.trim())
    .bind(trimmed(&t.color))
    .execute(&ctx.pool)
    .await?
    .last_insert_rowid();
    audit::log(ctx, Some(actor.user_id), "expense_type.create", "expense_types", Some(id), &serde_json::json!({ "name": t.name_en })).await?;
    Ok(id)
}

#[derive(Debug, Deserialize)]
pub struct UpdateTypeArgs {
    pub id: i64,
    #[serde(rename = "type")]
    pub expense_type: ExpenseTypeInput,
}

pub async fn update_expense_type(ctx: &Ctx, args: UpdateTypeArgs) -> Result<()> {
    let actor = auth::require(ctx, "expenses.edit").await?;
    let t = &args.expense_type;
    validate_type(t)?;
    sqlx::query("UPDATE expense_types SET name_en = $1, name_ku = $2, name_ar = $3, icon_key = $4, color = $5 WHERE id = $6")
        .bind(t.name_en.trim())
        .bind(t.name_ku.trim())
        .bind(trimmed(&t.name_ar))
        .bind(t.icon_key.trim())
        .bind(trimmed(&t.color))
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    audit::log(ctx, Some(actor.user_id), "expense_type.update", "expense_types", Some(args.id), &serde_json::Value::Null).await
}

#[derive(Debug, Deserialize)]
pub struct ActiveArgs {
    pub id: i64,
    pub active: bool,
}

/// Types are archived, never deleted: past expenses keep pointing at them.
pub async fn set_expense_type_active(ctx: &Ctx, args: ActiveArgs) -> Result<()> {
    auth::require(ctx, "expenses.edit").await?;
    sqlx::query("UPDATE expense_types SET is_active = $1 WHERE id = $2")
        .bind(args.active)
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    Ok(())
}

// --------------------------------------------------------------- expenses

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseInput {
    pub type_id: i64,
    pub amount: f64,
    /// YYYY-MM-DD
    pub spent_on: String,
    pub note: Option<String>,
    pub receipt_media_id: Option<i64>,
    /// weekly | monthly
    pub recurring: Option<String>,
}

fn validate_expense(e: &ExpenseInput) -> Result<String> {
    if !(e.amount >= 0.0) {
        return Err(Error::Rule("negative_amount"));
    }
    if let Some(r) = trimmed(&e.recurring) {
        if !matches!(r, "weekly" | "monthly") {
            return Err(Error::Invalid(format!("unknown repeat {r}")));
        }
    }
    parse_date(&e.spent_on)
}

pub async fn create_expense(ctx: &Ctx, e: ExpenseInput) -> Result<i64> {
    let actor = auth::require(ctx, "expenses.edit").await?;
    let on = validate_expense(&e)?;
    let id = sqlx::query(
        "INSERT INTO expenses (type_id, amount, spent_on, note, receipt_media_id, recurring, created_by, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(e.type_id)
    .bind(e.amount)
    .bind(&on)
    .bind(trimmed(&e.note))
    .bind(e.receipt_media_id)
    .bind(trimmed(&e.recurring))
    .bind(actor.user_id)
    .bind(now())
    .execute(&ctx.pool)
    .await?
    .last_insert_rowid();
    audit::log(ctx, Some(actor.user_id), "expense.create", "expenses", Some(id), &serde_json::json!({ "amount": e.amount, "on": on })).await?;
    generate_recurring(ctx).await?;
    Ok(id)
}

#[derive(Debug, Deserialize)]
pub struct UpdateExpenseArgs {
    pub id: i64,
    pub expense: ExpenseInput,
}

pub async fn update_expense(ctx: &Ctx, args: UpdateExpenseArgs) -> Result<()> {
    let actor = auth::require(ctx, "expenses.edit").await?;
    let e = &args.expense;
    let on = validate_expense(e)?;
    let done = sqlx::query(
        "UPDATE expenses SET type_id = $1, amount = $2, spent_on = $3, note = $4,
            receipt_media_id = $5, recurring = CASE WHEN recurring_parent_id IS NULL THEN $6 ELSE NULL END
         WHERE id = $7",
    )
    .bind(e.type_id)
    .bind(e.amount)
    .bind(&on)
    .bind(trimmed(&e.note))
    .bind(e.receipt_media_id)
    .bind(trimmed(&e.recurring))
    .bind(args.id)
    .execute(&ctx.pool)
    .await?;
    if done.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    audit::log(ctx, Some(actor.user_id), "expense.update", "expenses", Some(args.id), &serde_json::json!({ "amount": e.amount })).await?;
    generate_recurring(ctx).await
}

#[derive(Debug, Deserialize)]
pub struct IdArgs {
    pub id: i64,
}

pub async fn confirm_expense(ctx: &Ctx, args: IdArgs) -> Result<()> {
    let actor = auth::require(ctx, "expenses.edit").await?;
    sqlx::query("UPDATE expenses SET status = 'confirmed' WHERE id = $1")
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    audit::log(ctx, Some(actor.user_id), "expense.confirm", "expenses", Some(args.id), &serde_json::Value::Null).await?;
    notifications::refresh_quietly(ctx).await;
    Ok(())
}

pub async fn delete_expense(ctx: &Ctx, args: IdArgs) -> Result<()> {
    let actor = auth::require(ctx, "expenses.edit").await?;
    sqlx::query("DELETE FROM expenses WHERE id = $1")
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    audit::log(ctx, Some(actor.user_id), "expense.delete", "expenses", Some(args.id), &serde_json::Value::Null).await?;
    notifications::refresh_quietly(ctx).await;
    Ok(())
}

/// The period's date after `d`. Monthly keeps the day, clamped to month end
/// (Jan 31 → Feb 28 → Mar 28 would drift, so it's computed from the start).
fn occurrence(start: NaiveDate, recurring: &str, n: u32) -> Option<NaiveDate> {
    match recurring {
        "weekly" => Some(start + Duration::weeks(n as i64)),
        "monthly" => start.checked_add_months(Months::new(n)),
        _ => None,
    }
}

/// Creates a draft for every period of every recurring expense up to today.
/// Idempotent (one row per template and date); runs at startup, hourly and
/// after expense edits.
pub async fn generate_recurring(ctx: &Ctx) -> Result<()> {
    let today = Local::now().date_naive();
    let templates: Vec<(i64, i64, f64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, type_id, amount, spent_on, note, recurring FROM expenses
         WHERE recurring IS NOT NULL AND recurring_parent_id IS NULL AND status = 'confirmed'",
    )
    .fetch_all(&ctx.pool)
    .await?;
    for (id, type_id, amount, on, note, recurring) in templates {
        let Ok(start) = NaiveDate::parse_from_str(&on, "%Y-%m-%d") else { continue };
        // Bounded so a template dated years back can't flood the table
        for n in 1..=60 {
            let Some(date) = occurrence(start, &recurring, n) else { break };
            if date > today {
                break;
            }
            sqlx::query(
                "INSERT OR IGNORE INTO expenses (type_id, amount, spent_on, note, recurring_parent_id, status, created_at)
                 VALUES ($1, $2, $3, $4, $5, 'draft', $6)",
            )
            .bind(type_id)
            .bind(amount)
            .bind(date.format("%Y-%m-%d").to_string())
            .bind(&note)
            .bind(id)
            .bind(now())
            .execute(&ctx.pool)
            .await?;
        }
    }
    notifications::refresh_quietly(ctx).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::ctx::test_support;

    #[test]
    fn monthly_dates_clamp_without_drifting() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        let d: Vec<String> = (1..=3).map(|n| occurrence(start, "monthly", n).unwrap().to_string()).collect();
        assert_eq!(d, ["2026-02-28", "2026-03-31", "2026-04-30"]);
    }

    #[test]
    fn recurring_creates_drafts_once_and_alerts() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let start = (Local::now().date_naive() - Duration::days(20)).format("%Y-%m-%d").to_string();
            create_expense(&ctx, ExpenseInput {
                type_id: 1,
                amount: 50000.0,
                spent_on: start,
                note: None,
                receipt_media_id: None,
                recurring: Some("weekly".into()),
            })
            .await
            .unwrap();
            generate_recurring(&ctx).await.unwrap(); // again: must not duplicate
            let (drafts,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM expenses WHERE status = 'draft'")
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            assert_eq!(drafts, 2, "days 7 and 14 after the start are due; day 21 isn't yet");
            let (alerts,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE kind = 'expense_due' AND resolved_at IS NULL")
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            assert_eq!(alerts, 2);

            let (draft,): (i64,) = sqlx::query_as("SELECT id FROM expenses WHERE status = 'draft' LIMIT 1")
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            confirm_expense(&ctx, IdArgs { id: draft }).await.unwrap();
            let (alerts,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE kind = 'expense_due' AND resolved_at IS NULL")
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            assert_eq!(alerts, 1, "confirming clears its alert");
        });
    }
}
