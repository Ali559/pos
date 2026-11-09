//! CSV export. The save dialog is opened here, in Rust, so the WebView can
//! never choose where a file is written.

use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::auth;
use crate::ctx::Ctx;
use crate::error::{Error, Result};

/// Checks the permission without writing anything; the browser dev build
/// uses this, then downloads the file itself.
pub async fn authorize_export(ctx: &Ctx) -> Result<()> {
    auth::require(ctx, "analytics.export").await.map(|_| ())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCsvArgs {
    pub file_name: String,
    /// Rows of cells; the first row is the header
    pub rows: Vec<Vec<String>>,
}

/// UTF-8 with a BOM, so Excel shows Kurdish and Arabic correctly.
pub fn to_csv(rows: &[Vec<String>]) -> Result<Vec<u8>> {
    let mut out = Vec::from("\u{FEFF}".as_bytes());
    {
        let mut w = csv::Writer::from_writer(&mut out);
        for row in rows {
            w.write_record(row).map_err(|e| Error::Invalid(e.to_string()))?;
        }
        w.flush()?;
    }
    Ok(out)
}

fn safe_name(name: &str, ext: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '-' })
        .collect();
    format!("{}.{ext}", name.trim_end_matches(&format!(".{ext}")))
}

/// Asks where to save (in Rust, so the WebView never picks a path). None
/// if the user cancelled.
fn ask_path(app: &AppHandle, file_name: &str, ext: &str, label: &str) -> Option<std::path::PathBuf> {
    app.dialog()
        .file()
        .set_file_name(safe_name(file_name, ext))
        .add_filter(label, &[ext])
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
}

async fn saved(ctx: &Ctx, actor: &auth::Actor, path: &std::path::Path, rows: usize) -> Result<()> {
    crate::audit::log(ctx, Some(actor.user_id), "export", "", None, &serde_json::json!({ "file": path.file_name().map(|f| f.to_string_lossy().into_owned()), "rows": rows })).await
}

/// Returns false if the user cancelled the dialog.
#[tauri::command]
pub async fn save_csv(app: AppHandle, args: SaveCsvArgs) -> Result<bool> {
    let ctx = Ctx::from_app(&app).await?;
    let actor = auth::require(&ctx, "analytics.export").await?;
    let bytes = to_csv(&args.rows)?;
    let Some(path) = ask_path(&app, &args.file_name, "csv", "CSV") else {
        return Ok(false);
    };
    std::fs::write(&path, bytes)?;
    saved(&ctx, &actor, &path, args.rows.len()).await?;
    Ok(true)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveXlsxArgs {
    pub file_name: String,
    /// The sheet's name (Excel allows 31 characters)
    pub sheet: String,
    /// The first row is the header; numbers stay numbers so Excel can add them up
    pub rows: Vec<Vec<serde_json::Value>>,
    /// Kurdish and Arabic: the sheet reads right to left
    #[serde(default)]
    pub rtl: bool,
}

pub fn to_xlsx(args: &SaveXlsxArgs) -> Result<Vec<u8>> {
    use rust_xlsxwriter::{Format, Workbook};
    let bad = |e: rust_xlsxwriter::XlsxError| Error::Invalid(e.to_string());
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    let name: String = args.sheet.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
    if !name.trim().is_empty() {
        sheet.set_name(name.trim()).map_err(bad)?;
    }
    sheet.set_right_to_left(args.rtl);
    let header = Format::new().set_bold();
    let number = Format::new().set_num_format("#,##0.##");
    for (r, row) in args.rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            let (r, c) = (r as u32, c as u16);
            match cell {
                serde_json::Value::Number(n) if r > 0 => {
                    sheet.write_number_with_format(r, c, n.as_f64().unwrap_or(0.0), &number).map_err(bad)?;
                }
                serde_json::Value::Null => {}
                v => {
                    let text = v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string());
                    if r == 0 {
                        sheet.write_string_with_format(r, c, text, &header).map_err(bad)?;
                    } else {
                        sheet.write_string(r, c, text).map_err(bad)?;
                    }
                }
            }
        }
    }
    sheet.set_freeze_panes(1, 0).map_err(bad)?;
    sheet.autofit();
    book.save_to_buffer().map_err(bad)
}

/// An Excel workbook with one sheet. Returns false if cancelled.
#[tauri::command]
pub async fn save_xlsx(app: AppHandle, args: SaveXlsxArgs) -> Result<bool> {
    let ctx = Ctx::from_app(&app).await?;
    let actor = auth::require(&ctx, "analytics.export").await?;
    let bytes = to_xlsx(&args)?;
    let Some(path) = ask_path(&app, &args.file_name, "xlsx", "Excel") else {
        return Ok(false);
    };
    std::fs::write(&path, bytes)?;
    saved(&ctx, &actor, &path, args.rows.len()).await?;
    Ok(true)
}

#[derive(Debug, Deserialize)]
pub struct RecordExportArgs {
    pub details: serde_json::Value,
}

/// An extra till saved a report on its own computer: logged here.
pub async fn record_export(ctx: &Ctx, args: RecordExportArgs) -> Result<()> {
    let actor = auth::require(ctx, "analytics.export").await?;
    crate::audit::log(ctx, Some(actor.user_id), "export", "", None, &args.details).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xlsx_is_a_zip_with_numbers_kept() {
        let bytes = to_xlsx(&SaveXlsxArgs {
            file_name: "x".into(),
            sheet: "فرۆشتن/2026".into(),
            rows: vec![
                vec!["name".into(), "total".into()],
                vec!["شیر".into(), serde_json::json!(1750.5)],
            ],
            rtl: true,
        })
        .unwrap();
        assert_eq!(&bytes[..2], b"PK");
    }

    #[test]
    fn csv_has_bom_and_quotes_commas() {
        let bytes = to_csv(&[vec!["name".into(), "total".into()], vec!["شیر, تازە".into(), "1,750".into()]]).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with('\u{FEFF}'));
        assert!(text.contains("\"شیر, تازە\",\"1,750\""));
    }
}
