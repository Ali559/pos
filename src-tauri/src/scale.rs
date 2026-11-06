//! What a scanned code means at the till: a product, one of its pack sizes,
//! or a label printed by a scale.
//!
//! Scale labels are EAN-13 codes in the in-store range: a prefix (e.g. 20),
//! the product's item code (PLU), then either the weight or the price, and a
//! check digit. The layout is a shop setting (`scale_*`). Checkout decodes
//! the label again itself, so a price printed on a label is never taken from
//! the screen.

use serde::{Deserialize, Serialize};

use crate::auth;
use crate::ctx::Ctx;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Weight,
    Price,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub prefix: String,
    pub code_length: usize,
    pub mode: Mode,
    pub decimals: u32,
}

impl Config {
    /// Digits left for the weight or price
    fn value_length(&self) -> usize {
        12usize.saturating_sub(self.prefix.len() + self.code_length)
    }
}

/// None when scale barcodes are switched off (blank prefix).
pub async fn config(ctx: &Ctx) -> Result<Option<Config>> {
    let get = |k: &'static str, d: &'static str| crate::settings::get(ctx, k, d);
    let prefix = get("scale_prefix", "20").await?.trim().to_owned();
    if prefix.is_empty() {
        return Ok(None);
    }
    Ok(Some(Config {
        prefix,
        code_length: get("scale_code_length", "5").await?.parse().unwrap_or(5),
        mode: if get("scale_mode", "weight").await? == "price" { Mode::Price } else { Mode::Weight },
        decimals: get("scale_decimals", "3").await?.parse().unwrap_or(3),
    }))
}

/// Validates a layout for `set_setting`. Prefix 1–3 digits, item code 3–6,
/// at least 4 digits left for the value.
pub fn layout_ok(prefix: &str, code_length: usize) -> bool {
    (1..=3).contains(&prefix.len())
        && prefix.bytes().all(|b| b.is_ascii_digit())
        && (3..=6).contains(&code_length)
        && 12 >= prefix.len() + code_length + 4
}

pub fn ean13_ok(code: &str) -> bool {
    let d: Vec<u32> = code.chars().filter_map(|c| c.to_digit(10)).collect();
    if d.len() != 13 || code.len() != 13 {
        return false;
    }
    let sum: u32 = d[..12].iter().enumerate().map(|(i, v)| if i % 2 == 0 { *v } else { v * 3 }).sum();
    (10 - sum % 10) % 10 == d[12]
}

/// Item code with leading zeros dropped, so "00042" and "42" match.
pub fn normalize_plu(code: &str) -> String {
    let t = code.trim_start_matches('0');
    if t.is_empty() { "0".into() } else { t.into() }
}

/// (item code, weight in kg or price) from a scale label.
pub fn decode(cfg: &Config, code: &str) -> Option<(String, f64)> {
    if !code.starts_with(&cfg.prefix) || !ean13_ok(code) || cfg.value_length() < 4 {
        return None;
    }
    let rest = &code[cfg.prefix.len()..12];
    let (plu, value) = rest.split_at(cfg.code_length);
    let value: f64 = value.parse().ok()?;
    Some((normalize_plu(plu), value / 10f64.powi(cfg.decimals as i32)))
}

/// What a scale label sells: quantity in the product's unit and, for price
/// labels, the line total. `price` is the product's price per unit.
pub fn line_from_label(cfg: &Config, value: f64, price: f64) -> (f64, Option<f64>) {
    match cfg.mode {
        Mode::Weight => (value, None),
        Mode::Price => {
            let qty = if price > 0.0 { (value / price * 1000.0).round() / 1000.0 } else { 0.0 };
            (qty, Some(value))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ScanArgs {
    pub code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub product_id: i64,
    /// A pack size's barcode
    pub unit_id: Option<i64>,
    /// In the pack (unit_id) or the product's own unit (weight from a label)
    pub quantity: f64,
    /// A price label's total
    pub amount: Option<f64>,
    /// Set for scale labels; checkout decodes it again
    pub scale_barcode: Option<String>,
}

/// Looks a code up: a product's barcode or SKU, then pack barcodes, then
/// scale labels. Active products only. None if nothing matches.
pub async fn resolve_scan(ctx: &Ctx, args: ScanArgs) -> Result<Option<ScanResult>> {
    auth::current(ctx).await?;
    let code = args.code.trim();
    if code.is_empty() {
        return Ok(None);
    }
    let product: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM products WHERE is_active = 1
           AND (barcode = $1 OR sku = $1 OR id IN (SELECT product_id FROM product_barcodes WHERE barcode = $1))
         LIMIT 1",
    )
    .bind(code)
    .fetch_optional(&ctx.pool)
    .await?;
    if let Some((id,)) = product {
        return Ok(Some(ScanResult { product_id: id, unit_id: None, quantity: 1.0, amount: None, scale_barcode: None }));
    }
    let unit: Option<(i64, i64)> = sqlx::query_as(
        "SELECT u.id, u.product_id FROM product_units u JOIN products p ON p.id = u.product_id
         WHERE u.barcode = $1 AND p.is_active = 1",
    )
    .bind(code)
    .fetch_optional(&ctx.pool)
    .await?;
    if let Some((unit_id, product_id)) = unit {
        return Ok(Some(ScanResult { product_id, unit_id: Some(unit_id), quantity: 1.0, amount: None, scale_barcode: None }));
    }
    let Some(cfg) = config(ctx).await? else { return Ok(None) };
    let Some((plu, value)) = decode(&cfg, code) else { return Ok(None) };
    let found: Option<(i64, f64)> = sqlx::query_as(
        "SELECT id, CAST(selling_price AS REAL) FROM products WHERE is_active = 1 AND plu = $1",
    )
    .bind(&plu)
    .fetch_optional(&ctx.pool)
    .await?;
    Ok(found.map(|(product_id, price)| {
        let (quantity, amount) = line_from_label(&cfg, value, price);
        ScanResult { product_id, unit_id: None, quantity, amount, scale_barcode: Some(code.to_owned()) }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_check(first12: &str) -> String {
        let d: Vec<u32> = first12.chars().map(|c| c.to_digit(10).unwrap()).collect();
        let sum: u32 = d.iter().enumerate().map(|(i, v)| if i % 2 == 0 { *v } else { v * 3 }).sum();
        format!("{first12}{}", (10 - sum % 10) % 10)
    }

    #[test]
    fn decodes_weight_and_price_labels() {
        let weight = Config { prefix: "20".into(), code_length: 5, mode: Mode::Weight, decimals: 3 };
        let label = with_check("200004201250"); // PLU 42, 1.250 kg
        assert_eq!(decode(&weight, &label), Some(("42".into(), 1.25)));
        assert_eq!(line_from_label(&weight, 1.25, 8000.0), (1.25, None));

        // Wrong check digit, wrong prefix, wrong length
        let mut bad = label.clone();
        bad.replace_range(12..13, if label.ends_with('0') { "1" } else { "0" });
        assert_eq!(decode(&weight, &bad), None);
        assert_eq!(decode(&weight, &with_check("210004201250")), None);
        assert_eq!(decode(&weight, "20000420125"), None);

        let price = Config { prefix: "2".into(), code_length: 6, mode: Mode::Price, decimals: 0 };
        let label = with_check("200004209000"); // PLU 42, 9000 IQD
        assert_eq!(decode(&price, &label), Some(("42".into(), 9000.0)));
        // 9000 at 12000/kg = 0.75 kg, charged exactly what the label says
        assert_eq!(line_from_label(&price, 9000.0, 12000.0), (0.75, Some(9000.0)));
    }

    #[test]
    fn layouts() {
        assert!(layout_ok("20", 5));
        assert!(layout_ok("2", 6));
        assert!(!layout_ok("", 5));
        assert!(!layout_ok("2a", 5));
        assert!(!layout_ok("200", 6)); // only 3 digits left
        assert_eq!(normalize_plu("00042"), "42");
        assert_eq!(normalize_plu("000"), "0");
    }
}
