mod audit;
mod auth;
mod background;
mod backup;
#[cfg(debug_assertions)]
mod bridge;
mod catalog;
mod commands;
mod ctx;
mod db;
mod error;
mod expenses;
mod export;
mod lookup;
mod media;
mod notifications;
mod printing;
mod query;
mod returns;
mod sales;
mod scale;
mod seeds;
mod settings;
mod shifts;
mod staff;
mod stock;

use std::sync::Arc;

use tauri::Manager;

/// Opens the system print dialog for the page; the receipt is laid out by
/// print CSS. (`window.print()` does nothing in some webviews.)
#[tauri::command]
fn print_receipt(webview: tauri::Webview) -> error::Result<()> {
    Ok(webview.print()?)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Before the sql plugin: a staged restore must be in place before
        // the database is opened
        .plugin(backup::restore_plugin())
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations(db::DB_URL, db::migrations())
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(Arc::new(ctx::SessionState::default()))
        .manage(backup::RestoreCandidate::default())
        .setup(|app| {
            app.manage(db::open_read_pool(app.handle())?);
            background::start(app.handle().clone());
            #[cfg(debug_assertions)]
            bridge::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            media::upload_media,
            media::get_media_dir,
            export::save_csv,
            backup::choose_backup_folder,
            backup::inspect_backup,
            backup::restore_backup,
            commands::select,
            commands::auth_status,
            commands::complete_setup,
            commands::login,
            commands::login_pin,
            commands::logout,
            commands::end_shift,
            commands::shift_summary,
            commands::count_opening_cash,
            commands::list_printers,
            commands::print_raster,
            commands::authorize_override,
            commands::set_setting,
            commands::save_shop_profile,
            commands::create_role,
            commands::update_role,
            commands::delete_role,
            commands::create_user,
            commands::update_user,
            commands::save_schedule,
            commands::delete_schedule,
            commands::copy_schedule_week,
            commands::create_category,
            commands::update_category,
            commands::move_category,
            commands::set_category_active,
            commands::delete_category,
            commands::create_product,
            commands::update_product,
            commands::checkout,
            commands::void_line,
            commands::hold_sale,
            commands::take_held_sale,
            commands::create_return,
            print_receipt,
            commands::receive_stock,
            commands::remove_stock,
            commands::refresh_notifications,
            commands::mark_notifications_read,
            commands::dismiss_notification,
            commands::create_expense_type,
            commands::update_expense_type,
            commands::set_expense_type_active,
            commands::create_expense,
            commands::update_expense,
            commands::confirm_expense,
            commands::delete_expense,
            commands::authorize_export,
            commands::lookup_barcode,
            commands::resolve_scan,
            commands::backup_status,
            commands::backup_now,
            commands::seed_demo_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    /// Every command in commands.rs must also be registered above.
    #[test]
    fn all_commands_registered() {
        let src = include_str!("lib.rs");
        for name in crate::commands::NAMES {
            assert!(src.contains(&format!("commands::{name},")), "{name} is not in generate_handler!");
        }
    }
}
