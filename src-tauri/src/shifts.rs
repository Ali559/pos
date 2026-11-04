//! Shifts (what happened) and schedules (the plan). Times are RFC 3339 UTC.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};

/// A scheduled shift counts as "this shift" from this long before it starts.
const EARLY_CLOCK_IN_MINUTES: i64 = 60;
const MAX_SHIFT_HOURS: i64 = 24;

fn fmt(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn parse(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| Error::Invalid(format!("not an RFC 3339 time: {s}")))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ShiftInfo {
    pub id: i64,
    pub clocked_in_at: String,
    pub schedule_id: Option<i64>,
    /// End of the linked schedule, if the shift has one
    pub scheduled_end: Option<String>,
}

/// The user's open shift, if any.
pub async fn shift_info(ctx: &Ctx, user_id: i64) -> Result<Option<ShiftInfo>> {
    Ok(sqlx::query_as(
        "SELECT s.id, s.clocked_in_at, s.schedule_id, ss.ends_at AS scheduled_end
         FROM shifts s LEFT JOIN shift_schedules ss ON ss.id = s.schedule_id
         WHERE s.user_id = $1 AND s.clocked_out_at IS NULL",
    )
    .bind(user_id)
    .fetch_optional(&ctx.pool)
    .await?)
}

/// Called on sign-in: resumes the open shift or starts one, linked to the
/// schedule that covers now (or starts within the hour).
pub async fn start_or_resume(ctx: &Ctx, user_id: i64) -> Result<()> {
    if shift_info(ctx, user_id).await?.is_some() {
        return Ok(());
    }
    let now_t = Utc::now();
    let schedule: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM shift_schedules
         WHERE user_id = $1 AND starts_at <= $2 AND ends_at > $3
         ORDER BY starts_at LIMIT 1",
    )
    .bind(user_id)
    .bind(fmt(now_t + Duration::minutes(EARLY_CLOCK_IN_MINUTES)))
    .bind(fmt(now_t))
    .fetch_optional(&ctx.pool)
    .await?;
    let id = sqlx::query("INSERT INTO shifts (user_id, schedule_id, clocked_in_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(schedule.map(|(id,)| id))
        .bind(fmt(now_t))
        .execute(&ctx.pool)
        .await?
        .last_insert_rowid();
    ensure_drawer(ctx, user_id, id).await?;
    audit::log(ctx, Some(user_id), "shift.start", "shifts", Some(id), &serde_json::Value::Null).await
}

// ----------------------------------------------------------- cash drawer

/// The shift's drawer, opened with the `opening_float` setting the first
/// time it's needed (shifts from before drawers existed get one lazily).
async fn ensure_drawer(ctx: &Ctx, user_id: i64, shift_id: i64) -> Result<(i64, f64)> {
    let existing: Option<(i64, f64)> = sqlx::query_as(
        "SELECT d.id, CAST(d.opening_amount AS REAL) FROM shifts s JOIN cash_drawers d ON d.id = s.cash_drawer_id WHERE s.id = $1",
    )
    .bind(shift_id)
    .fetch_optional(&ctx.pool)
    .await?;
    if let Some(d) = existing {
        return Ok(d);
    }
    let float: f64 = crate::settings::get(ctx, "opening_float", "0").await?.parse().unwrap_or(0.0);
    let opened: (String,) = sqlx::query_as("SELECT clocked_in_at FROM shifts WHERE id = $1").bind(shift_id).fetch_one(&ctx.pool).await?;
    let id = sqlx::query("INSERT INTO cash_drawers (user_id, shift_id, opening_amount, opened_at) VALUES ($1, $2, $3, $4)")
        .bind(user_id)
        .bind(shift_id)
        .bind(float)
        .bind(&opened.0)
        .execute(&ctx.pool)
        .await?
        .last_insert_rowid();
    sqlx::query("UPDATE shifts SET cash_drawer_id = $1 WHERE id = $2").bind(id).bind(shift_id).execute(&ctx.pool).await?;
    Ok((id, float))
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShiftSummary {
    pub shift_id: i64,
    pub clocked_in_at: String,
    pub opening_float: f64,
    pub sales: i64,
    pub cash_sales: f64,
    pub card_sales: f64,
    /// Negative: money paid back out
    pub cash_refunds: f64,
    pub card_refunds: f64,
    /// What the drawer should hold now
    pub expected_cash: f64,
}

async fn summary_for(ctx: &Ctx, user_id: i64) -> Result<Option<ShiftSummary>> {
    let Some(shift) = shift_info(ctx, user_id).await? else {
        return Ok(None);
    };
    let (_, float) = ensure_drawer(ctx, user_id, shift.id).await?;
    let (sales, cash_sales, card_sales, cash_refunds, card_refunds): (i64, f64, f64, f64, f64) = sqlx::query_as(
        "SELECT COUNT(DISTINCT CASE WHEN amount > 0 THEN sale_id END),
                COALESCE(SUM(CASE WHEN payment_method = 'cash' AND amount > 0 THEN amount END), 0.0),
                COALESCE(SUM(CASE WHEN payment_method = 'card' AND amount > 0 THEN amount END), 0.0),
                COALESCE(SUM(CASE WHEN payment_method = 'cash' AND amount < 0 THEN amount END), 0.0),
                COALESCE(SUM(CASE WHEN payment_method = 'card' AND amount < 0 THEN amount END), 0.0)
         FROM payments WHERE shift_id = $1",
    )
    .bind(shift.id)
    .fetch_one(&ctx.pool)
    .await?;
    Ok(Some(ShiftSummary {
        shift_id: shift.id,
        clocked_in_at: shift.clocked_in_at,
        opening_float: float,
        sales,
        cash_sales,
        card_sales,
        cash_refunds,
        card_refunds,
        expected_cash: float + cash_sales + cash_refunds,
    }))
}

/// The signed-in user's open shift, for the End shift dialog.
pub async fn shift_summary(ctx: &Ctx) -> Result<Option<ShiftSummary>> {
    let actor = auth::current(ctx).await?;
    summary_for(ctx, actor.user_id).await
}

/// Records the count and closes the drawer. Called just before clock-out.
pub async fn close_drawer(ctx: &Ctx, user_id: i64, counted: Option<f64>, note: Option<&str>) -> Result<()> {
    if counted.is_some_and(|c| !(c >= 0.0)) {
        return Err(Error::Rule("negative_amount"));
    }
    let Some(sum) = summary_for(ctx, user_id).await? else {
        return Ok(());
    };
    let note = note.map(str::trim).filter(|s| !s.is_empty());
    sqlx::query(
        "UPDATE cash_drawers SET closing_amount = $1, expected_amount = $2, difference = $3, closed_at = $4, notes = $5
         WHERE id = (SELECT cash_drawer_id FROM shifts WHERE id = $6)",
    )
    .bind(counted)
    .bind(sum.expected_cash)
    .bind(counted.map(|c| c - sum.expected_cash))
    .bind(now())
    .bind(note)
    .bind(sum.shift_id)
    .execute(&ctx.pool)
    .await?;
    audit::log(ctx, Some(user_id), "drawer.close", "shifts", Some(sum.shift_id), &serde_json::json!({ "expected": sum.expected_cash, "counted": counted })).await
}

pub async fn clock_out(ctx: &Ctx, user_id: i64) -> Result<()> {
    let Some(shift) = shift_info(ctx, user_id).await? else {
        return Ok(());
    };
    sqlx::query("UPDATE shifts SET clocked_out_at = $1 WHERE id = $2")
        .bind(now())
        .bind(shift.id)
        .execute(&ctx.pool)
        .await?;
    audit::log(ctx, Some(user_id), "shift.end", "shifts", Some(shift.id), &serde_json::Value::Null).await
}

// ------------------------------------------------------------- schedules

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleInput {
    pub id: Option<i64>,
    pub user_id: i64,
    pub starts_at: String,
    pub ends_at: String,
    pub note: Option<String>,
}

async fn overlaps(ctx: &Ctx, user_id: i64, start: &str, end: &str, except: Option<i64>) -> Result<bool> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM shift_schedules
         WHERE user_id = $1 AND starts_at < $2 AND ends_at > $3 AND id IS NOT $4",
    )
    .bind(user_id)
    .bind(end)
    .bind(start)
    .bind(except)
    .fetch_one(&ctx.pool)
    .await?;
    Ok(n > 0)
}

/// Creates or updates one scheduled shift. Returns its id.
pub async fn save_schedule(ctx: &Ctx, args: ScheduleInput) -> Result<i64> {
    let actor = auth::require(ctx, "staff.shifts").await?;
    let (start, end) = (parse(&args.starts_at)?, parse(&args.ends_at)?);
    if end <= start || end - start > Duration::hours(MAX_SHIFT_HOURS) {
        return Err(Error::Rule("schedule_length"));
    }
    let (start, end) = (fmt(start), fmt(end));
    if overlaps(ctx, args.user_id, &start, &end, args.id).await? {
        return Err(Error::Rule("schedule_overlap"));
    }
    let note = args.note.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let id = match args.id {
        Some(id) => {
            let done = sqlx::query(
                "UPDATE shift_schedules SET user_id = $1, starts_at = $2, ends_at = $3, note = $4 WHERE id = $5",
            )
            .bind(args.user_id)
            .bind(&start)
            .bind(&end)
            .bind(note)
            .bind(id)
            .execute(&ctx.pool)
            .await?;
            if done.rows_affected() == 0 {
                return Err(Error::NotFound);
            }
            id
        }
        None => sqlx::query(
            "INSERT INTO shift_schedules (user_id, starts_at, ends_at, note) VALUES ($1, $2, $3, $4)",
        )
        .bind(args.user_id)
        .bind(&start)
        .bind(&end)
        .bind(note)
        .execute(&ctx.pool)
        .await?
        .last_insert_rowid(),
    };
    audit::log(ctx, Some(actor.user_id), "schedule.save", "shift_schedules", Some(id), &serde_json::json!({ "user": args.user_id, "start": start, "end": end })).await?;
    Ok(id)
}

#[derive(Debug, Deserialize)]
pub struct IdArgs {
    pub id: i64,
}

pub async fn delete_schedule(ctx: &Ctx, args: IdArgs) -> Result<()> {
    let actor = auth::require(ctx, "staff.shifts").await?;
    sqlx::query("DELETE FROM shift_schedules WHERE id = $1")
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    audit::log(ctx, Some(actor.user_id), "schedule.delete", "shift_schedules", Some(args.id), &serde_json::Value::Null).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyWeekArgs {
    /// Start of the source week (local midnight, sent as a UTC instant)
    pub from_start: String,
    pub to_start: String,
}

#[derive(Debug, Serialize)]
pub struct CopyWeekResult {
    pub copied: i64,
    pub skipped: i64,
}

/// Copies every schedule in the 7 days from `from_start` to the same offset
/// from `to_start`. Shifts that would overlap an existing one are skipped.
pub async fn copy_schedule_week(ctx: &Ctx, args: CopyWeekArgs) -> Result<CopyWeekResult> {
    let actor = auth::require(ctx, "staff.shifts").await?;
    let (from, to) = (parse(&args.from_start)?, parse(&args.to_start)?);
    let delta = to - from;
    let rows: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT user_id, starts_at, ends_at, note FROM shift_schedules
         WHERE starts_at >= $1 AND starts_at < $2 ORDER BY starts_at",
    )
    .bind(fmt(from))
    .bind(fmt(from + Duration::days(7)))
    .fetch_all(&ctx.pool)
    .await?;

    let mut result = CopyWeekResult { copied: 0, skipped: 0 };
    for (user_id, s, e, note) in rows {
        let (s, e) = (fmt(parse(&s)? + delta), fmt(parse(&e)? + delta));
        if overlaps(ctx, user_id, &s, &e, None).await? {
            result.skipped += 1;
            continue;
        }
        sqlx::query("INSERT INTO shift_schedules (user_id, starts_at, ends_at, note) VALUES ($1, $2, $3, $4)")
            .bind(user_id)
            .bind(&s)
            .bind(&e)
            .bind(note)
            .execute(&ctx.pool)
            .await?;
        result.copied += 1;
    }
    audit::log(ctx, Some(actor.user_id), "schedule.copy_week", "shift_schedules", None, &serde_json::json!({ "from": args.from_start, "to": args.to_start, "copied": result.copied })).await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::ctx::test_support;

    fn input(user_id: i64, start: &str, end: &str) -> ScheduleInput {
        ScheduleInput { id: None, user_id, starts_at: start.into(), ends_at: end.into(), note: None }
    }

    #[test]
    fn schedules_reject_overlap_and_copy_weeks() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            let uid = setup_owner(&ctx).await.session.unwrap().user.id;

            save_schedule(&ctx, input(uid, "2026-11-07T06:00:00Z", "2026-11-07T14:00:00Z")).await.unwrap();
            // Local-offset input is normalized to UTC
            save_schedule(&ctx, input(uid, "2026-11-08T09:00:00+03:00", "2026-11-08T17:00:00+03:00")).await.unwrap();
            let overlap = save_schedule(&ctx, input(uid, "2026-11-07T13:00:00Z", "2026-11-07T20:00:00Z")).await;
            assert!(matches!(overlap, Err(Error::Rule("schedule_overlap"))));
            let backwards = save_schedule(&ctx, input(uid, "2026-11-07T20:00:00Z", "2026-11-07T19:00:00Z")).await;
            assert!(matches!(backwards, Err(Error::Rule("schedule_length"))));

            let r = copy_schedule_week(&ctx, CopyWeekArgs {
                from_start: "2026-11-06T21:00:00Z".into(),
                to_start: "2026-11-13T21:00:00Z".into(),
            })
            .await
            .unwrap();
            assert_eq!((r.copied, r.skipped), (2, 0));
            // Copying again finds every slot taken
            let r = copy_schedule_week(&ctx, CopyWeekArgs {
                from_start: "2026-11-06T21:00:00Z".into(),
                to_start: "2026-11-13T21:00:00Z".into(),
            })
            .await
            .unwrap();
            assert_eq!((r.copied, r.skipped), (0, 2));

            let (copied_start,): (String,) = sqlx::query_as(
                "SELECT starts_at FROM shift_schedules ORDER BY starts_at DESC LIMIT 1",
            )
            .fetch_one(&ctx.pool)
            .await
            .unwrap();
            assert_eq!(copied_start, "2026-11-15T06:00:00Z");
        });
    }

    #[test]
    fn sign_in_links_the_current_schedule() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            let uid = setup_owner(&ctx).await.session.unwrap().user.id;
            crate::auth::end_shift(&ctx, Default::default()).await.unwrap();

            let now_t = Utc::now();
            let sched = save_schedule_unchecked(&ctx, uid, now_t - Duration::hours(1), now_t + Duration::hours(3)).await;
            start_or_resume(&ctx, uid).await.unwrap();
            let info = shift_info(&ctx, uid).await.unwrap().unwrap();
            assert_eq!(info.schedule_id, Some(sched));
            assert!(info.scheduled_end.is_some());
        });
    }

    async fn save_schedule_unchecked(ctx: &Ctx, uid: i64, s: DateTime<Utc>, e: DateTime<Utc>) -> i64 {
        sqlx::query("INSERT INTO shift_schedules (user_id, starts_at, ends_at) VALUES ($1, $2, $3)")
            .bind(uid)
            .bind(fmt(s))
            .bind(fmt(e))
            .execute(&ctx.pool)
            .await
            .unwrap()
            .last_insert_rowid()
    }
}
