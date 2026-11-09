//! Settings that belong to one computer rather than the shop: which receipt
//! printer it prints to, its paper width, printing after every sale, and
//! full screen. Kept in this computer's own database, so on an extra till
//! they stay on the till while everything else lives on the main computer.

use serde::Deserialize;
use tauri::AppHandle;

use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};

pub const KEYS: &[&str] = &["receipt_printer", "receipt_width", "auto_print", "fullscreen"];

#[derive(Debug, Deserialize)]
pub struct GetArgs {
    pub key: String,
}

#[tauri::command]
pub async fn get_device_setting(app: AppHandle, args: GetArgs) -> Result<Option<String>> {
    if !KEYS.contains(&args.key.as_str()) {
        return Err(Error::Invalid(format!("unknown device setting {}", args.key)));
    }
    let ctx = Ctx::from_app(&app).await?;
    get(&ctx, &args.key).await
}

pub async fn get(ctx: &Ctx, key: &str) -> Result<Option<String>> {
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT value FROM settings WHERE key = $1")
        .bind(key)
        .fetch_optional(&ctx.pool)
        .await?;
    Ok(row.and_then(|(v,)| v))
}

#[derive(Debug, Deserialize)]
pub struct SetArgs {
    pub key: String,
    pub value: String,
}

fn check(key: &str, v: &str) -> Result<()> {
    let ok = match key {
        "receipt_width" => matches!(v, "58" | "80"),
        "auto_print" | "fullscreen" => matches!(v, "0" | "1"),
        // "", "system:<printer name>" or "tcp:<host>:<port>"
        "receipt_printer" => {
            if !(v.is_empty()
                || v.strip_prefix("system:").is_some_and(|n| !n.trim().is_empty())
                || v.strip_prefix("tcp:").is_some_and(|a| a.rsplit_once(':').is_some_and(|(h, p)| !h.is_empty() && p.parse::<u16>().is_ok())))
            {
                return Err(Error::Rule("printer_address"));
            }
            true
        }
        _ => return Err(Error::Invalid(format!("unknown device setting {key}"))),
    };
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(format!("bad value for {key}")))
    }
}

#[tauri::command]
pub async fn set_device_setting(app: AppHandle, args: SetArgs) -> Result<()> {
    let ctx = Ctx::from_app(&app).await?;
    // On an extra till the main computer says who is signed in
    let actor = auth::require(&ctx, "settings.system").await?;
    check(&args.key, &args.value)?;
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, $3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(&args.key)
    .bind(&args.value)
    .bind(now())
    .execute(&ctx.pool)
    .await?;
    if ctx.remote.is_none() {
        crate::audit::log(&ctx, Some(actor.user_id), "setting", "settings", None, &serde_json::json!({ args.key: args.value })).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_checked() {
        assert!(check("receipt_width", "58").is_ok());
        assert!(check("receipt_width", "70").is_err());
        assert!(check("receipt_printer", "tcp:192.168.1.50:9100").is_ok());
        assert!(matches!(check("receipt_printer", "tcp:nope"), Err(Error::Rule("printer_address"))));
        assert!(check("tax_rate", "5").is_err());
    }
}
