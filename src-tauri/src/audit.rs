use crate::ctx::{now, Ctx};
use crate::error::Result;

/// Appends to `audit_log`. `details` is stored as JSON in `new_value`.
pub async fn log(
    ctx: &Ctx,
    user_id: Option<i64>,
    action: &str,
    table: &str,
    record_id: Option<i64>,
    details: &serde_json::Value,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_log (user_id, action, table_name, record_id, new_value, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(action)
    .bind(table)
    .bind(record_id)
    .bind((!details.is_null()).then(|| details.to_string()))
    .bind(now())
    .execute(&ctx.pool)
    .await?;
    Ok(())
}
