//! The command surface. Each entry becomes a `#[tauri::command]` taking one
//! `args` object (invoke("login", { args: {...} })) and is also reachable by
//! name through `dispatch`, which the dev bridge uses. One list, so the two
//! can't drift apart.

use tauri::AppHandle;

use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::{auth, catalog, query, settings, shifts, staff};

macro_rules! commands {
    ($( $name:ident $( ($args:ty) )? -> $out:ty => $f:path; )*) => {
        $(
            #[tauri::command]
            pub async fn $name(app: AppHandle $(, args: $args)?) -> Result<$out> {
                let ctx = Ctx::from_app(&app).await?;
                $f(&ctx $(, { let a: $args = args; a })?).await
            }
        )*

        #[cfg(test)]
        pub const NAMES: &[&str] = &[$(stringify!($name)),*];

        // Only the dev bridge calls this, and it's compiled out of release builds
        #[allow(unused_variables)]
        #[cfg_attr(not(debug_assertions), allow(dead_code))]
        pub async fn dispatch(ctx: &Ctx, name: &str, args: serde_json::Value) -> Result<serde_json::Value> {
            match name {
                $(
                    stringify!($name) => {
                        $( let a: $args = serde_json::from_value(args)?; )?
                        Ok(serde_json::to_value($f(ctx $(, { let _: Option<$args> = None; a })?).await?)?)
                    }
                )*
                _ => Err(Error::Invalid(format!("unknown command {name}"))),
            }
        }
    };
}

commands! {
    select (query::SelectArgs) -> Vec<serde_json::Map<String, serde_json::Value>> => query::select;

    auth_status -> auth::AuthStatus => auth::status;
    complete_setup (auth::SetupArgs) -> auth::AuthStatus => auth::complete_setup;
    login (auth::LoginArgs) -> auth::AuthStatus => auth::login;
    login_pin (auth::PinArgs) -> auth::AuthStatus => auth::login_pin;
    logout -> () => auth::logout;
    end_shift -> () => auth::end_shift;

    set_setting (settings::SetSettingArgs) -> () => settings::set_setting;
    save_shop_profile (settings::SaveShopArgs) -> () => settings::save_shop_profile;

    create_role (staff::RoleInput) -> i64 => staff::create_role;
    update_role (staff::UpdateRoleArgs) -> () => staff::update_role;
    delete_role (staff::DeleteRoleArgs) -> () => staff::delete_role;
    create_user (staff::UserInput) -> i64 => staff::create_user;
    update_user (staff::UpdateUserArgs) -> () => staff::update_user;

    save_schedule (shifts::ScheduleInput) -> i64 => shifts::save_schedule;
    delete_schedule (shifts::IdArgs) -> () => shifts::delete_schedule;
    copy_schedule_week (shifts::CopyWeekArgs) -> shifts::CopyWeekResult => shifts::copy_schedule_week;

    create_category (catalog::CategoryInput) -> i64 => catalog::create_category;
    update_category (catalog::UpdateCategoryArgs) -> () => catalog::update_category;
    move_category (catalog::MoveArgs) -> () => catalog::move_category;
    set_category_active (catalog::ActiveArgs) -> () => catalog::set_category_active;
    delete_category (catalog::IdArgs) -> () => catalog::delete_category;
    create_product (catalog::ProductInput) -> i64 => catalog::create_product;
    update_product (catalog::UpdateProductArgs) -> () => catalog::update_product;

    seed_demo_data -> () => crate::db::seed_demo_data;
}
