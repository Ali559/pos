//! Importing a product list from a spreadsheet (CSV, Excel, LibreOffice).
//!
//! The file is picked with a dialog opened here and read into rows kept in
//! memory; the WebView maps columns to fields and asks for a dry run (what
//! would be created, updated or refused) before the real import. Products
//! are matched by barcode, then SKU. Numbers may use Eastern Arabic digits
//! and thousands separators; unit names may be English, Kurdish or Arabic.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};

/// Most rows one import takes
const MAX_ROWS: usize = 20_000;

// ------------------------------------------------------------ reading

fn read_csv(bytes: &[u8]) -> Result<Vec<Vec<String>>> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{FEFF}');
    // Excel in Arabic/Kurdish locales often saves with ; or tabs
    let first = text.lines().next().unwrap_or("");
    let delimiter = [b',', b';', b'\t']
        .into_iter()
        .max_by_key(|d| first.bytes().filter(|b| b == d).count())
        .unwrap_or(b',');
    let mut reader = csv::ReaderBuilder::new().has_headers(false).flexible(true).delimiter(delimiter).from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for rec in reader.records() {
        let rec = rec.map_err(|_| Error::Rule("import_unreadable"))?;
        rows.push(rec.iter().map(|c| c.trim().to_owned()).collect());
    }
    Ok(rows)
}

fn read_sheet(path: &Path) -> Result<Vec<Vec<String>>> {
    use calamine::{open_workbook_auto, Reader};
    let mut book = open_workbook_auto(path).map_err(|_| Error::Rule("import_unreadable"))?;
    let range = book
        .worksheet_range_at(0)
        .ok_or(Error::Rule("import_unreadable"))?
        .map_err(|_| Error::Rule("import_unreadable"))?;
    Ok(range.rows().map(|r| r.iter().map(|c| c.to_string().trim().to_owned()).collect()).collect())
}

pub fn read_file(path: &Path) -> Result<Vec<Vec<String>>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut rows = match ext.as_str() {
        "csv" | "txt" => read_csv(&std::fs::read(path)?)?,
        "xlsx" | "xlsm" | "xls" | "ods" => read_sheet(path)?,
        _ => return Err(Error::Rule("import_unreadable")),
    };
    rows.retain(|r| r.iter().any(|c| !c.is_empty()));
    if rows.len() < 2 {
        return Err(Error::Rule("import_empty"));
    }
    if rows.len() > MAX_ROWS + 1 {
        return Err(Error::Rule("import_too_big"));
    }
    Ok(rows)
}

// ------------------------------------------------------------ values

/// "١٬٢٥٠", "1,250", "1 250.5" → 1250.0 / 1250.5
pub fn parse_number(s: &str) -> Option<f64> {
    let mut out = String::new();
    for c in s.trim().chars() {
        match c {
            '0'..='9' | '-' => out.push(c),
            '٠'..='٩' => out.push(char::from(b'0' + (c as u32 - '٠' as u32) as u8)),
            '۰'..='۹' => out.push(char::from(b'0' + (c as u32 - '۰' as u32) as u8)),
            '.' | '٫' => out.push('.'),
            // thousands separators and currency words are dropped
            _ => {}
        }
    }
    if out.is_empty() {
        return None;
    }
    out.parse().ok()
}

/// The app's unit for a word in any of the three languages.
pub fn parse_unit(s: &str) -> Option<&'static str> {
    let w = s.trim().to_lowercase();
    Some(match w.as_str() {
        "" => "piece",
        "piece" | "pieces" | "pcs" | "pc" | "each" | "دانە" | "دانه" | "قطعة" | "حبة" => "piece",
        "kg" | "kilo" | "kilogram" | "کیلۆ" | "کیلۆگرام" | "كيلو" | "كغم" | "كيلوغرام" => "kg",
        "l" | "liter" | "litre" | "لیتر" | "لتر" => "liter",
        "pack" | "pk" | "پاکەت" | "باكيت" | "علبة" => "pack",
        "box" | "carton" | "کارتۆن" | "سندوق" | "كرتون" => "box",
        _ => return None,
    })
}

/// Which column holds which field, guessed from the header names (the
/// export's own headers in each language, and common supplier words).
pub fn guess_mapping(headers: &[String]) -> HashMap<String, usize> {
    const WORDS: &[(&str, &[&str])] = &[
        ("name", &["name", "product", "item", "description", "ناو", "کاڵا", "الاسم", "المنتج", "اسم"]),
        ("barcode", &["barcode", "ean", "upc", "بارکۆد", "باركود"]),
        ("sku", &["sku", "code", "item code", "کۆد", "الرمز", "رمز"]),
        ("price", &["price", "sell", "selling price", "retail", "نرخ", "نرخی فرۆشتن", "السعر", "سعر البيع"]),
        ("wholesale", &["wholesale", "wholesale price", "جوملە", "نرخی جوملە", "جملة", "سعر الجملة"]),
        ("cost", &["cost", "cost price", "buy price", "تێچوو", "التكلفة", "سعر الشراء"]),
        ("stock", &["stock", "qty", "quantity", "on hand", "کۆگا", "بڕ", "المخزون", "الكمية"]),
        ("minStock", &["reorder", "reorder level", "min", "min stock", "سنووری داواکردنەوە", "حد إعادة الطلب"]),
        ("category", &["category", "group", "department", "پۆل", "الصنف", "الفئة"]),
        ("unit", &["unit", "uom", "sold by", "یەکە", "الوحدة"]),
    ];
    let mut out = HashMap::new();
    for (field, words) in WORDS {
        // Exact header first, then "contains"
        let norm: Vec<String> = headers.iter().map(|h| h.trim().to_lowercase()).collect();
        let hit = norm
            .iter()
            .position(|h| words.contains(&h.as_str()))
            .or_else(|| norm.iter().position(|h| words.iter().any(|w| w.len() > 2 && h.contains(w))));
        if let Some(i) = hit {
            if !out.values().any(|v| *v == i) {
                out.insert((*field).to_owned(), i);
            }
        }
    }
    out
}

// ------------------------------------------------------------ importing

/// A cell's text, trimmed; None when the column isn't mapped or it's blank.
fn pick(r: &[String], i: Option<usize>) -> Option<&str> {
    i.and_then(|i| r.get(i)).map(|s| s.trim()).filter(|s| !s.is_empty())
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOptions {
    /// field → column index
    pub mapping: HashMap<String, usize>,
    /// Change products that already exist (else they're skipped)
    #[serde(default)]
    pub update_existing: bool,
    /// Work it all out, write nothing
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RowResult {
    /// 1-based, counting the header as row 1 (as the spreadsheet shows it)
    pub row: usize,
    /// created | updated | skipped | error
    pub status: String,
    pub name: String,
    /// An error code (errors.*) for refused rows
    pub reason: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: usize,
    pub new_categories: Vec<String>,
    /// Every row with a problem, and the first few hundred others
    pub rows: Vec<RowResult>,
}

/// Runs an import over `rows` (the first row is the header). Everything in
/// one transaction: a refused row doesn't stop the others, but nothing is
/// written on a dry run.
pub async fn run_import(ctx: &Ctx, rows: &[Vec<String>], opts: &ImportOptions) -> Result<ImportReport> {
    let actor = auth::require(ctx, "inventory.edit").await?;
    let col = |field: &str| opts.mapping.get(field).copied();
    if col("name").is_none() && col("barcode").is_none() && col("sku").is_none() {
        return Err(Error::Rule("import_no_key"));
    }
    let may_add_categories = actor.can("catalog.categories");

    let mut tx = ctx.pool.begin().await?;
    let mut report = ImportReport::default();
    let mut categories: HashMap<String, i64> = HashMap::new();
    for (name, id) in sqlx::query_as::<_, (String, i64)>("SELECT name, id FROM categories").fetch_all(&mut *tx).await? {
        categories.insert(name.trim().to_lowercase(), id);
    }
    let at = now();

    for (i, r) in rows.iter().enumerate().skip(1) {
        let row = i + 1;
        let name = pick(r, col("name")).unwrap_or("").to_owned();
        let push = |report: &mut ImportReport, status: &str, reason: Option<&str>| {
            match status {
                "created" => report.created += 1,
                "updated" => report.updated += 1,
                "skipped" => report.skipped += 1,
                _ => report.errors += 1,
            }
            if reason.is_some() || report.rows.len() < 300 {
                report.rows.push(RowResult { row, status: status.into(), name: name.clone(), reason: reason.map(Into::into) });
            }
        };

        // Numbers: present but unreadable is an error, absent is fine
        let num = |field: &str| -> std::result::Result<Option<f64>, ()> {
            match pick(r, col(field)) {
                None => Ok(None),
                Some(s) => parse_number(s).filter(|n| *n >= 0.0).map(Some).ok_or(()),
            }
        };
        let (Ok(price), Ok(cost), Ok(wholesale), Ok(stock), Ok(min)) = (num("price"), num("cost"), num("wholesale"), num("stock"), num("minStock")) else {
            push(&mut report, "error", Some("import_bad_number"));
            continue;
        };
        let unit = match pick(r, col("unit")) {
            None => None,
            Some(u) => match parse_unit(u) {
                Some(u) => Some(u),
                None => {
                    push(&mut report, "error", Some("import_bad_unit"));
                    continue;
                }
            },
        };
        let barcode = pick(r, col("barcode"));
        let sku = pick(r, col("sku"));

        // Category by name, made if missing (and allowed)
        let category = match pick(r, col("category")) {
            None => None,
            Some(c) => match categories.get(&c.to_lowercase()) {
                Some(id) => Some(*id),
                None if !may_add_categories => {
                    push(&mut report, "error", Some("import_unknown_category"));
                    continue;
                }
                None => {
                    report.new_categories.push(c.to_owned());
                    let id = if opts.dry_run {
                        -(report.new_categories.len() as i64)
                    } else {
                        sqlx::query("INSERT INTO categories (name, icon_key, display_order) VALUES ($1, 'tabler:tag', (SELECT COALESCE(MAX(display_order), 0) + 1 FROM categories))")
                            .bind(c)
                            .execute(&mut *tx)
                            .await?
                            .last_insert_rowid()
                    };
                    categories.insert(c.to_lowercase(), id);
                    Some(id)
                }
            },
        };

        // Match: barcode (any of the product's), then SKU
        let existing: Option<(i64,)> = match (barcode, sku) {
            (Some(b), _) => {
                sqlx::query_as("SELECT id FROM products WHERE barcode = $1 OR id IN (SELECT product_id FROM product_barcodes WHERE barcode = $1)")
                    .bind(b)
                    .fetch_optional(&mut *tx)
                    .await?
            }
            (None, Some(s)) => sqlx::query_as("SELECT id FROM products WHERE sku = $1").bind(s).fetch_optional(&mut *tx).await?,
            _ => None,
        };
        let existing = match (existing, sku) {
            (None, Some(s)) if barcode.is_some() => sqlx::query_as("SELECT id FROM products WHERE sku = $1").bind(s).fetch_optional(&mut *tx).await?,
            (e, _) => e,
        };

        match existing {
            Some((id,)) => {
                if !opts.update_existing {
                    push(&mut report, "skipped", None);
                    continue;
                }
                if !opts.dry_run {
                    crate::pricing::record_changes(&mut tx, id, price, wholesale.map(Some), cost, "import", actor.user_id).await?;
                    sqlx::query(
                        "UPDATE products SET
                            name = COALESCE($1, name), selling_price = COALESCE($2, selling_price), cost_price = COALESCE($3, cost_price),
                            wholesale_price = COALESCE($4, wholesale_price), min_stock_level = COALESCE($5, min_stock_level),
                            category_id = COALESCE($6, category_id), unit = COALESCE($7, unit)
                         WHERE id = $8",
                    )
                    .bind(pick(r, col("name")))
                    .bind(price)
                    .bind(cost)
                    .bind(wholesale)
                    .bind(min)
                    .bind(category)
                    .bind(unit)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                }
                push(&mut report, "updated", None);
            }
            None => {
                if name.is_empty() {
                    push(&mut report, "error", Some("product_name_required"));
                    continue;
                }
                let Some(price) = price else {
                    push(&mut report, "error", Some("import_no_price"));
                    continue;
                };
                let unit = unit.unwrap_or("piece");
                if crate::catalog::is_whole_unit(unit) && stock.is_some_and(|s| s.fract() != 0.0) {
                    push(&mut report, "error", Some("import_bad_number"));
                    continue;
                }
                if !opts.dry_run {
                    let sku_value = sku.map(str::to_owned).unwrap_or_else(|| format!("tmp-{}", uuid::Uuid::new_v4()));
                    let done = sqlx::query(
                        "INSERT INTO products (sku, name, category_id, barcode, cost_price, selling_price, wholesale_price, min_stock_level, unit, stock_quantity)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                    )
                    .bind(&sku_value)
                    .bind(&name)
                    .bind(category)
                    .bind(barcode)
                    .bind(cost.unwrap_or(0.0))
                    .bind(price)
                    .bind(wholesale)
                    .bind(min.unwrap_or(0.0))
                    .bind(unit)
                    .bind(stock.unwrap_or(0.0))
                    .execute(&mut *tx)
                    .await;
                    let id = match done {
                        Ok(d) => d.last_insert_rowid(),
                        Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
                            push(&mut report, "error", Some(if db.message().contains("sku") { "sku_taken" } else { "barcode_taken" }));
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    };
                    if sku.is_none() {
                        sqlx::query("UPDATE products SET sku = $1 WHERE id = $2").bind(format!("P-{id:05}")).bind(id).execute(&mut *tx).await?;
                    }
                    if let Some(s) = stock.filter(|s| *s > 0.0) {
                        sqlx::query(
                            "INSERT INTO stock_adjustments (product_id, adjustment_type, quantity, reason, user_id, created_at) VALUES ($1, 'addition', $2, 'import', $3, $4)",
                        )
                        .bind(id)
                        .bind(s)
                        .bind(actor.user_id)
                        .bind(&at)
                        .execute(&mut *tx)
                        .await?;
                        crate::stock::insert_batch(&mut tx, id, s, None, cost, Some("import"), actor.user_id).await?;
                    }
                }
                push(&mut report, "created", None);
            }
        }
    }

    if opts.dry_run {
        tx.rollback().await?;
    } else {
        tx.commit().await?;
        audit::log(
            ctx,
            Some(actor.user_id),
            "import",
            "products",
            None,
            &serde_json::json!({ "created": report.created, "updated": report.updated, "errors": report.errors }),
        )
        .await?;
        crate::notifications::refresh_quietly(ctx).await;
    }
    Ok(report)
}

// ------------------------------------------------------------ commands

/// The file picked for import, until it's imported or replaced.
#[derive(Default)]
pub struct ImportFile(Mutex<Option<(String, Vec<Vec<String>>)>>);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedFile {
    pub token: String,
    pub file_name: String,
    pub headers: Vec<String>,
    /// The first few rows under the header
    pub sample: Vec<Vec<String>>,
    pub row_count: usize,
    pub guess: HashMap<String, usize>,
}

/// Opens a file dialog and reads the spreadsheet. None if cancelled.
#[tauri::command]
pub async fn open_import_file(app: AppHandle) -> Result<Option<OpenedFile>> {
    // Files on this computer feed the main computer's database: do it there
    if crate::remote::client(&app).is_some() {
        return Err(crate::error::Error::Rule("main_only"));
    }
    let ctx = Ctx::from_app(&app).await?;
    auth::require(&ctx, "inventory.edit").await?;
    let picked = app.dialog().file().add_filter("Spreadsheet", &["csv", "xlsx", "xls", "ods", "txt"]).blocking_pick_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let rows = {
        let path = path.clone();
        tauri::async_runtime::spawn_blocking(move || read_file(&path)).await.map_err(|e| Error::Invalid(e.to_string()))??
    };
    let token = uuid::Uuid::new_v4().to_string();
    let opened = OpenedFile {
        token: token.clone(),
        file_name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        headers: rows[0].clone(),
        sample: rows.iter().skip(1).take(5).cloned().collect(),
        row_count: rows.len() - 1,
        guess: guess_mapping(&rows[0]),
    };
    *app.state::<ImportFile>().0.lock().unwrap() = Some((token, rows));
    Ok(Some(opened))
}

#[derive(Debug, Deserialize)]
pub struct ImportArgs {
    pub token: String,
    #[serde(flatten)]
    pub options: ImportOptions,
}

#[tauri::command]
pub async fn import_products(app: AppHandle, args: ImportArgs) -> Result<ImportReport> {
    // Files on this computer feed the main computer's database: do it there
    if crate::remote::client(&app).is_some() {
        return Err(crate::error::Error::Rule("main_only"));
    }
    let ctx = Ctx::from_app(&app).await?;
    let rows = {
        let state = app.state::<ImportFile>();
        let slot = state.0.lock().unwrap();
        match slot.as_ref() {
            Some((token, rows)) if *token == args.token => rows.clone(),
            _ => return Err(Error::Rule("import_reopen")),
        }
    };
    let report = run_import(&ctx, &rows, &args.options).await?;
    if !args.options.dry_run {
        *app.state::<ImportFile>().0.lock().unwrap() = None;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::ctx::test_support;

    #[test]
    fn numbers_units_and_headers() {
        assert_eq!(parse_number("1,250"), Some(1250.0));
        assert_eq!(parse_number("١٬٢٥٠"), Some(1250.0));
        assert_eq!(parse_number("۱۲۵۰٫۵"), Some(1250.5));
        assert_eq!(parse_number("2 500 IQD"), Some(2500.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_unit("کیلۆ"), Some("kg"));
        assert_eq!(parse_unit("PCS"), Some("piece"));
        assert_eq!(parse_unit("dozen"), None);
        let headers: Vec<String> = ["ناو", "Barcode", "نرخ", "Cost price", "Qty", "پۆل"].map(String::from).to_vec();
        let g = guess_mapping(&headers);
        assert_eq!((g["name"], g["barcode"], g["price"], g["cost"], g["stock"], g["category"]), (0, 1, 2, 3, 4, 5));
        // ; separated, with a BOM
        let rows = read_csv("\u{FEFF}name;price\nشیر;1.750\n".as_bytes()).unwrap();
        assert_eq!(rows, vec![vec!["name".to_owned(), "price".into()], vec!["شیر".into(), "1.750".into()]]);
    }

    #[test]
    fn dry_run_then_import_creates_and_updates() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let rows: Vec<Vec<String>> = [
                ["name", "barcode", "price", "cost", "stock", "category", "unit"],
                ["Milk", "111", "1,750", "1500", "24", "Dairy", "piece"],
                ["Cheese", "222", "۱۲۰۰۰", "", "3.5", "Dairy", "کیلۆ"],
                ["Bad", "333", "free", "", "", "", ""],
                ["", "444", "100", "", "", "", ""],
            ]
            .iter()
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .collect();
            let mapping: HashMap<String, usize> = guess_mapping(&rows[0]);
            let dry = run_import(&ctx, &rows, &ImportOptions { mapping: mapping.clone(), update_existing: true, dry_run: true }).await.unwrap();
            assert_eq!((dry.created, dry.updated, dry.errors), (2, 0, 2));
            assert_eq!(dry.new_categories, vec!["Dairy"], "made once, used by both rows");
            let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM products").fetch_one(&ctx.pool).await.unwrap();
            assert_eq!(n, 0, "a dry run writes nothing");

            let real = run_import(&ctx, &rows, &ImportOptions { mapping: mapping.clone(), update_existing: true, dry_run: false }).await.unwrap();
            assert_eq!((real.created, real.errors), (2, 2));
            let (price, stock, unit, cat): (f64, f64, String, String) = sqlx::query_as(
                "SELECT CAST(p.selling_price AS REAL), CAST(p.stock_quantity AS REAL), p.unit, c.name FROM products p JOIN categories c ON c.id = p.category_id WHERE p.barcode = '222'",
            )
            .fetch_one(&ctx.pool)
            .await
            .unwrap();
            assert_eq!((price, stock, unit.as_str(), cat.as_str()), (12000.0, 3.5, "kg", "Dairy"));

            // Again with a new price: an update, remembered in the history
            let mut again = rows[..2].to_vec();
            again[1][2] = "2000".into();
            let r = run_import(&ctx, &again, &ImportOptions { mapping, update_existing: true, dry_run: false }).await.unwrap();
            assert_eq!(r.updated, 1);
            let (h,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM price_history WHERE source = 'import'").fetch_one(&ctx.pool).await.unwrap();
            assert_eq!(h, 1);
        });
    }
}
