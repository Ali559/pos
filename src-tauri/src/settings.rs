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

/// Only known keys, each with its own rule for who may change it.
pub async fn set_setting(ctx: &Ctx, args: SetSettingArgs) -> Result<()> {
    let actor = match args.key.as_str() {
        // Anyone, even before sign-in: the language picker is on the login screen
        "language" => {
            if !matches!(args.value.as_str(), "ku" | "ar" | "en") {
                return Err(Error::Invalid(format!("unknown language {}", args.value)));
            }
            None
        }
        "expiry_warn_days" => {
            let actor = auth::require(ctx, "settings.system").await?;
            match args.value.parse::<u32>() {
                Ok(v) if v <= 90 => Some(actor),
                _ => return Err(Error::Invalid("expiry_warn_days must be 0–90".into())),
            }
        }
        "os_notifications" => {
            let actor = auth::require(ctx, "settings.system").await?;
            if !matches!(args.value.as_str(), "0" | "1") {
                return Err(Error::Invalid("os_notifications must be 0 or 1".into()));
            }
            Some(actor)
        }
        "tax_rate" => {
            let actor = auth::require(ctx, "settings.system").await?;
            match args.value.parse::<f64>() {
                Ok(v) if (0.0..=100.0).contains(&v) => Some(actor),
                _ => return Err(Error::Rule("tax_rate_range")),
            }
        }
        other => return Err(Error::Invalid(format!("unknown setting {other}"))),
    };
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, $3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(&args.key)
    .bind(&args.value)
    .bind(now())
    .execute(&ctx.pool)
    .await?;
    if let Some(actor) = actor {
        audit::log(ctx, Some(actor.user_id), "setting", "settings", None, &serde_json::json!({ args.key: args.value })).await?;
    }
    Ok(())
}

/// Mirrors `ShopProfile` in src/db/shop.ts.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ShopProfileInput {
    pub name_en: String,
    pub name_ku: String,
    pub name_ar: Option<String>,
    pub logo_media_id: Option<i64>,
    pub phone: Option<String>,
    pub phone_alt: Option<String>,
    pub address_en: Option<String>,
    pub address_ku: Option<String>,
    pub address_ar: Option<String>,
    pub tax_number: Option<String>,
    pub receipt_footer_en: Option<String>,
    pub receipt_footer_ku: Option<String>,
    pub receipt_footer_ar: Option<String>,
    pub currency: String,
}

impl ShopProfileInput {
    #[cfg(test)]
    pub fn named(en: &str, ku: &str) -> Self {
        Self {
            name_en: en.into(),
            name_ku: ku.into(),
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
    let (name_en, name_ku) = (p.name_en.trim(), p.name_ku.trim());
    if name_en.is_empty() || name_ku.is_empty() {
        return Err(Error::Rule("shop_names_required"));
    }
    if !matches!(p.currency.as_str(), "IQD" | "USD") {
        return Err(Error::Invalid(format!("unsupported currency {}", p.currency)));
    }
    sqlx::query(
        "INSERT INTO shop_profile (id, name_en, name_ku, name_ar, logo_media_id, phone, phone_alt,
            address_en, address_ku, address_ar, tax_number,
            receipt_footer_en, receipt_footer_ku, receipt_footer_ar, currency, updated_at)
         VALUES (1, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
         ON CONFLICT(id) DO UPDATE SET
            name_en = excluded.name_en, name_ku = excluded.name_ku, name_ar = excluded.name_ar,
            logo_media_id = excluded.logo_media_id, phone = excluded.phone,
            phone_alt = excluded.phone_alt, address_en = excluded.address_en,
            address_ku = excluded.address_ku, address_ar = excluded.address_ar,
            tax_number = excluded.tax_number, receipt_footer_en = excluded.receipt_footer_en,
            receipt_footer_ku = excluded.receipt_footer_ku,
            receipt_footer_ar = excluded.receipt_footer_ar, currency = excluded.currency,
            updated_at = excluded.updated_at",
    )
    .bind(name_en)
    .bind(name_ku)
    .bind(opt(&p.name_ar))
    .bind(p.logo_media_id)
    .bind(opt(&p.phone))
    .bind(opt(&p.phone_alt))
    .bind(opt(&p.address_en))
    .bind(opt(&p.address_ku))
    .bind(opt(&p.address_ar))
    .bind(opt(&p.tax_number))
    .bind(opt(&p.receipt_footer_en))
    .bind(opt(&p.receipt_footer_ku))
    .bind(opt(&p.receipt_footer_ar))
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
            let r = set_setting(&ctx, SetSettingArgs { key: "tax_rate".into(), value: "5".into() }).await;
            assert!(matches!(r, Err(Error::NotSignedIn)));

            setup_owner(&ctx).await;
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
