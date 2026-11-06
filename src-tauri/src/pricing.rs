//! Prices beyond the single selling price: wholesale, cheaper-at-quantity,
//! promotions, and a history of every price and cost change.
//!
//! Checkout uses `unit_price` and `best_promotion`; the register mirrors
//! the same rules in src/lib/pricing.ts for its preview, but what's charged
//! is always worked out here.

use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, Transaction};

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::sales::{round_money, shop_currency};

// ------------------------------------------------------------ price levels

/// The price of one unit for a sale at `level` with `qty` on the line:
/// wholesale when asked for and set; otherwise retail, or a cheaper
/// quantity price when the line is big enough.
pub async fn unit_price(tx: &mut Transaction<'_, Sqlite>, product_id: i64, retail: f64, wholesale: Option<f64>, level: &str, qty: f64) -> Result<f64> {
    if level == "wholesale" {
        return Ok(wholesale.unwrap_or(retail));
    }
    let tier: Option<(f64,)> = sqlx::query_as(
        "SELECT CAST(price AS REAL) FROM product_qty_prices WHERE product_id = $1 AND min_qty <= $2 ORDER BY min_qty DESC LIMIT 1",
    )
    .bind(product_id)
    .bind(qty + 1e-9)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(tier.map_or(retail, |(p,)| p.min(retail)))
}

// ------------------------------------------------------------ promotions

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Percent(f64),
    /// Buy `buy`, get `free` free
    Multibuy { buy: f64, free: f64 },
    /// `qty` for `price`
    Bundle { qty: f64, price: f64 },
}

#[derive(Debug, Clone)]
pub struct Promotion {
    pub id: i64,
    pub name: String,
    pub kind: Kind,
}

/// What a promotion takes off a line of `qty` at `price` each.
pub fn promo_discount(kind: &Kind, qty: f64, price: f64, currency: &str) -> f64 {
    let d = match *kind {
        Kind::Percent(p) => price * qty * p.clamp(0.0, 100.0) / 100.0,
        Kind::Multibuy { buy, free } if buy > 0.0 && free > 0.0 => (qty / (buy + free)).floor() * free * price,
        Kind::Bundle { qty: n, price: bundle } if n > 1.0 => ((qty / n).floor() * (n * price - bundle)).max(0.0),
        _ => 0.0,
    };
    round_money(d, currency)
}

/// The promotions running today that cover this product (directly or by
/// its category or the category's parent).
pub async fn promotions_for(tx: &mut Transaction<'_, Sqlite>, product_id: i64, today: &str) -> Result<Vec<Promotion>> {
    let rows: Vec<(i64, String, String, Option<f64>, Option<f64>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT DISTINCT pr.id, pr.name, pr.kind, pr.percent, pr.buy_qty, pr.free_qty, pr.bundle_price
         FROM promotions pr
         JOIN promotion_targets t ON t.promotion_id = pr.id
         JOIN products p ON p.id = $1
         LEFT JOIN categories c ON c.id = p.category_id
         WHERE pr.is_active = 1
           AND (pr.starts_on IS NULL OR pr.starts_on <= $2)
           AND (pr.ends_on IS NULL OR pr.ends_on >= $2)
           AND (t.product_id = p.id OR t.category_id = p.category_id OR t.category_id = c.parent_id)",
    )
    .bind(product_id)
    .bind(today)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, name, kind, pct, buy, free, bundle)| {
            let kind = match kind.as_str() {
                "percent" => Kind::Percent(pct?),
                "multibuy" => Kind::Multibuy { buy: buy?, free: free? },
                "bundle" => Kind::Bundle { qty: buy?, price: bundle? },
                _ => return None,
            };
            Some(Promotion { id, name, kind })
        })
        .collect())
}

/// The promotion worth the most on this line, if any is worth anything.
pub fn best_promotion(promos: &[Promotion], qty: f64, price: f64, currency: &str) -> Option<(Promotion, f64)> {
    promos
        .iter()
        .map(|p| (p.clone(), promo_discount(&p.kind, qty, price, currency)))
        .filter(|(_, d)| *d > 0.0)
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromotionInput {
    pub name: String,
    /// percent | multibuy | bundle
    pub kind: String,
    pub percent: Option<f64>,
    pub buy_qty: Option<f64>,
    pub free_qty: Option<f64>,
    pub bundle_price: Option<f64>,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
    #[serde(default)]
    pub product_ids: Vec<i64>,
    #[serde(default)]
    pub category_ids: Vec<i64>,
    #[serde(default = "yes")]
    pub is_active: bool,
}

fn yes() -> bool {
    true
}

fn date(v: &Option<String>) -> Result<Option<String>> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(crate::stock::parse_date).transpose()
}

fn validate(p: &PromotionInput) -> Result<(Option<String>, Option<String>)> {
    if p.name.trim().is_empty() {
        return Err(Error::Rule("promotion_name_required"));
    }
    let ok = match p.kind.as_str() {
        "percent" => p.percent.is_some_and(|x| x > 0.0 && x <= 100.0),
        "multibuy" => p.buy_qty.is_some_and(|x| x >= 1.0) && p.free_qty.is_some_and(|x| x >= 1.0),
        "bundle" => p.buy_qty.is_some_and(|x| x >= 2.0) && p.bundle_price.is_some_and(|x| x >= 0.0),
        _ => false,
    };
    if !ok {
        return Err(Error::Rule("promotion_values"));
    }
    if p.product_ids.is_empty() && p.category_ids.is_empty() {
        return Err(Error::Rule("promotion_targets"));
    }
    let (from, to) = (date(&p.starts_on)?, date(&p.ends_on)?);
    if let (Some(a), Some(b)) = (&from, &to) {
        if b < a {
            return Err(Error::Rule("promotion_dates"));
        }
    }
    Ok((from, to))
}

#[derive(Debug, Deserialize)]
pub struct SavePromotionArgs {
    /// None creates one
    pub id: Option<i64>,
    pub promotion: PromotionInput,
}

pub async fn save_promotion(ctx: &Ctx, args: SavePromotionArgs) -> Result<i64> {
    let actor = auth::require(ctx, "catalog.promotions").await?;
    let p = &args.promotion;
    let (from, to) = validate(p)?;
    // Only the numbers the kind uses are kept
    let percent = (p.kind == "percent").then_some(p.percent).flatten();
    let buy = (p.kind != "percent").then_some(p.buy_qty).flatten();
    let free = (p.kind == "multibuy").then_some(p.free_qty).flatten();
    let bundle = (p.kind == "bundle").then_some(p.bundle_price).flatten();
    let mut tx = ctx.pool.begin().await?;
    let id = match args.id {
        Some(id) => {
            let done = sqlx::query(
                "UPDATE promotions SET name = $1, kind = $2, percent = $3, buy_qty = $4, free_qty = $5, bundle_price = $6,
                    starts_on = $7, ends_on = $8, is_active = $9 WHERE id = $10",
            )
            .bind(p.name.trim())
            .bind(&p.kind)
            .bind(percent)
            .bind(buy)
            .bind(free)
            .bind(bundle)
            .bind(&from)
            .bind(&to)
            .bind(p.is_active)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            if done.rows_affected() == 0 {
                return Err(Error::NotFound);
            }
            sqlx::query("DELETE FROM promotion_targets WHERE promotion_id = $1").bind(id).execute(&mut *tx).await?;
            id
        }
        None => sqlx::query(
            "INSERT INTO promotions (name, kind, percent, buy_qty, free_qty, bundle_price, starts_on, ends_on, is_active, created_by, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(p.name.trim())
        .bind(&p.kind)
        .bind(percent)
        .bind(buy)
        .bind(free)
        .bind(bundle)
        .bind(&from)
        .bind(&to)
        .bind(p.is_active)
        .bind(actor.user_id)
        .bind(now())
        .execute(&mut *tx)
        .await?
        .last_insert_rowid(),
    };
    for pid in &p.product_ids {
        sqlx::query("INSERT INTO promotion_targets (promotion_id, product_id) VALUES ($1, $2)").bind(id).bind(pid).execute(&mut *tx).await?;
    }
    for cid in &p.category_ids {
        sqlx::query("INSERT INTO promotion_targets (promotion_id, category_id) VALUES ($1, $2)").bind(id).bind(cid).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "promotion.save", "promotions", Some(id), &serde_json::to_value(p)?).await?;
    Ok(id)
}

#[derive(Debug, Deserialize)]
pub struct IdArgs {
    pub id: i64,
}

/// Deletes a promotion no sale used; one that was used is switched off so
/// past receipts keep making sense.
pub async fn delete_promotion(ctx: &Ctx, args: IdArgs) -> Result<()> {
    let actor = auth::require(ctx, "catalog.promotions").await?;
    let (used,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sale_items WHERE promotion_id = $1").bind(args.id).fetch_one(&ctx.pool).await?;
    if used > 0 {
        sqlx::query("UPDATE promotions SET is_active = 0 WHERE id = $1").bind(args.id).execute(&ctx.pool).await?;
    } else {
        sqlx::query("DELETE FROM promotions WHERE id = $1").bind(args.id).execute(&ctx.pool).await?;
    }
    audit::log(ctx, Some(actor.user_id), "promotion.delete", "promotions", Some(args.id), &serde_json::json!({ "archived": used > 0 })).await
}

// ------------------------------------------------------------ history

/// Records changed prices/cost of a product. Call before writing the new
/// values, with the new ones; unchanged fields are skipped.
pub async fn record_changes(
    tx: &mut Transaction<'_, Sqlite>,
    product_id: i64,
    price: Option<f64>,
    wholesale: Option<Option<f64>>,
    cost: Option<f64>,
    source: &str,
    user_id: i64,
) -> Result<()> {
    let old: Option<(f64, Option<f64>, f64)> = sqlx::query_as(
        "SELECT CAST(selling_price AS REAL), CAST(wholesale_price AS REAL), CAST(cost_price AS REAL) FROM products WHERE id = $1",
    )
    .bind(product_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((old_price, old_wholesale, old_cost)) = old else { return Ok(()) };
    let at = now();
    let changed = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(x), Some(y)) => (x - y).abs() > 1e-9,
        (None, None) => false,
        _ => true,
    };
    let mut rows: Vec<(&str, Option<f64>, Option<f64>)> = Vec::new();
    if let Some(p) = price.filter(|p| changed(Some(*p), Some(old_price))) {
        rows.push(("price", Some(old_price), Some(p)));
    }
    if let Some(w) = wholesale.filter(|w| changed(*w, old_wholesale)) {
        rows.push(("wholesale", old_wholesale, w));
    }
    if let Some(c) = cost.filter(|c| changed(Some(*c), Some(old_cost))) {
        rows.push(("cost", Some(old_cost), Some(c)));
    }
    for (field, from, to) in rows {
        sqlx::query("INSERT INTO price_history (product_id, field, old_value, new_value, source, user_id, changed_at) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(product_id)
            .bind(field)
            .bind(from)
            .bind(to)
            .bind(source)
            .bind(user_id)
            .bind(&at)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

// ------------------------------------------------------------ bulk edit

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkPriceArgs {
    pub product_ids: Vec<i64>,
    /// percent | amount
    pub mode: String,
    /// +10 raises by 10 % (or 10 IQD); −5 lowers
    pub value: f64,
    /// Round the new price to a multiple of this (e.g. 250); 0 = shop rounding
    #[serde(default)]
    pub round_to: f64,
}

/// The new price for one product.
pub fn bulk_new_price(old: f64, mode: &str, value: f64, round_to: f64, currency: &str) -> f64 {
    let raw = if mode == "percent" { old * (1.0 + value / 100.0) } else { old + value };
    let rounded = if round_to > 0.0 { (raw / round_to).round() * round_to } else { round_money(raw, currency) };
    rounded.max(0.0)
}

pub async fn bulk_update_prices(ctx: &Ctx, args: BulkPriceArgs) -> Result<i64> {
    let actor = auth::require(ctx, "catalog.bulk_prices").await?;
    if !matches!(args.mode.as_str(), "percent" | "amount") || !args.value.is_finite() || args.value == 0.0 || !(args.round_to >= 0.0) {
        return Err(Error::Invalid("bad bulk price change".into()));
    }
    if args.mode == "percent" && args.value <= -100.0 {
        return Err(Error::Rule("negative_amount"));
    }
    let currency = shop_currency(ctx).await?;
    let mut tx = ctx.pool.begin().await?;
    let mut changed = 0;
    for id in &args.product_ids {
        let row: Option<(f64, bool)> = sqlx::query_as("SELECT CAST(selling_price AS REAL), open_price FROM products WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((old, open)) = row else { continue };
        if open {
            continue;
        }
        let new = bulk_new_price(old, &args.mode, args.value, args.round_to, &currency);
        if (new - old).abs() < 1e-9 {
            continue;
        }
        record_changes(&mut tx, *id, Some(new), None, None, "bulk", actor.user_id).await?;
        sqlx::query("UPDATE products SET selling_price = $1 WHERE id = $2").bind(new).bind(id).execute(&mut *tx).await?;
        changed += 1;
    }
    tx.commit().await?;
    audit::log(
        ctx,
        Some(actor.user_id),
        "prices.bulk",
        "products",
        None,
        &serde_json::json!({ "products": args.product_ids.len(), "changed": changed, "mode": args.mode, "value": args.value, "roundTo": args.round_to }),
    )
    .await?;
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_maths() {
        let iqd = "IQD";
        // 10 % off 3 × 750
        assert_eq!(promo_discount(&Kind::Percent(10.0), 3.0, 750.0, iqd), 225.0);
        // Buy 2 get 1: 3 → 1 free, 5 → 1 free, 6 → 2 free
        let b2g1 = Kind::Multibuy { buy: 2.0, free: 1.0 };
        assert_eq!(promo_discount(&b2g1, 2.0, 750.0, iqd), 0.0);
        assert_eq!(promo_discount(&b2g1, 3.0, 750.0, iqd), 750.0);
        assert_eq!(promo_discount(&b2g1, 5.0, 750.0, iqd), 750.0);
        assert_eq!(promo_discount(&b2g1, 6.0, 750.0, iqd), 1500.0);
        // 3 for 2000 (instead of 2250): 7 → two bundles
        let three = Kind::Bundle { qty: 3.0, price: 2000.0 };
        assert_eq!(promo_discount(&three, 7.0, 750.0, iqd), 500.0);
        // A "bundle" dearer than buying singly gives nothing
        assert_eq!(promo_discount(&Kind::Bundle { qty: 3.0, price: 3000.0 }, 3.0, 750.0, iqd), 0.0);

        let promos = vec![
            Promotion { id: 1, name: "10%".into(), kind: Kind::Percent(10.0) },
            Promotion { id: 2, name: "B2G1".into(), kind: b2g1 },
        ];
        assert_eq!(best_promotion(&promos, 2.0, 750.0, iqd).unwrap().0.id, 1);
        assert_eq!(best_promotion(&promos, 3.0, 750.0, iqd).unwrap().0.id, 2);
    }

    #[test]
    fn bulk_rounding() {
        assert_eq!(bulk_new_price(750.0, "percent", 10.0, 0.0, "IQD"), 825.0);
        assert_eq!(bulk_new_price(750.0, "percent", 10.0, 250.0, "IQD"), 750.0);
        assert_eq!(bulk_new_price(1000.0, "percent", 15.0, 250.0, "IQD"), 1250.0);
        assert_eq!(bulk_new_price(1000.0, "amount", -250.0, 0.0, "IQD"), 750.0);
        assert_eq!(bulk_new_price(100.0, "amount", -500.0, 0.0, "IQD"), 0.0);
    }
}
