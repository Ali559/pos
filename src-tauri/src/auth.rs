//! Sign-in, sessions and permission checks.
//!
//! Every command that changes data calls [`require`] first. The session only
//! stores the user id; role and permissions are re-read from the database on
//! each check, so edits to a role apply immediately.

use std::collections::HashSet;

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use serde::{Deserialize, Serialize};

use crate::ctx::{now, Ctx, SessionKey};
use crate::error::{Error, Result};
use crate::{audit, settings, shifts};

pub const OWNER_ROLE_ID: i64 = 1;
pub const MIN_PASSWORD_LEN: usize = 6;

// ---------------------------------------------------------------- secrets

pub fn hash_secret(secret: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| Error::Hash)
}

pub fn verify_secret(secret: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(secret.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

pub fn validate_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(Error::Rule("password_short"));
    }
    Ok(())
}

pub fn validate_pin(pin: &str) -> Result<()> {
    if !(4..=6).contains(&pin.len()) || !pin.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Rule("pin_format"));
    }
    Ok(())
}

pub fn validate_username(username: &str) -> Result<()> {
    let ok = (2..=32).contains(&username.len())
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !ok {
        return Err(Error::Rule("username_format"));
    }
    Ok(())
}

// ------------------------------------------------------------ the actor

/// The signed-in user as far as permission checks are concerned.
#[derive(Debug, Clone)]
pub struct Actor {
    pub user_id: i64,
    #[allow(dead_code)]
    pub role_id: i64,
    pub is_owner: bool,
    pub permissions: HashSet<String>,
}

impl Actor {
    pub fn can(&self, permission: &str) -> bool {
        self.is_owner || self.permissions.contains(permission)
    }
}

pub async fn all_permission_keys(ctx: &Ctx) -> Result<HashSet<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT key FROM permissions")
        .fetch_all(&ctx.pool)
        .await?;
    Ok(rows.into_iter().map(|(k,)| k).collect())
}

/// Permissions a role grants. The system (Owner) role grants all of them.
pub async fn role_permissions(ctx: &Ctx, role_id: i64) -> Result<(bool, HashSet<String>)> {
    let (is_system,): (bool,) = sqlx::query_as("SELECT is_system FROM roles WHERE id = $1")
        .bind(role_id)
        .fetch_optional(&ctx.pool)
        .await?
        .ok_or(Error::NotFound)?;
    if is_system {
        return Ok((true, all_permission_keys(ctx).await?));
    }
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT permission_key FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .fetch_all(&ctx.pool)
            .await?;
    Ok((false, rows.into_iter().map(|(k,)| k).collect()))
}

/// The signed-in user, re-read from the database. A deactivated user is
/// signed out on their next action.
pub async fn current(ctx: &Ctx) -> Result<Actor> {
    let key = ctx.session.get().ok_or(Error::NotSignedIn)?;
    let row: Option<(i64, bool)> =
        sqlx::query_as("SELECT role_id, is_active FROM users WHERE id = $1")
            .bind(key.user_id)
            .fetch_optional(&ctx.pool)
            .await?;
    let Some((role_id, true)) = row else {
        ctx.session.set(None);
        return Err(Error::NotSignedIn);
    };
    let (is_owner, permissions) = role_permissions(ctx, role_id).await?;
    Ok(Actor {
        user_id: key.user_id,
        role_id,
        is_owner,
        permissions,
    })
}

/// The signed-in user, if they hold `permission`.
pub async fn require(ctx: &Ctx, permission: &str) -> Result<Actor> {
    let actor = current(ctx).await?;
    if actor.can(permission) {
        Ok(actor)
    } else {
        Err(Error::Forbidden(permission.to_owned()))
    }
}

/// First run: nobody can sign in until an owner account exists.
pub async fn needs_setup(ctx: &Ctx) -> Result<bool> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&ctx.pool)
        .await?;
    Ok(n == 0)
}

// ----------------------------------------------------------- status DTOs

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SessionUser {
    pub id: i64,
    pub full_name: String,
    pub username: String,
    pub role_id: i64,
    #[serde(rename = "roleName")]
    pub role_name_en: String,
    #[serde(rename = "roleName_ku")]
    pub role_name_ku: String,
    #[serde(rename = "roleName_ar")]
    pub role_name_ar: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub user: SessionUser,
    pub is_owner: bool,
    pub permissions: Vec<String>,
    pub shift: Option<shifts::ShiftInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub needs_setup: bool,
    pub session: Option<SessionInfo>,
}

pub async fn status(ctx: &Ctx) -> Result<AuthStatus> {
    let needs_setup = needs_setup(ctx).await?;
    let session = match current(ctx).await {
        Ok(actor) => {
            let user: SessionUser = sqlx::query_as(
                "SELECT u.id, u.full_name, u.username, u.role_id,
                        r.name_en AS role_name_en, r.name_ku AS role_name_ku, r.name_ar AS role_name_ar
                 FROM users u JOIN roles r ON r.id = u.role_id WHERE u.id = $1",
            )
            .bind(actor.user_id)
            .fetch_one(&ctx.pool)
            .await?;
            let mut permissions: Vec<String> = actor.permissions.into_iter().collect();
            permissions.sort();
            Some(SessionInfo {
                user,
                is_owner: actor.is_owner,
                permissions,
                shift: shifts::shift_info(ctx, actor.user_id).await?,
            })
        }
        Err(Error::NotSignedIn) => None,
        Err(e) => return Err(e),
    };
    Ok(AuthStatus {
        needs_setup,
        session,
    })
}

// --------------------------------------------------------------- actions

async fn sign_in(ctx: &Ctx, user_id: i64, how: &str) -> Result<AuthStatus> {
    sqlx::query("UPDATE users SET last_login = $1 WHERE id = $2")
        .bind(now())
        .bind(user_id)
        .execute(&ctx.pool)
        .await?;
    ctx.session.set(Some(SessionKey { user_id }));
    shifts::start_or_resume(ctx, user_id).await?;
    audit::log(ctx, Some(user_id), "login", "users", Some(user_id), &serde_json::json!({ "via": how })).await?;
    status(ctx).await
}

#[derive(Debug, Deserialize)]
pub struct LoginArgs {
    pub username: String,
    pub password: String,
}

pub async fn login(ctx: &Ctx, args: LoginArgs) -> Result<AuthStatus> {
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, password_hash FROM users WHERE username = $1 AND is_active = 1",
    )
    .bind(args.username.trim())
    .fetch_optional(&ctx.pool)
    .await?;
    match row {
        Some((id, hash)) if verify_secret(&args.password, &hash) => sign_in(ctx, id, "password").await,
        _ => Err(Error::BadCredentials),
    }
}

#[derive(Debug, Deserialize)]
pub struct PinArgs {
    pub pin: String,
}

/// Finds the active user whose PIN matches. PINs are salted hashes, so this
/// checks each one; fine for a shop's handful of staff.
pub async fn find_by_pin(ctx: &Ctx, pin: &str, except_user: Option<i64>) -> Result<Option<i64>> {
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, pin_hash FROM users WHERE pin_hash IS NOT NULL AND is_active = 1",
    )
    .fetch_all(&ctx.pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter(|(id, _)| Some(*id) != except_user)
        .find(|(_, hash)| verify_secret(pin, hash))
        .map(|(id, _)| id))
}

pub async fn login_pin(ctx: &Ctx, args: PinArgs) -> Result<AuthStatus> {
    validate_pin(&args.pin).map_err(|_| Error::BadCredentials)?;
    match find_by_pin(ctx, &args.pin, None).await? {
        Some(id) => sign_in(ctx, id, "pin").await,
        None => Err(Error::BadCredentials),
    }
}

/// Signs out but leaves the shift open (cashiers swap at the till).
pub async fn logout(ctx: &Ctx) -> Result<()> {
    if let Some(key) = ctx.session.get() {
        audit::log(ctx, Some(key.user_id), "logout", "users", Some(key.user_id), &serde_json::Value::Null).await?;
    }
    ctx.session.set(None);
    Ok(())
}

/// Clocks the signed-in user out and signs them out.
pub async fn end_shift(ctx: &Ctx) -> Result<()> {
    let actor = current(ctx).await?;
    shifts::clock_out(ctx, actor.user_id).await?;
    logout(ctx).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerInput {
    pub full_name: String,
    pub username: String,
    pub password: String,
    pub pin: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetupArgs {
    pub shop: settings::ShopProfileInput,
    pub owner: OwnerInput,
}

/// First run: saves the shop profile, creates the owner and signs them in.
/// Refused once any user exists.
pub async fn complete_setup(ctx: &Ctx, args: SetupArgs) -> Result<AuthStatus> {
    if !needs_setup(ctx).await? {
        return Err(Error::Rule("setup_done"));
    }
    let owner = args.owner;
    let full_name = owner.full_name.trim();
    if full_name.is_empty() {
        return Err(Error::Rule("name_required"));
    }
    validate_username(owner.username.trim())?;
    validate_password(&owner.password)?;
    let pin_hash = match owner.pin.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(pin) => {
            validate_pin(pin)?;
            Some(hash_secret(pin)?)
        }
        None => None,
    };

    settings::upsert_shop_profile(ctx, &args.shop).await?;
    let id = sqlx::query(
        "INSERT INTO users (username, password_hash, pin_hash, full_name, role_id)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(owner.username.trim())
    .bind(hash_secret(&owner.password)?)
    .bind(pin_hash)
    .bind(full_name)
    .bind(OWNER_ROLE_ID)
    .execute(&ctx.pool)
    .await?
    .last_insert_rowid();
    audit::log(ctx, Some(id), "setup", "users", Some(id), &serde_json::Value::Null).await?;
    sign_in(ctx, id, "setup").await
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;

    /// Runs first-run setup: owner "owner" / "secret1", PIN 1234, signed in.
    pub async fn setup_owner(ctx: &Ctx) -> AuthStatus {
        complete_setup(
            ctx,
            SetupArgs {
                shop: settings::ShopProfileInput::named("Shop", "دووکان"),
                owner: OwnerInput {
                    full_name: "Owner".into(),
                    username: "owner".into(),
                    password: "secret1".into(),
                    pin: Some("1234".into()),
                },
            },
        )
        .await
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::setup_owner;
    use super::*;
    use crate::ctx::test_support;

    #[test]
    fn setup_signs_in_owner_once() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            assert!(status(&ctx).await.unwrap().needs_setup);

            let st = setup_owner(&ctx).await;
            let session = st.session.unwrap();
            assert!(!st.needs_setup);
            assert!(session.is_owner);
            assert!(session.permissions.contains(&"roles.manage".to_string()));
            assert!(session.shift.is_some(), "signing in starts a shift");

            let again = complete_setup(
                &ctx,
                SetupArgs {
                    shop: settings::ShopProfileInput::named("x", "x"),
                    owner: OwnerInput {
                        full_name: "x".into(),
                        username: "x2".into(),
                        password: "secret1".into(),
                        pin: None,
                    },
                },
            )
            .await;
            assert!(matches!(again, Err(Error::Rule("setup_done"))));
        });
    }

    #[test]
    fn password_and_pin_login() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            logout(&ctx).await.unwrap();
            assert!(status(&ctx).await.unwrap().session.is_none());

            let bad = login(&ctx, LoginArgs { username: "owner".into(), password: "nope".into() }).await;
            assert!(matches!(bad, Err(Error::BadCredentials)));
            // Usernames are case-insensitive
            login(&ctx, LoginArgs { username: "OWNER".into(), password: "secret1".into() })
                .await
                .unwrap();
            logout(&ctx).await.unwrap();

            assert!(matches!(
                login_pin(&ctx, PinArgs { pin: "9999".into() }).await,
                Err(Error::BadCredentials)
            ));
            let st = login_pin(&ctx, PinArgs { pin: "1234".into() }).await.unwrap();
            assert_eq!(st.session.unwrap().user.username, "owner");
        });
    }

    #[test]
    fn logout_keeps_shift_end_shift_closes_it() {
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            let first = setup_owner(&ctx).await.session.unwrap().shift.unwrap().id;
            logout(&ctx).await.unwrap();
            let resumed = login_pin(&ctx, PinArgs { pin: "1234".into() })
                .await
                .unwrap()
                .session
                .unwrap()
                .shift
                .unwrap()
                .id;
            assert_eq!(first, resumed);

            end_shift(&ctx).await.unwrap();
            assert!(ctx.session.get().is_none());
            let next = login_pin(&ctx, PinArgs { pin: "1234".into() })
                .await
                .unwrap()
                .session
                .unwrap()
                .shift
                .unwrap()
                .id;
            assert_ne!(first, next);
        });
    }

    #[test]
    fn validators() {
        assert!(validate_pin("1234").is_ok());
        assert!(validate_pin("123").is_err());
        assert!(validate_pin("12a4").is_err());
        assert!(validate_username("rebin.a").is_ok());
        assert!(validate_username("ربین").is_err());
        assert!(validate_password("12345").is_err());
    }
}
