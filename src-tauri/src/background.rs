//! Housekeeping that runs without anyone clicking: at startup and hourly,
//! create due recurring-expense drafts and recompute alerts (expiry dates
//! roll over at midnight with no write to trigger them), and sweep media
//! nothing uses any more.

use std::time::Duration;

use tauri::AppHandle;

use crate::ctx::Ctx;
use crate::{expenses, media, notifications};

const EVERY: Duration = Duration::from_secs(60 * 60);

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            match Ctx::from_app(&app).await {
                Ok(ctx) => {
                    if let Err(e) = expenses::generate_recurring(&ctx).await {
                        eprintln!("[background] recurring expenses: {e}");
                    }
                    notifications::refresh_quietly(&ctx).await;
                    if let Err(e) = media::clean_unused(&ctx).await {
                        eprintln!("[background] media cleanup: {e}");
                    }
                }
                Err(e) => eprintln!("[background] {e}"),
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}
