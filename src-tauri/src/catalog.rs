//! Categories (two levels, each with an icon) and products.

use std::collections::HashSet;

use serde::Deserialize;
use sqlx::{Sqlite, Transaction};

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};

pub const UNITS: &[&str] = &["piece", "kg", "liter", "pack", "box"];

fn trimmed(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// "<set>:<name>", e.g. "tabler:bottle".
fn valid_icon(key: &str) -> bool {
    matches!(key.split_once(':'), Some((set, name))
        if !set.is_empty() && !name.is_empty()
            && key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-')))
}

fn unique_violation(e: &sqlx::Error) -> Option<String> {
    match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => Some(db.message().to_owned()),
        _ => None,
    }
}

// -------------------------------------------------------------- categories

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInput {
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub icon_key: String,
    pub color: Option<String>,
    pub image_media_id: Option<i64>,
}

async fn validate_category(ctx: &Ctx, c: &CategoryInput, id: Option<i64>) -> Result<()> {
    if c.name.trim().is_empty() {
        return Err(Error::Rule("category_name_required"));
    }
    if !valid_icon(c.icon_key.trim()) {
        return Err(Error::Rule("icon_required"));
    }
    if let Some(color) = trimmed(&c.color) {
        let hex = color.strip_prefix('#').unwrap_or_default();
        if hex.len() != 6 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(Error::Invalid(format!("not a #rrggbb color: {color}")));
        }
    }
    if let Some(parent) = c.parent_id {
        // Two levels only: the parent must itself be top-level…
        let row: Option<(Option<i64>,)> = sqlx::query_as("SELECT parent_id FROM categories WHERE id = $1")
            .bind(parent)
            .fetch_optional(&ctx.pool)
            .await?;
        match row {
            None => return Err(Error::NotFound),
            Some((Some(_),)) => return Err(Error::Rule("category_depth")),
            Some((None,)) if Some(parent) == id => return Err(Error::Rule("category_depth")),
            _ => {}
        }
        // …and a category that has children can't become a child
        if let Some(id) = id {
            let (children,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM categories WHERE parent_id = $1")
                .bind(id)
                .fetch_one(&ctx.pool)
                .await?;
            if children > 0 {
                return Err(Error::Rule("category_depth"));
            }
        }
    }
    Ok(())
}

fn map_category_unique(e: sqlx::Error) -> Error {
    match unique_violation(&e) {
        Some(_) => Error::Rule("category_name_taken"),
        None => e.into(),
    }
}

pub async fn create_category(ctx: &Ctx, c: CategoryInput) -> Result<i64> {
    let actor = auth::require(ctx, "catalog.categories").await?;
    validate_category(ctx, &c, None).await?;
    let (next,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(display_order), 0) + 1 FROM categories WHERE parent_id IS $1",
    )
    .bind(c.parent_id)
    .fetch_one(&ctx.pool)
    .await?;
    let id = sqlx::query(
        "INSERT INTO categories (name, description, parent_id, icon_key, color, image_media_id, display_order)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(c.name.trim())
    .bind(trimmed(&c.description))
    .bind(c.parent_id)
    .bind(c.icon_key.trim())
    .bind(trimmed(&c.color))
    .bind(c.image_media_id)
    .bind(next)
    .execute(&ctx.pool)
    .await
    .map_err(map_category_unique)?
    .last_insert_rowid();
    audit::log(ctx, Some(actor.user_id), "category.create", "categories", Some(id), &serde_json::json!({ "name": c.name })).await?;
    Ok(id)
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryArgs {
    pub id: i64,
    pub category: CategoryInput,
}

pub async fn update_category(ctx: &Ctx, args: UpdateCategoryArgs) -> Result<()> {
    let actor = auth::require(ctx, "catalog.categories").await?;
    let c = &args.category;
    validate_category(ctx, c, Some(args.id)).await?;
    let done = sqlx::query(
        "UPDATE categories SET name = $1, description = $2,
            parent_id = $3, icon_key = $4, color = $5, image_media_id = $6
         WHERE id = $7",
    )
    .bind(c.name.trim())
    .bind(trimmed(&c.description))
    .bind(c.parent_id)
    .bind(c.icon_key.trim())
    .bind(trimmed(&c.color))
    .bind(c.image_media_id)
    .bind(args.id)
    .execute(&ctx.pool)
    .await
    .map_err(map_category_unique)?;
    if done.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    audit::log(ctx, Some(actor.user_id), "category.update", "categories", Some(args.id), &serde_json::json!({ "name": c.name })).await
}

#[derive(Debug, Deserialize)]
pub struct MoveArgs {
    pub id: i64,
    /// -1 = earlier, 1 = later
    pub offset: i64,
}

/// Swaps a category with its neighbour among its siblings.
pub async fn move_category(ctx: &Ctx, args: MoveArgs) -> Result<()> {
    auth::require(ctx, "catalog.categories").await?;
    let (parent,): (Option<i64>,) = sqlx::query_as("SELECT parent_id FROM categories WHERE id = $1")
        .bind(args.id)
        .fetch_optional(&ctx.pool)
        .await?
        .ok_or(Error::NotFound)?;
    let siblings: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM categories WHERE parent_id IS $1 ORDER BY display_order, id",
    )
    .bind(parent)
    .fetch_all(&ctx.pool)
    .await?;
    let mut ids: Vec<i64> = siblings.into_iter().map(|(id,)| id).collect();
    let Some(pos) = ids.iter().position(|&id| id == args.id) else {
        return Err(Error::NotFound);
    };
    let target = pos as i64 + args.offset.signum();
    if target < 0 || target >= ids.len() as i64 {
        return Ok(());
    }
    ids.swap(pos, target as usize);
    // Renumber the whole sibling list so orders stay dense
    let mut tx = ctx.pool.begin().await?;
    for (i, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE categories SET display_order = $1 WHERE id = $2")
            .bind(i as i64 + 1)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ActiveArgs {
    pub id: i64,
    pub active: bool,
}

/// Archiving hides a category (and its subcategories) from the register and
/// pickers; nothing is deleted, so past sales still show it.
pub async fn set_category_active(ctx: &Ctx, args: ActiveArgs) -> Result<()> {
    let actor = auth::require(ctx, "catalog.categories").await?;
    sqlx::query("UPDATE categories SET is_active = $1 WHERE id = $2 OR (parent_id = $2 AND $1 = 0)")
        .bind(args.active)
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    audit::log(ctx, Some(actor.user_id), "category.active", "categories", Some(args.id), &serde_json::json!({ "active": args.active })).await
}

#[derive(Debug, Deserialize)]
pub struct IdArgs {
    pub id: i64,
}

/// Only empty categories can be deleted; others are archived instead.
pub async fn delete_category(ctx: &Ctx, args: IdArgs) -> Result<()> {
    let actor = auth::require(ctx, "catalog.categories").await?;
    let (used,): (i64,) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM categories WHERE parent_id = $1)
              + (SELECT COUNT(*) FROM products WHERE category_id = $1)",
    )
    .bind(args.id)
    .fetch_one(&ctx.pool)
    .await?;
    if used > 0 {
        return Err(Error::Rule("category_in_use"));
    }
    sqlx::query("DELETE FROM categories WHERE id = $1")
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    audit::log(ctx, Some(actor.user_id), "category.delete", "categories", Some(args.id), &serde_json::Value::Null).await
}

// ---------------------------------------------------------------- products

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductInput {
    pub name: String,
    /// Blank: generated as P-00042 from the id
    pub sku: Option<String>,
    pub category_id: Option<i64>,
    pub barcode: Option<String>,
    #[serde(default)]
    pub extra_barcodes: Vec<String>,
    pub cost_price: f64,
    pub selling_price: f64,
    pub min_stock: f64,
    pub unit: String,
    pub icon_key: Option<String>,
    /// In display order; the first is the cover
    #[serde(default)]
    pub media_ids: Vec<i64>,
    pub is_active: bool,
    /// Create only; later changes go through stock adjustments
    pub initial_stock: Option<f64>,
    /// Create only: expiry of the opening stock (YYYY-MM-DD)
    pub initial_expiry: Option<String>,
}

/// Primary barcode first, then the extras; trimmed, de-duplicated.
fn all_barcodes(p: &ProductInput) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for code in trimmed(&p.barcode).into_iter().chain(p.extra_barcodes.iter().map(|s| s.trim())) {
        if code.is_empty() {
            continue;
        }
        if !seen.insert(code.to_owned()) {
            return Err(Error::Rule("barcode_duplicate"));
        }
        out.push(code.to_owned());
    }
    Ok(out)
}

async fn validate_product(ctx: &Ctx, p: &ProductInput, id: Option<i64>) -> Result<Vec<String>> {
    if p.name.trim().is_empty() {
        return Err(Error::Rule("product_name_required"));
    }
    if p.cost_price < 0.0 || p.selling_price < 0.0 || p.min_stock < 0.0 || p.initial_stock.is_some_and(|s| s < 0.0) {
        return Err(Error::Rule("negative_amount"));
    }
    if !UNITS.contains(&p.unit.as_str()) {
        return Err(Error::Invalid(format!("unknown unit {}", p.unit)));
    }
    if let Some(icon) = trimmed(&p.icon_key) {
        if !valid_icon(icon) {
            return Err(Error::Invalid(format!("bad icon key {icon}")));
        }
    }
    if let Some(cat) = p.category_id {
        let found: Option<(i64,)> = sqlx::query_as("SELECT id FROM categories WHERE id = $1")
            .bind(cat)
            .fetch_optional(&ctx.pool)
            .await?;
        found.ok_or(Error::NotFound)?;
    }
    let codes = all_barcodes(p)?;
    for code in &codes {
        let (n,): (i64,) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM products WHERE barcode = $1 AND id IS NOT $2)
                  + (SELECT COUNT(*) FROM product_barcodes WHERE barcode = $1 AND product_id IS NOT $2)",
        )
        .bind(code)
        .bind(id)
        .fetch_one(&ctx.pool)
        .await?;
        if n > 0 {
            return Err(Error::Rule("barcode_taken"));
        }
    }
    Ok(codes)
}

fn map_product_unique(e: sqlx::Error) -> Error {
    match unique_violation(&e) {
        Some(msg) if msg.contains("sku") => Error::Rule("sku_taken"),
        Some(_) => Error::Rule("barcode_taken"),
        None => e.into(),
    }
}

async fn write_children(tx: &mut Transaction<'_, Sqlite>, id: i64, codes: &[String], media: &[i64]) -> Result<()> {
    sqlx::query("DELETE FROM product_barcodes WHERE product_id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    // The primary barcode lives on products.barcode; extras here
    for code in codes.iter().skip(1) {
        sqlx::query("INSERT INTO product_barcodes (product_id, barcode) VALUES ($1, $2)")
            .bind(id)
            .bind(code)
            .execute(&mut **tx)
            .await
            .map_err(map_product_unique)?;
    }
    sqlx::query("DELETE FROM product_media WHERE product_id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    let mut seen = HashSet::new();
    for (pos, media_id) in media.iter().filter(|m| seen.insert(**m)).enumerate() {
        sqlx::query("INSERT INTO product_media (product_id, media_id, position) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(media_id)
            .bind(pos as i64)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

pub async fn create_product(ctx: &Ctx, p: ProductInput) -> Result<i64> {
    let actor = auth::require(ctx, "inventory.edit").await?;
    let codes = validate_product(ctx, &p, None).await?;
    let stock = p.initial_stock.unwrap_or(0.0);
    let expiry = trimmed(&p.initial_expiry).map(crate::stock::parse_date).transpose()?;

    let mut tx = ctx.pool.begin().await?;
    // A placeholder SKU keeps the NOT NULL UNIQUE column happy until we know the id
    let sku = trimmed(&p.sku).map(str::to_owned).unwrap_or_else(|| format!("tmp-{}", uuid::Uuid::new_v4()));
    let id = sqlx::query(
        "INSERT INTO products (sku, name, category_id, barcode, cost_price,
            selling_price, min_stock_level, unit, icon_key, is_active, stock_quantity)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(&sku)
    .bind(p.name.trim())
    .bind(p.category_id)
    .bind(codes.first())
    .bind(p.cost_price)
    .bind(p.selling_price)
    .bind(p.min_stock)
    .bind(&p.unit)
    .bind(trimmed(&p.icon_key))
    .bind(p.is_active)
    .bind(stock)
    .execute(&mut *tx)
    .await
    .map_err(map_product_unique)?
    .last_insert_rowid();
    if trimmed(&p.sku).is_none() {
        sqlx::query("UPDATE products SET sku = $1 WHERE id = $2")
            .bind(format!("P-{id:05}"))
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_product_unique)?;
    }
    write_children(&mut tx, id, &codes, &p.media_ids).await?;
    if stock > 0.0 {
        sqlx::query(
            "INSERT INTO stock_adjustments (product_id, adjustment_type, quantity, reason, user_id, created_at)
             VALUES ($1, 'addition', $2, 'initial', $3, $4)",
        )
        .bind(id)
        .bind(stock)
        .bind(actor.user_id)
        .bind(now())
        .execute(&mut *tx)
        .await?;
        crate::stock::insert_batch(&mut tx, id, stock, expiry.as_deref(), Some(p.cost_price), Some("initial"), actor.user_id).await?;
    }
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "product.create", "products", Some(id), &serde_json::json!({ "name": p.name, "stock": stock })).await?;
    Ok(id)
}

#[derive(Debug, Deserialize)]
pub struct UpdateProductArgs {
    pub id: i64,
    pub product: ProductInput,
}

pub async fn update_product(ctx: &Ctx, args: UpdateProductArgs) -> Result<()> {
    let actor = auth::require(ctx, "inventory.edit").await?;
    let p = &args.product;
    let codes = validate_product(ctx, p, Some(args.id)).await?;
    let mut tx = ctx.pool.begin().await?;
    let mut q = sqlx::query(
        "UPDATE products SET name = $1, category_id = $2, barcode = $3,
            cost_price = $4, selling_price = $5, min_stock_level = $6, unit = $7, icon_key = $8,
            is_active = $9, sku = COALESCE($10, sku)
         WHERE id = $11",
    )
    .bind(p.name.trim())
    .bind(p.category_id)
    .bind(codes.first())
    .bind(p.cost_price)
    .bind(p.selling_price)
    .bind(p.min_stock)
    .bind(&p.unit)
    .bind(trimmed(&p.icon_key))
    .bind(p.is_active);
    q = q.bind(trimmed(&p.sku)).bind(args.id);
    let done = q.execute(&mut *tx).await.map_err(map_product_unique)?;
    if done.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    write_children(&mut tx, args.id, &codes, &p.media_ids).await?;
    tx.commit().await?;
    audit::log(ctx, Some(actor.user_id), "product.update", "products", Some(args.id), &serde_json::json!({ "name": p.name })).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::ctx::test_support;

    fn cat(name: &str, parent: Option<i64>) -> CategoryInput {
        CategoryInput {
            name: name.into(),
            description: None,
            parent_id: parent,
            icon_key: "tabler:bottle".into(),
            color: Some("#2d5a8f".into()),
            image_media_id: None,
        }
    }

    fn product(name: &str, barcode: &str) -> ProductInput {
        ProductInput {
            name: name.into(),
            sku: None,
            category_id: None,
            barcode: Some(barcode.into()),
            extra_barcodes: vec![],
            cost_price: 500.0,
            selling_price: 750.0,
            min_stock: 10.0,
            unit: "piece".into(),
            icon_key: None,
            media_ids: vec![],
            is_active: true,
            initial_stock: Some(24.0),
            initial_expiry: None,
        }
    }

    #[test]
    fn categories_are_two_levels_with_icons() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let drinks = create_category(&ctx, cat("Drinks", None)).await.unwrap();
            let soft = create_category(&ctx, cat("Soft", Some(drinks))).await.unwrap();
            assert!(matches!(create_category(&ctx, cat("Cola", Some(soft))).await, Err(Error::Rule("category_depth"))));
            // A parent can't move under another category
            let snacks = create_category(&ctx, cat("Snacks", None)).await.unwrap();
            let r = update_category(&ctx, UpdateCategoryArgs { id: drinks, category: cat("Drinks", Some(snacks)) }).await;
            assert!(matches!(r, Err(Error::Rule("category_depth"))));

            let mut no_icon = cat("X", None);
            no_icon.icon_key = "  ".into();
            assert!(matches!(create_category(&ctx, no_icon).await, Err(Error::Rule("icon_required"))));

            // Same name is fine under different parents, not under the same one
            create_category(&ctx, cat("Other", Some(drinks))).await.unwrap();
            create_category(&ctx, cat("Other", Some(snacks))).await.unwrap();
            assert!(matches!(create_category(&ctx, cat("OTHER", Some(drinks))).await, Err(Error::Rule("category_name_taken"))));

            assert!(matches!(delete_category(&ctx, IdArgs { id: drinks }).await, Err(Error::Rule("category_in_use"))));
        });
    }

    #[test]
    fn move_category_swaps_neighbours() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let a = create_category(&ctx, cat("A", None)).await.unwrap();
            let b = create_category(&ctx, cat("B", None)).await.unwrap();
            move_category(&ctx, MoveArgs { id: b, offset: -1 }).await.unwrap();
            // (setup also created a starter category; look only at these two)
            let order: Vec<(i64,)> = sqlx::query_as("SELECT id FROM categories WHERE id IN ($1, $2) ORDER BY display_order")
                .bind(a)
                .bind(b)
                .fetch_all(&ctx.pool)
                .await
                .unwrap();
            assert_eq!(order, vec![(b,), (a,)]);
            // Moving past the end is a no-op
            move_category(&ctx, MoveArgs { id: b, offset: -1 }).await.unwrap();
        });
    }

    #[test]
    fn product_create_generates_sku_and_records_initial_stock() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let mut p = product("Cola", "111");
            p.extra_barcodes = vec!["222".into()];
            let id = create_product(&ctx, p).await.unwrap();
            let (sku, stock): (String, f64) = sqlx::query_as("SELECT sku, CAST(stock_quantity AS REAL) FROM products WHERE id = $1")
                .bind(id)
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            assert_eq!(sku, format!("P-{id:05}"));
            assert_eq!(stock, 24.0);
            let (adj,): (f64,) = sqlx::query_as("SELECT quantity FROM stock_adjustments WHERE product_id = $1")
                .bind(id)
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            assert_eq!(adj, 24.0);

            // Barcodes are unique across the primary column and the extras table
            assert!(matches!(create_product(&ctx, product("Fake", "222")).await, Err(Error::Rule("barcode_taken"))));
            let mut dup = product("Dup", "333");
            dup.extra_barcodes = vec!["333".into()];
            assert!(matches!(create_product(&ctx, dup).await, Err(Error::Rule("barcode_duplicate"))));
            // Editing a product may keep its own barcodes
            let mut same = product("Cola Zero", "111");
            same.extra_barcodes = vec!["222".into()];
            update_product(&ctx, UpdateProductArgs { id, product: same }).await.unwrap();
        });
    }

    #[test]
    fn cashier_cannot_edit_catalog() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            crate::staff::create_user(&ctx, crate::staff::UserInput {
                full_name: "C".into(),
                username: "cash".into(),
                role_id: 3,
                phone: None,
                photo_media_id: None,
                hire_date: None,
                wage_amount: None,
                wage_period: None,
                is_active: true,
                password: Some("secret1".into()),
                pin: None,
            })
            .await
            .unwrap();
            crate::auth::logout(&ctx).await.unwrap();
            crate::auth::login(&ctx, crate::auth::LoginArgs { username: "cash".into(), password: "secret1".into() })
                .await
                .unwrap();
            assert!(matches!(create_product(&ctx, product("X", "9")).await, Err(Error::Forbidden(_))));
            assert!(matches!(create_category(&ctx, cat("X", None)).await, Err(Error::Forbidden(_))));
        });
    }
}
