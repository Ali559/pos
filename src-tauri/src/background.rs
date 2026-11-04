//! Housekeeping that runs without anyone clicking: at startup and hourly,
//! create due recurring-expense drafts and recompute alerts (expiry dates
//! roll over at midnight with no write to trigger them).

use std::time::Duration;

use tauri::AppHandle;

use crate::ctx::Ctx;
use crate::{expenses, notifications};

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
                }
                Err(e) => eprintln!("[background] {e}"),
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}
