//! The command surface. Each entry becomes a `#[tauri::command]` taking one
//! `args` object (invoke("login", { args: {...} })) and is also reachable by
//! name through `dispatch`, which the dev bridge uses. One list, so the two
//! can't drift apart.

use tauri::AppHandle;

use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::{auth, backup, catalog, customers, expenses, export, lookup, loyalty, notifications, pricing, printing, query, rates, returns, sales, scale, settings, shifts, staff, stock, stockcount, suppliers};

macro_rules! commands {
    ($( $name:ident $( ($args:ty) )? -> $out:ty => $f:path; )*) => {
        $(
            #[tauri::command]
            pub async fn $name(app: AppHandle, args: Option<serde_json::Value>) -> Result<serde_json::Value> {
                run(&app, stringify!($name), args.unwrap_or(serde_json::Value::Null)).await
            }
        )*

        #[cfg(test)]
        pub const NAMES: &[&str] = &[$(stringify!($name)),*];

        /// Runs a command by name on this computer's database. The app
        /// window (through `run`), the dev bridge and the main computer's
        /// till server all come through here.
        #[allow(unused_variables)]
        pub async fn dispatch(ctx: &Ctx, name: &str, args: serde_json::Value) -> Result<serde_json::Value> {
            match name {
                $(
                    stringify!($name) => {
                        $( let a: $args = serde_json::from_value(args)?; )?
                        let out: $out = $f(ctx $(, { let _: Option<$args> = None; a })?).await?;
                        Ok(serde_json::to_value(out)?)
                    }
                )*
                _ => Err(Error::Invalid(format!("unknown command {name}"))),
            }
        }
    };
}

/// On an extra till everything but printing goes to the main computer;
/// otherwise it runs here.
pub async fn run(app: &AppHandle, name: &str, args: serde_json::Value) -> Result<serde_json::Value> {
    if let Some(remote) = crate::remote::client(app) {
        if !crate::remote::LOCAL.contains(&name) {
            return remote.invoke(name, args).await;
        }
    }
    let ctx = Ctx::from_app(app).await?;
    dispatch(&ctx, name, args).await
}

commands! {
    select (query::SelectArgs) -> Vec<serde_json::Map<String, serde_json::Value>> => query::select;

    auth_status -> auth::AuthStatus => auth::status;
    complete_setup (auth::SetupArgs) -> auth::AuthStatus => auth::complete_setup;
    login (auth::LoginArgs) -> auth::AuthStatus => auth::login;
    login_pin (auth::PinArgs) -> auth::AuthStatus => auth::login_pin;
    logout -> () => auth::logout;
    end_shift (auth::EndShiftArgs) -> () => auth::end_shift;
    shift_summary -> Option<shifts::ShiftSummary> => shifts::shift_summary;
    count_opening_cash (shifts::OpeningCountArgs) -> () => shifts::count_opening_cash;
    list_printers -> Vec<printing::PrinterInfo> => printing::list_printers;
    print_raster (printing::RasterArgs) -> () => printing::print_raster;
    authorize_override (auth::OverrideArgs) -> auth::OverrideGrant => auth::authorize_override;
    whoami -> auth::Actor => auth::whoami;

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

    checkout (sales::CheckoutArgs) -> sales::SaleResult => sales::checkout;
    void_line (sales::VoidLineArgs) -> () => sales::void_line;
    hold_sale (sales::HoldArgs) -> i64 => sales::hold_sale;
    take_held_sale (sales::HeldIdArgs) -> serde_json::Value => sales::take_held_sale;
    create_return (returns::ReturnArgs) -> returns::ReturnResult => returns::create_return;
    receive_stock (stock::ReceiveArgs) -> () => stock::receive_stock;
    remove_stock (stock::RemoveArgs) -> () => stock::remove_stock;

    refresh_notifications -> () => notifications::refresh_notifications;
    mark_notifications_read (notifications::MarkReadArgs) -> () => notifications::mark_notifications_read;
    dismiss_notification (notifications::IdArgs) -> () => notifications::dismiss_notification;

    create_expense_type (expenses::ExpenseTypeInput) -> i64 => expenses::create_expense_type;
    update_expense_type (expenses::UpdateTypeArgs) -> () => expenses::update_expense_type;
    set_expense_type_active (expenses::ActiveArgs) -> () => expenses::set_expense_type_active;
    create_expense (expenses::ExpenseInput) -> i64 => expenses::create_expense;
    update_expense (expenses::UpdateExpenseArgs) -> () => expenses::update_expense;
    confirm_expense (expenses::IdArgs) -> () => expenses::confirm_expense;
    delete_expense (expenses::IdArgs) -> () => expenses::delete_expense;

    authorize_export -> () => export::authorize_export;
    record_export (export::RecordExportArgs) -> () => export::record_export;
    lookup_barcode (lookup::LookupArgs) -> lookup::Lookup => lookup::lookup_barcode;
    resolve_scan (scale::ScanArgs) -> Option<scale::ScanResult> => scale::resolve_scan;

    create_customer (customers::CustomerInput) -> i64 => customers::create_customer;
    update_customer (customers::UpdateCustomerArgs) -> () => customers::update_customer;
    record_customer_payment (customers::PaymentArgs) -> customers::PaymentResult => customers::record_customer_payment;
    adjust_customer_balance (customers::AdjustArgs) -> f64 => customers::adjust_customer_balance;
    adjust_loyalty_points (loyalty::AdjustPointsArgs) -> i64 => loyalty::adjust_loyalty_points;

    create_supplier (suppliers::SupplierInput) -> i64 => suppliers::create_supplier;
    update_supplier (suppliers::UpdateSupplierArgs) -> () => suppliers::update_supplier;
    receive_delivery (suppliers::DeliveryArgs) -> suppliers::DeliveryResult => suppliers::receive_delivery;
    pay_supplier (suppliers::SupplierPaymentArgs) -> f64 => suppliers::pay_supplier;

    exchange_rate -> rates::RateStatus => rates::exchange_rate;
    set_rate (rates::SetRateArgs) -> rates::RateStatus => rates::set_rate;
    accept_rate_offer (rates::SetRateArgs) -> rates::RateStatus => rates::accept_rate_offer;
    dismiss_rate_offer -> rates::RateStatus => rates::dismiss_rate_offer;
    fetch_rate_now -> rates::RateStatus => rates::fetch_rate_now;
    adjust_supplier_balance (suppliers::SupplierAdjustArgs) -> f64 => suppliers::adjust_supplier_balance;

    start_stock_count (stockcount::StartCountArgs) -> i64 => stockcount::start_stock_count;
    record_count (stockcount::RecordCountArgs) -> stockcount::Counted => stockcount::record_count;
    remove_count_item (stockcount::CountItemArgs) -> () => stockcount::remove_count_item;
    apply_stock_count (stockcount::CountIdArgs) -> stockcount::Applied => stockcount::apply_stock_count;
    cancel_stock_count (stockcount::CountIdArgs) -> () => stockcount::cancel_stock_count;

    save_promotion (pricing::SavePromotionArgs) -> i64 => pricing::save_promotion;
    delete_promotion (pricing::IdArgs) -> () => pricing::delete_promotion;
    bulk_update_prices (pricing::BulkPriceArgs) -> i64 => pricing::bulk_update_prices;
    move_quick_key (catalog::QuickKeyMoveArgs) -> () => catalog::move_quick_key;

    backup_status -> backup::BackupStatus => backup::backup_status;
    backup_now -> backup::BackupFile => backup::backup_now;

    seed_demo_data -> () => crate::db::seed_demo_data;
}
