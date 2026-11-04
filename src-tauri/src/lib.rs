mod audit;
mod auth;
#[cfg(debug_assertions)]
mod bridge;
mod commands;
mod ctx;
mod db;
mod error;
mod media;
mod query;
mod settings;
mod shifts;
mod staff;

use std::sync::Arc;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations(db::DB_URL, db::migrations())
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(ctx::SessionState::default()))
        .setup(|app| {
            app.manage(db::open_read_pool(app.handle())?);
            #[cfg(debug_assertions)]
            bridge::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            media::upload_media,
            media::get_media_dir,
            commands::select,
            commands::auth_status,
            commands::complete_setup,
            commands::login,
            commands::login_pin,
            commands::logout,
            commands::end_shift,
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
