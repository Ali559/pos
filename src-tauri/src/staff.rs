//! Roles and employees.
//!
//! Besides the permission each command requires, two rules stop privilege
//! escalation by anyone who isn't an Owner:
//! - they can't grant a permission they don't hold (in a role, or by giving
//!   someone a role that has it);
//! - they can't edit an employee whose role holds permissions they don't.

use std::collections::HashSet;

use serde::Deserialize;

use crate::audit;
use crate::auth::{self, Actor, OWNER_ROLE_ID};
use crate::error::{Error, Result};
use crate::ctx::Ctx;

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.is_unique_violation())
}

/// Refuses if `permissions` contains anything `actor` lacks.
fn ensure_within(actor: &Actor, permissions: &HashSet<String>) -> Result<()> {
    if actor.is_owner || permissions.is_subset(&actor.permissions) {
        Ok(())
    } else {
        Err(Error::Rule("above_your_permissions"))
    }
}

async fn ensure_role_within(ctx: &Ctx, actor: &Actor, role_id: i64) -> Result<()> {
    let (is_system, perms) = auth::role_permissions(ctx, role_id).await?;
    if is_system && !actor.is_owner {
        return Err(Error::Rule("above_your_permissions"));
    }
    ensure_within(actor, &perms)
}

// ------------------------------------------------------------------ roles

#[derive(Debug, Deserialize)]
pub struct RoleInput {
    pub name_en: String,
    pub name_ku: String,
    pub name_ar: Option<String>,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

async fn validate_role(ctx: &Ctx, actor: &Actor, role: &RoleInput) -> Result<HashSet<String>> {
    if role.name_en.trim().is_empty() || role.name_ku.trim().is_empty() {
        return Err(Error::Rule("role_names_required"));
    }
    let wanted: HashSet<String> = role.permissions.iter().cloned().collect();
    let known = auth::all_permission_keys(ctx).await?;
    if let Some(unknown) = wanted.difference(&known).next() {
        return Err(Error::Invalid(format!("unknown permission {unknown}")));
    }
    ensure_within(actor, &wanted)?;
    Ok(wanted)
}

fn blank_to_none(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

async fn write_role_permissions(ctx: &Ctx, role_id: i64, perms: &HashSet<String>) -> Result<()> {
    sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
        .bind(role_id)
        .execute(&ctx.pool)
        .await?;
    for key in perms {
        sqlx::query("INSERT INTO role_permissions (role_id, permission_key) VALUES ($1, $2)")
            .bind(role_id)
            .bind(key)
            .execute(&ctx.pool)
            .await?;
    }
    Ok(())
}

pub async fn create_role(ctx: &Ctx, role: RoleInput) -> Result<i64> {
    let actor = auth::require(ctx, "roles.manage").await?;
    let perms = validate_role(ctx, &actor, &role).await?;
    let id = sqlx::query("INSERT INTO roles (name_en, name_ku, name_ar, description) VALUES ($1, $2, $3, $4)")
        .bind(role.name_en.trim())
        .bind(role.name_ku.trim())
        .bind(blank_to_none(&role.name_ar))
        .bind(blank_to_none(&role.description))
        .execute(&ctx.pool)
        .await?
        .last_insert_rowid();
    write_role_permissions(ctx, id, &perms).await?;
    audit::log(ctx, Some(actor.user_id), "role.create", "roles", Some(id), &serde_json::json!({ "name": role.name_en, "permissions": role.permissions })).await?;
    Ok(id)
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleArgs {
    pub id: i64,
    pub role: RoleInput,
}

pub async fn update_role(ctx: &Ctx, args: UpdateRoleArgs) -> Result<()> {
    let actor = auth::require(ctx, "roles.manage").await?;
    let (is_system, current) = auth::role_permissions(ctx, args.id).await?;
    if is_system {
        return Err(Error::Rule("system_role"));
    }
    // Can't edit a role that already holds more than you do, either
    ensure_within(&actor, &current)?;
    let perms = validate_role(ctx, &actor, &args.role).await?;
    sqlx::query("UPDATE roles SET name_en = $1, name_ku = $2, name_ar = $3, description = $4 WHERE id = $5")
        .bind(args.role.name_en.trim())
        .bind(args.role.name_ku.trim())
        .bind(blank_to_none(&args.role.name_ar))
        .bind(blank_to_none(&args.role.description))
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    write_role_permissions(ctx, args.id, &perms).await?;
    audit::log(ctx, Some(actor.user_id), "role.update", "roles", Some(args.id), &serde_json::json!({ "name": args.role.name_en, "permissions": args.role.permissions })).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRoleArgs {
    pub id: i64,
    /// Required when employees still have this role
    pub reassign_to: Option<i64>,
}

pub async fn delete_role(ctx: &Ctx, args: DeleteRoleArgs) -> Result<()> {
    let actor = auth::require(ctx, "roles.manage").await?;
    let (is_system, current) = auth::role_permissions(ctx, args.id).await?;
    if is_system {
        return Err(Error::Rule("system_role"));
    }
    ensure_within(&actor, &current)?;
    let (users,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role_id = $1")
        .bind(args.id)
        .fetch_one(&ctx.pool)
        .await?;
    if users > 0 {
        let target = args.reassign_to.filter(|t| *t != args.id).ok_or(Error::Rule("role_in_use"))?;
        ensure_role_within(ctx, &actor, target).await?;
        sqlx::query("UPDATE users SET role_id = $1 WHERE role_id = $2")
            .bind(target)
            .bind(args.id)
            .execute(&ctx.pool)
            .await?;
    }
    sqlx::query("DELETE FROM roles WHERE id = $1")
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    audit::log(ctx, Some(actor.user_id), "role.delete", "roles", Some(args.id), &serde_json::json!({ "reassignedTo": args.reassign_to })).await
}

// ------------------------------------------------------------- employees

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInput {
    pub full_name: String,
    pub username: String,
    pub role_id: i64,
    pub phone: Option<String>,
    pub photo_media_id: Option<i64>,
    pub hire_date: Option<String>,
    pub wage_amount: Option<f64>,
    pub wage_period: Option<String>,
    pub is_active: bool,
    /// Required on create; on update, `None` or empty keeps the current one
    pub password: Option<String>,
    /// `None` keeps the current PIN, `Some("")` removes it
    pub pin: Option<String>,
}

async fn validate_user(ctx: &Ctx, actor: &Actor, u: &UserInput, user_id: Option<i64>) -> Result<()> {
    if u.full_name.trim().is_empty() {
        return Err(Error::Rule("name_required"));
    }
    auth::validate_username(u.username.trim())?;
    if let Some(p) = u.wage_period.as_deref() {
        if !matches!(p, "hour" | "month") {
            return Err(Error::Invalid(format!("unknown wage period {p}")));
        }
    }
    if let Some(pin) = u.pin.as_deref().filter(|p| !p.is_empty()) {
        auth::validate_pin(pin)?;
        // A PIN alone signs someone in, so it must be unique
        if auth::find_by_pin(ctx, pin, user_id).await?.is_some() {
            return Err(Error::Rule("pin_taken"));
        }
    }
    ensure_role_within(ctx, actor, u.role_id).await
}

fn map_unique(e: sqlx::Error) -> Error {
    if is_unique_violation(&e) {
        Error::Rule("username_taken")
    } else {
        e.into()
    }
}

pub async fn create_user(ctx: &Ctx, u: UserInput) -> Result<i64> {
    let actor = auth::require(ctx, "staff.edit").await?;
    validate_user(ctx, &actor, &u, None).await?;
    let password = u.password.as_deref().unwrap_or_default();
    auth::validate_password(password)?;
    let pin_hash = match u.pin.as_deref().filter(|p| !p.is_empty()) {
        Some(pin) => Some(auth::hash_secret(pin)?),
        None => None,
    };
    let id = sqlx::query(
        "INSERT INTO users (username, password_hash, pin_hash, full_name, role_id, phone,
            photo_media_id, hire_date, wage_amount, wage_period, is_active)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(u.username.trim())
    .bind(auth::hash_secret(password)?)
    .bind(pin_hash)
    .bind(u.full_name.trim())
    .bind(u.role_id)
    .bind(blank_to_none(&u.phone))
    .bind(u.photo_media_id)
    .bind(blank_to_none(&u.hire_date))
    .bind(u.wage_amount)
    .bind(u.wage_amount.and(u.wage_period.as_deref()))
    .bind(u.is_active)
    .execute(&ctx.pool)
    .await
    .map_err(map_unique)?
    .last_insert_rowid();
    audit::log(ctx, Some(actor.user_id), "user.create", "users", Some(id), &serde_json::json!({ "username": u.username, "role": u.role_id })).await?;
    Ok(id)
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserArgs {
    pub id: i64,
    pub user: UserInput,
}

/// Active owners other than `user_id`.
async fn other_owners(ctx: &Ctx, user_id: i64) -> Result<i64> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM users WHERE role_id = $1 AND is_active = 1 AND id != $2",
    )
    .bind(OWNER_ROLE_ID)
    .bind(user_id)
    .fetch_one(&ctx.pool)
    .await?;
    Ok(n)
}

pub async fn update_user(ctx: &Ctx, args: UpdateUserArgs) -> Result<()> {
    let actor = auth::require(ctx, "staff.edit").await?;
    let u = &args.user;
    let (current_role,): (i64,) = sqlx::query_as("SELECT role_id FROM users WHERE id = $1")
        .bind(args.id)
        .fetch_optional(&ctx.pool)
        .await?
        .ok_or(Error::NotFound)?;
    // Can't touch someone who outranks you
    ensure_role_within(ctx, &actor, current_role).await?;
    validate_user(ctx, &actor, u, Some(args.id)).await?;

    let stays_owner = u.role_id == OWNER_ROLE_ID && u.is_active;
    if current_role == OWNER_ROLE_ID && !stays_owner && other_owners(ctx, args.id).await? == 0 {
        return Err(Error::Rule("last_owner"));
    }
    if args.id == actor.user_id && !u.is_active {
        return Err(Error::Rule("cannot_deactivate_self"));
    }

    sqlx::query(
        "UPDATE users SET username = $1, full_name = $2, role_id = $3, phone = $4,
            photo_media_id = $5, hire_date = $6, wage_amount = $7, wage_period = $8, is_active = $9
         WHERE id = $10",
    )
    .bind(u.username.trim())
    .bind(u.full_name.trim())
    .bind(u.role_id)
    .bind(blank_to_none(&u.phone))
    .bind(u.photo_media_id)
    .bind(blank_to_none(&u.hire_date))
    .bind(u.wage_amount)
    .bind(u.wage_amount.and(u.wage_period.as_deref()))
    .bind(u.is_active)
    .bind(args.id)
    .execute(&ctx.pool)
    .await
    .map_err(map_unique)?;

    if let Some(password) = u.password.as_deref().filter(|p| !p.is_empty()) {
        auth::validate_password(password)?;
        sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(auth::hash_secret(password)?)
            .bind(args.id)
            .execute(&ctx.pool)
            .await?;
    }
    match u.pin.as_deref() {
        None => {}
        Some("") => {
            sqlx::query("UPDATE users SET pin_hash = NULL WHERE id = $1")
                .bind(args.id)
                .execute(&ctx.pool)
                .await?;
        }
        Some(pin) => {
            sqlx::query("UPDATE users SET pin_hash = $1 WHERE id = $2")
                .bind(auth::hash_secret(pin)?)
                .bind(args.id)
                .execute(&ctx.pool)
                .await?;
        }
    }
    audit::log(ctx, Some(actor.user_id), "user.update", "users", Some(args.id), &serde_json::json!({
        "username": u.username, "role": u.role_id, "active": u.is_active,
        "passwordChanged": u.password.as_deref().is_some_and(|p| !p.is_empty()),
        "pinChanged": u.pin.is_some(),
    })).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_helpers::setup_owner;
    use crate::auth::{login, logout, LoginArgs};
    use crate::ctx::test_support;

    const MANAGER: i64 = 2;
    const CASHIER: i64 = 3;

    fn user(username: &str, role_id: i64) -> UserInput {
        UserInput {
            full_name: username.to_uppercase(),
            username: username.into(),
            role_id,
            phone: None,
            photo_media_id: None,
            hire_date: None,
            wage_amount: None,
            wage_period: None,
            is_active: true,
            password: Some("secret1".into()),
            pin: None,
        }
    }

    async fn sign_in_as(ctx: &Ctx, username: &str) {
        logout(ctx).await.unwrap();
        login(ctx, LoginArgs { username: username.into(), password: "secret1".into() })
            .await
            .unwrap();
    }

    #[test]
    fn cashier_cannot_manage_staff() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            create_user(&ctx, user("cash", CASHIER)).await.unwrap();
            sign_in_as(&ctx, "cash").await;
            let r = create_user(&ctx, user("x", CASHIER)).await;
            assert!(matches!(r, Err(Error::Forbidden(p)) if p == "staff.edit"));
        });
    }

    #[test]
    fn non_owner_cannot_escalate() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            // A manager who may edit staff and roles, but lacks settings.system
            update_role(&ctx, UpdateRoleArgs {
                id: MANAGER,
                role: RoleInput {
                    name_en: "Manager".into(),
                    name_ku: "بەڕێوەبەر".into(),
                    name_ar: None,
                    description: None,
                    permissions: ["staff.edit", "staff.view", "roles.manage", "sales.sell", "sales.reprint", "inventory.view"]
                        .map(String::from)
                        .to_vec(),
                },
            })
            .await
            .unwrap();
            create_user(&ctx, user("boss", MANAGER)).await.unwrap();
            sign_in_as(&ctx, "boss").await;

            // Can hire a cashier…
            create_user(&ctx, user("cash", CASHIER)).await.unwrap();
            // …but not an owner
            assert!(matches!(create_user(&ctx, user("o2", OWNER_ROLE_ID)).await, Err(Error::Rule("above_your_permissions"))));
            // …nor a role with permissions they lack
            let r = create_role(&ctx, RoleInput {
                name_en: "Admin".into(),
                name_ku: "ئەدمین".into(),
                name_ar: None,
                description: None,
                permissions: vec!["settings.system".into()],
            })
            .await;
            assert!(matches!(r, Err(Error::Rule("above_your_permissions"))));
            // …nor edit the owner
            let mut o = user("owner", OWNER_ROLE_ID);
            o.password = None;
            assert!(matches!(update_user(&ctx, UpdateUserArgs { id: 1, user: o }).await, Err(Error::Rule("above_your_permissions"))));
        });
    }

    #[test]
    fn last_owner_and_unique_rules() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let mut me = user("owner", CASHIER);
            me.password = None;
            assert!(matches!(update_user(&ctx, UpdateUserArgs { id: 1, user: me }).await, Err(Error::Rule("last_owner"))));

            assert!(matches!(create_user(&ctx, user("OWNER", CASHIER)).await, Err(Error::Rule("username_taken"))));
            let mut dup_pin = user("pinny", CASHIER);
            dup_pin.pin = Some("1234".into()); // the owner's PIN
            assert!(matches!(create_user(&ctx, dup_pin).await, Err(Error::Rule("pin_taken"))));
        });
    }

    #[test]
    fn deleting_a_role_in_use_needs_a_replacement() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let cid = create_user(&ctx, user("cash", CASHIER)).await.unwrap();
            assert!(matches!(
                delete_role(&ctx, DeleteRoleArgs { id: CASHIER, reassign_to: None }).await,
                Err(Error::Rule("role_in_use"))
            ));
            delete_role(&ctx, DeleteRoleArgs { id: CASHIER, reassign_to: Some(MANAGER) }).await.unwrap();
            let (role,): (i64,) = sqlx::query_as("SELECT role_id FROM users WHERE id = $1")
                .bind(cid)
                .fetch_one(&ctx.pool)
                .await
                .unwrap();
            assert_eq!(role, MANAGER);
            assert!(matches!(
                delete_role(&ctx, DeleteRoleArgs { id: OWNER_ROLE_ID, reassign_to: None }).await,
                Err(Error::Rule("system_role"))
            ));
        });
    }

    #[test]
    fn deactivated_user_is_signed_out() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            let cid = create_user(&ctx, user("cash", CASHIER)).await.unwrap();
            sign_in_as(&ctx, "cash").await;
            sqlx::query("UPDATE users SET is_active = 0 WHERE id = $1")
                .bind(cid)
                .execute(&ctx.pool)
                .await
                .unwrap();
            assert!(matches!(auth::current(&ctx).await, Err(Error::NotSignedIn)));
            assert!(ctx.session.get().is_none());
        });
    }
}
