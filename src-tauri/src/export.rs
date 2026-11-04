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

/// Returns false if the user cancelled the dialog.
#[tauri::command]
pub async fn save_csv(app: AppHandle, args: SaveCsvArgs) -> Result<bool> {
    let ctx = Ctx::from_app(&app).await?;
    let actor = auth::require(&ctx, "analytics.export").await?;
    let bytes = to_csv(&args.rows)?;
    let name: String = args
        .file_name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '-' })
        .collect();
    let picked = app
        .dialog()
        .file()
        .set_file_name(format!("{}.csv", name.trim_end_matches(".csv")))
        .add_filter("CSV", &["csv"])
        .blocking_save_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(false);
    };
    std::fs::write(&path, bytes)?;
    crate::audit::log(&ctx, Some(actor.user_id), "export", "", None, &serde_json::json!({ "file": path.file_name().map(|f| f.to_string_lossy().into_owned()), "rows": args.rows.len() })).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_has_bom_and_quotes_commas() {
        let bytes = to_csv(&[vec!["name".into(), "total".into()], vec!["شیر, تازە".into(), "1,750".into()]]).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with('\u{FEFF}'));
        assert!(text.contains("\"شیر, تازە\",\"1,750\""));
    }
}
