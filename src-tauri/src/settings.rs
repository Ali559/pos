//! Settings and shop profile writes. The WebView can only read the database;
//! these commands are the way to change it.

use serde::{Deserialize, Serialize};

use crate::auth;
use crate::audit;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};

#[derive(Debug, Deserialize)]
pub struct SetSettingArgs {
    pub key: String,
    pub value: String,
}

fn flag(value: &str) -> Result<()> {
    if matches!(value, "0" | "1") {
        Ok(())
    } else {
        Err(Error::Invalid("expected 0 or 1".into()))
    }
}

/// Only known keys, each with its own rule for who may change it.
pub async fn set_setting(ctx: &Ctx, args: SetSettingArgs) -> Result<()> {
    let v = args.value.as_str();
    let actor = match args.key.as_str() {
        // Chosen once during first-run setup, then fixed: every name in the
        // shop is written in this language
        "language" => {
            if !auth::needs_setup(ctx).await? {
                return Err(Error::Rule("language_locked"));
            }
            if !matches!(v, "ku" | "ar" | "en") {
                return Err(Error::Invalid(format!("unknown language {v}")));
            }
            None
        }
        "tax_rate" => {
            let actor = auth::require(ctx, "settings.system").await?;
            match v.parse::<f64>() {
                Ok(n) if (0.0..=100.0).contains(&n) => Some(actor),
                _ => return Err(Error::Rule("tax_rate_range")),
            }
        }
        "expiry_warn_days" => {
            let actor = auth::require(ctx, "settings.system").await?;
            match v.parse::<u32>() {
                Ok(n) if n <= 90 => Some(actor),
                _ => return Err(Error::Invalid("expiry_warn_days must be 0–90".into())),
            }
        }
        "os_notifications" | "barcode_lookup" | "auto_print" | "block_expired" | "backup_auto" => {
            let actor = auth::require(ctx, "settings.system").await?;
            flag(v)?;
            Some(actor)
        }
        // Scale label layout (see scale.rs); a blank prefix switches it off
        "scale_prefix" | "scale_code_length" => {
            let actor = auth::require(ctx, "settings.system").await?;
            let (prefix, len) = if args.key == "scale_prefix" {
                (v.trim().to_owned(), get(ctx, "scale_code_length", "5").await?.parse().unwrap_or(5))
            } else {
                (get(ctx, "scale_prefix", "20").await?, v.parse().unwrap_or(0))
            };
            if !(prefix.is_empty() && args.key == "scale_prefix") && !crate::scale::layout_ok(&prefix, len) {
                return Err(Error::Rule("scale_layout"));
            }
            Some(actor)
        }
        "scale_mode" => {
            let actor = auth::require(ctx, "settings.system").await?;
            if !matches!(v, "weight" | "price") {
                return Err(Error::Invalid("scale_mode must be weight or price".into()));
            }
            Some(actor)
        }
        "scale_decimals" => {
            let actor = auth::require(ctx, "settings.system").await?;
            if !matches!(v, "0" | "1" | "2" | "3") {
                return Err(Error::Invalid("scale_decimals must be 0–3".into()));
            }
            Some(actor)
        }
        // How many daily backups to keep
        "backup_keep" => {
            let actor = auth::require(ctx, "settings.system").await?;
            match v.parse::<u32>() {
                Ok(n) if (1..=365).contains(&n) => Some(actor),
                _ => return Err(Error::Invalid("backup_keep must be 1–365".into())),
            }
        }
        "receipt_width" => {
            let actor = auth::require(ctx, "settings.system").await?;
            if !matches!(v, "58" | "80") {
                return Err(Error::Invalid("receipt_width must be 58 or 80".into()));
            }
            Some(actor)
        }
        "opening_float" => {
            let actor = auth::require(ctx, "settings.system").await?;
            match v.parse::<f64>() {
                Ok(n) if n >= 0.0 => Some(actor),
                _ => return Err(Error::Rule("negative_amount")),
            }
        }
        // "", "system:<printer name>" or "tcp:<host>:<port>"
        "receipt_printer" => {
            let actor = auth::require(ctx, "settings.system").await?;
            let ok = v.is_empty()
                || v.strip_prefix("system:").is_some_and(|n| !n.trim().is_empty())
                || v.strip_prefix("tcp:").is_some_and(|a| a.rsplit_once(':').is_some_and(|(h, p)| !h.is_empty() && p.parse::<u16>().is_ok()));
            if !ok {
                return Err(Error::Rule("printer_address"));
            }
            Some(actor)
        }
        other => return Err(Error::Invalid(format!("unknown setting {other}"))),
    };
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, $3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(&args.key)
    .bind(v)
    .bind(now())
    .execute(&ctx.pool)
    .await?;
    if let Some(actor) = actor {
        audit::log(ctx, Some(actor.user_id), "setting", "settings", None, &serde_json::json!({ args.key: v })).await?;
    }
    Ok(())
}

pub async fn get(ctx: &Ctx, key: &str, default: &str) -> Result<String> {
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT value FROM settings WHERE key = $1")
        .bind(key)
        .fetch_optional(&ctx.pool)
        .await?;
    Ok(row.and_then(|(v,)| v).unwrap_or_else(|| default.to_owned()))
}

pub const SHOP_TYPES: &[&str] = &["mini_market", "supermarket", "other"];

/// Mirrors `ShopProfile` in src/db/shop.ts. Names are in the shop's language.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ShopProfileInput {
    pub name: String,
    pub shop_type: String,
    pub logo_media_id: Option<i64>,
    pub phone: Option<String>,
    pub phone_alt: Option<String>,
    pub address: Option<String>,
    pub tax_number: Option<String>,
    pub receipt_footer: Option<String>,
    pub currency: String,
}

impl ShopProfileInput {
    #[cfg(test)]
    pub fn named(name: &str) -> Self {
        Self {
            name: name.into(),
            shop_type: "mini_market".into(),
            currency: "IQD".into(),
            ..Default::default()
        }
    }
}

/// Blank strings are stored as NULL.
fn opt(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

pub async fn upsert_shop_profile(ctx: &Ctx, p: &ShopProfileInput) -> Result<()> {
    if p.name.trim().is_empty() {
        return Err(Error::Rule("shop_name_required"));
    }
    if !matches!(p.currency.as_str(), "IQD" | "USD") {
        return Err(Error::Invalid(format!("unsupported currency {}", p.currency)));
    }
    if !SHOP_TYPES.contains(&p.shop_type.as_str()) {
        return Err(Error::Invalid(format!("unknown shop type {}", p.shop_type)));
    }
    sqlx::query(
        "INSERT INTO shop_profile (id, name, shop_type, logo_media_id, phone, phone_alt, address,
            tax_number, receipt_footer, currency, updated_at)
         VALUES (1, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name, shop_type = excluded.shop_type,
            logo_media_id = excluded.logo_media_id, phone = excluded.phone,
            phone_alt = excluded.phone_alt, address = excluded.address,
            tax_number = excluded.tax_number, receipt_footer = excluded.receipt_footer,
            currency = excluded.currency, updated_at = excluded.updated_at",
    )
    .bind(p.name.trim())
    .bind(&p.shop_type)
    .bind(p.logo_media_id)
    .bind(opt(&p.phone))
    .bind(opt(&p.phone_alt))
    .bind(opt(&p.address))
    .bind(opt(&p.tax_number))
    .bind(opt(&p.receipt_footer))
    .bind(&p.currency)
    .bind(now())
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct SaveShopArgs {
    pub profile: ShopProfileInput,
}

pub async fn save_shop_profile(ctx: &Ctx, args: SaveShopArgs) -> Result<()> {
    let actor = auth::require(ctx, "settings.shop").await?;
    upsert_shop_profile(ctx, &args.profile).await?;
    audit::log(ctx, Some(actor.user_id), "shop.update", "shop_profile", Some(1), &serde_json::to_value(&args.profile)?).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::ctx::test_support;

    #[test]
    fn language_is_open_tax_rate_is_not() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            set_setting(&ctx, SetSettingArgs { key: "language".into(), value: "ar".into() })
                .await
                .unwrap();
            // …but only until setup is done
            let r = set_setting(&ctx, SetSettingArgs { key: "tax_rate".into(), value: "5".into() }).await;
            assert!(matches!(r, Err(Error::NotSignedIn)));

            setup_owner(&ctx).await;
            let r = set_setting(&ctx, SetSettingArgs { key: "language".into(), value: "en".into() }).await;
            assert!(matches!(r, Err(Error::Rule("language_locked"))));
            set_setting(&ctx, SetSettingArgs { key: "tax_rate".into(), value: "5".into() })
                .await
                .unwrap();
            let r = set_setting(&ctx, SetSettingArgs { key: "tax_rate".into(), value: "150".into() }).await;
            assert!(matches!(r, Err(Error::Rule("tax_rate_range"))));
            let r = set_setting(&ctx, SetSettingArgs { key: "secret".into(), value: "1".into() }).await;
            assert!(matches!(r, Err(Error::Invalid(_))));
        });
    }
}
