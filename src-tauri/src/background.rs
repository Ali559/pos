//! Housekeeping that runs without anyone clicking: at startup and hourly,
//! create due recurring-expense drafts and recompute alerts (expiry dates
//! roll over at midnight with no write to trigger them), take the daily
//! backup when one is due, and sweep media nothing uses any more.

use std::time::Duration;

use tauri::AppHandle;

use crate::ctx::Ctx;
use crate::{backup, catalog, expenses, media, notifications};

const EVERY: Duration = Duration::from_secs(60 * 60);
/// The sql plugin opens the database in the background at startup; until
/// it has, try again soon rather than in an hour
const NOT_READY_RETRY: Duration = Duration::from_secs(3);

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut waited = false;
        loop {
            match Ctx::from_app(&app).await {
                Ok(ctx) => {
                    // Shops set up before the misc item existed get it here
                    if let Err(e) = catalog::ensure_misc_product(&ctx).await {
                        eprintln!("[background] misc item: {e}");
                    }
                    if let Err(e) = expenses::generate_recurring(&ctx).await {
                        eprintln!("[background] recurring expenses: {e}");
                    }
                    // Before the alerts, so a failed backup shows up at once
                    if let Err(e) = backup::run_if_due(&ctx).await {
                        eprintln!("[background] backup: {e}");
                    }
                    notifications::refresh_quietly(&ctx).await;
                    if let Err(e) = media::clean_unused(&ctx).await {
                        eprintln!("[background] media cleanup: {e}");
                    }
                }
                Err(crate::error::Error::DbNotReady) => {
                    if !waited {
                        eprintln!("[background] waiting for the database");
                        waited = true;
                    }
                    tokio::time::sleep(NOT_READY_RETRY).await;
                    continue;
                }
                Err(e) => eprintln!("[background] {e}"),
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}
