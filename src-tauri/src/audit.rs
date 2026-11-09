use crate::ctx::{now, Ctx};
use crate::error::Result;

/// Appends to `audit_log`. `details` is stored as JSON in `new_value`.
/// On an extra till the log lives on the main computer; the only thing a
/// till writes itself is saving a file (export), which it reports there.
pub async fn log(
    ctx: &Ctx,
    user_id: Option<i64>,
    action: &str,
    table: &str,
    record_id: Option<i64>,
    details: &serde_json::Value,
) -> Result<()> {
    if let Some(remote) = &ctx.remote {
        if action == "export" {
            remote.invoke("record_export", serde_json::json!({ "details": details })).await?;
        }
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO audit_log (user_id, action, table_name, record_id, new_value, created_at, till_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(user_id)
    .bind(action)
    .bind(table)
    .bind(record_id)
    .bind((!details.is_null()).then(|| details.to_string()))
    .bind(now())
    .bind(ctx.till_id)
    .execute(&ctx.pool)
    .await?;
    Ok(())
}
