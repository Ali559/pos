mod audit;
mod auth;
mod background;
mod backup;
#[cfg(debug_assertions)]
mod bridge;
mod catalog;
mod commands;
mod ctx;
mod customers;
mod db;
mod device;
mod error;
mod expenses;
mod export;
mod import;
mod lookup;
mod loyalty;
mod media;
mod notifications;
mod pricing;
mod printing;
mod query;
mod remote;
mod returns;
mod sales;
mod scale;
mod seeds;
mod server;
mod settings;
mod shifts;
mod staff;
mod stock;
mod stockcount;
mod suppliers;
mod tills;

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
        // Updates: signed releases from GitHub (see TODO.md / RELEASING.md)
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(Arc::new(ctx::SessionState::default()))
        .manage(Arc::new(server::LanServer::default()))
        .manage(backup::RestoreCandidate::default())
        .manage(import::ImportFile::default())
        .setup(|app| {
            app.manage(db::open_read_pool(app.handle())?);
            let link = remote::load(app.handle());
            let till = link.0.clone();
            app.manage(link);
            match till {
                // An extra till: the main computer does the housekeeping.
                // It forgets who was signed in here before this start.
                Some(client) => {
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = client.hello().await {
                            eprintln!("[till] main computer: {e}");
                        }
                    });
                }
                None => {
                    background::start(app.handle().clone());
                    server::start_if_enabled(app.handle());
                }
            }
            #[cfg(debug_assertions)]
            bridge::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            media::upload_media,
            media::get_media_dir,
            media::media_base,
            export::save_csv,
            export::save_xlsx,
            device::get_device_setting,
            device::set_device_setting,
            tills::lan_status,
            tills::set_lan_server,
            tills::new_pairing_code,
            tills::revoke_till,
            tills::pair_with_main,
            tills::leave_main,
            tills::probe_main,
            backup::choose_backup_folder,
            backup::inspect_backup,
            backup::restore_backup,
            import::open_import_file,
            import::import_products,
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
            commands::whoami,
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
            commands::record_export,
            commands::lookup_barcode,
            commands::resolve_scan,
            commands::create_customer,
            commands::update_customer,
            commands::record_customer_payment,
            commands::adjust_customer_balance,
            commands::adjust_loyalty_points,
            commands::create_supplier,
            commands::update_supplier,
            commands::receive_delivery,
            commands::pay_supplier,
            commands::adjust_supplier_balance,
            commands::start_stock_count,
            commands::record_count,
            commands::remove_count_item,
            commands::apply_stock_count,
            commands::cancel_stock_count,
            commands::save_promotion,
            commands::delete_promotion,
            commands::bulk_update_prices,
            commands::move_quick_key,
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
