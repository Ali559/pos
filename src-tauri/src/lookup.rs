//! Filling the product form from a scanned barcode.
//!
//! First the shop's own catalog (a known barcode opens that product). Then,
//! if the `barcode_lookup` setting is on and the till is online, Open Food
//! Facts, a free product database: name, brand, size, category tags and a
//! front photo. Offline or unknown barcodes simply return nothing to fill.
//! Only the barcode is sent.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::auth;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::media::{self, MediaFile};
use crate::settings;

const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
const USER_AGENT: &str = "POS-till/0.1 (shop point of sale; barcode lookup)";

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lookup {
    /// The barcode already belongs to this product
    pub existing_product_id: Option<i64>,
    pub name: Option<String>,
    pub brand: Option<String>,
    /// e.g. "330 ml"
    pub quantity: Option<String>,
    /// Open Food Facts tags like "en:carbonated-drinks", for matching a category
    pub category_tags: Vec<String>,
    pub image: Option<MediaFile>,
    /// "local" | "openfoodfacts" | "none"
    pub source: &'static str,
    /// The online lookup was attempted but failed (no connection, timeout)
    pub offline: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct LookupArgs {
    pub barcode: String,
}

fn valid_barcode(code: &str) -> bool {
    (4..=32).contains(&code.len()) && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The useful fields of an Open Food Facts product, preferring the shop's language.
pub fn parse_off(body: &Value, lang: &str) -> Option<(Option<String>, Option<String>, Option<String>, Vec<String>, Option<String>)> {
    if body["status"].as_i64() != Some(1) {
        return None;
    }
    let p = &body["product"];
    let text = |k: &str| p[k].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
    let lang_key = format!("product_name_{lang}");
    let name = text(&lang_key)
        .or_else(|| text("product_name"))
        .or_else(|| text("product_name_en"))
        .or_else(|| text("generic_name"));
    let brand = text("brands").map(|b| b.split(',').next().unwrap_or_default().trim().to_owned());
    // "400 g e": the trailing ℮ is the EU estimated-quantity mark
    let quantity = text("quantity").map(|q| {
        let q = q.trim_end_matches('℮').trim_end();
        q.strip_suffix(" e").unwrap_or(q).trim_end().to_owned()
    });
    let tags = p["categories_tags"]
        .as_array()
        .map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    let image = text("image_front_url").or_else(|| text("image_url"));
    Some((name, brand, quantity, tags, image))
}

/// "Brand Name Size", without repeating parts the name already has.
pub fn display_name(name: Option<&str>, brand: Option<&str>, quantity: Option<&str>) -> Option<String> {
    let mut out = name?.to_owned();
    if let Some(b) = brand.filter(|b| !out.to_lowercase().contains(&b.to_lowercase())) {
        out = format!("{b} {out}");
    }
    if let Some(q) = quantity.filter(|q| !out.to_lowercase().contains(&q.to_lowercase().replace(' ', ""))
        && !out.to_lowercase().contains(&q.to_lowercase()))
    {
        out = format!("{out} {q}");
    }
    Some(out)
}

pub async fn lookup_barcode(ctx: &Ctx, args: LookupArgs) -> Result<Lookup> {
    auth::require(ctx, "inventory.edit").await?;
    let code = args.barcode.trim();
    if !valid_barcode(code) {
        return Err(Error::Invalid("not a barcode".into()));
    }
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM products WHERE barcode = $1
         UNION SELECT product_id FROM product_barcodes WHERE barcode = $1 LIMIT 1",
    )
    .bind(code)
    .fetch_optional(&ctx.pool)
    .await?;
    if let Some((id,)) = existing {
        return Ok(Lookup { existing_product_id: Some(id), source: "local", ..Default::default() });
    }
    if settings::get(ctx, "barcode_lookup", "1").await? != "1" {
        return Ok(Lookup { source: "none", ..Default::default() });
    }

    let client = reqwest::Client::builder().timeout(TIMEOUT).user_agent(USER_AGENT).build()
        .map_err(|e| Error::Invalid(e.to_string()))?;
    let url = format!(
        "https://world.openfoodfacts.org/api/v2/product/{code}.json?fields=product_name,product_name_en,product_name_ar,product_name_ku,generic_name,brands,quantity,categories_tags,image_front_url,image_url"
    );
    let body: Value = match client.get(&url).send().await {
        Ok(r) => r.json().await.unwrap_or(Value::Null),
        Err(_) => return Ok(Lookup { source: "none", offline: true, ..Default::default() }),
    };
    let lang = settings::get(ctx, "language", "ku").await?;
    let Some((name, brand, quantity, tags, image_url)) = parse_off(&body, &lang) else {
        return Ok(Lookup { source: "none", ..Default::default() });
    };

    // The photo goes through the same pipeline as uploads (WebP + blurhash);
    // a failed download just means no photo
    let mut image = None;
    if let Some(u) = image_url.filter(|u| u.starts_with("https://")) {
        if let Ok(resp) = client.get(&u).send().await {
            let is_image = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|t| t.starts_with("image/"));
            if is_image {
                if let Ok(bytes) = resp.bytes().await {
                    if bytes.len() <= MAX_IMAGE_BYTES {
                        image = media::upload(ctx, bytes.to_vec(), Some(format!("off-{code}"))).await.ok();
                    }
                }
            }
        }
    }

    Ok(Lookup {
        existing_product_id: None,
        name: display_name(name.as_deref(), brand.as_deref(), quantity.as_deref()),
        brand,
        quantity,
        category_tags: tags,
        image,
        source: "openfoodfacts",
        offline: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_and_prefers_the_shop_language() {
        let body = json!({ "status": 1, "product": {
            "product_name": "Coca-Cola", "product_name_ar": "كوكا كولا", "brands": "Coca-Cola, The Coca-Cola Company",
            "quantity": "330 ml ℮", "categories_tags": ["en:beverages", "en:carbonated-drinks"],
            "image_front_url": "https://images.openfoodfacts.org/x.jpg"
        }});
        let (name, brand, qty, tags, img) = parse_off(&body, "ar").unwrap();
        assert_eq!(name.as_deref(), Some("كوكا كولا"));
        assert_eq!(brand.as_deref(), Some("Coca-Cola"));
        assert_eq!(tags.len(), 2);
        assert!(img.is_some());
        // Falls back to the generic name when there's no language match
        let (name, ..) = parse_off(&body, "ku").unwrap();
        assert_eq!(name.as_deref(), Some("Coca-Cola"));
        assert_eq!(qty.as_deref(), Some("330 ml"));
        assert!(parse_off(&json!({ "status": 0 }), "en").is_none());
        let (.., qty, _, _) = parse_off(&json!({ "status": 1, "product": { "quantity": "400 g e" } }), "en").unwrap();
        assert_eq!(qty.as_deref(), Some("400 g"));
        let (.., qty, _, _) = parse_off(&json!({ "status": 1, "product": { "quantity": "1 piece" } }), "en").unwrap();
        assert_eq!(qty.as_deref(), Some("1 piece"));
    }

    #[test]
    fn display_name_adds_brand_and_size_once() {
        assert_eq!(display_name(Some("Cola"), Some("Pepsi"), Some("330 ml")).as_deref(), Some("Pepsi Cola 330 ml"));
        assert_eq!(display_name(Some("Pepsi Cola 330ml"), Some("Pepsi"), Some("330 ml")).as_deref(), Some("Pepsi Cola 330ml"));
        assert_eq!(display_name(None, Some("Pepsi"), None), None);
    }

    #[test]
    fn barcode_shape() {
        assert!(valid_barcode("5449000000996"));
        assert!(!valid_barcode("../etc"));
        assert!(!valid_barcode("12"));
    }
}
