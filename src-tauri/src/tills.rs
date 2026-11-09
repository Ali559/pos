//! Settings → Tills: make this computer the main one, pair extra tills
//! with it, or make this computer an extra till of another one.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::auth;
use crate::audit;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::remote;
use crate::server::{self, LanServer};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanStatus {
    /// standalone | main | till
    pub role: &'static str,
    pub version: String,
    /// Main: listening, on which port and addresses
    pub running: bool,
    pub port: u16,
    pub addresses: Vec<String>,
    pub pairing_code: Option<String>,
    pub pairing_seconds: Option<u64>,
    /// Till: the main computer it uses, and its name for this till
    pub server: Option<String>,
    pub till_name: Option<String>,
}

fn state(app: &AppHandle) -> Arc<LanServer> {
    app.state::<Arc<LanServer>>().inner().clone()
}

#[tauri::command]
pub async fn lan_status(app: AppHandle) -> Result<LanStatus> {
    let version = app.package_info().version.to_string();
    if let Some(c) = remote::client(&app) {
        return Ok(LanStatus {
            role: "till",
            version,
            running: false,
            port: 0,
            addresses: vec![],
            pairing_code: None,
            pairing_seconds: None,
            server: Some(c.link.server.clone()),
            till_name: Some(c.link.name.clone()),
        });
    }
    let s = state(&app);
    let code = s.current_code();
    Ok(LanStatus {
        role: if s.running() { "main" } else { "standalone" },
        version,
        running: s.running(),
        port: s.port(),
        addresses: server::local_addresses(),
        pairing_code: code.as_ref().map(|c| c.0.clone()),
        pairing_seconds: code.map(|c| c.1),
        server: None,
        till_name: None,
    })
}

async fn main_only(app: &AppHandle) -> Result<Ctx> {
    if remote::client(app).is_some() {
        return Err(Error::Rule("main_only"));
    }
    Ctx::from_app(app).await
}

#[derive(Debug, Deserialize)]
pub struct EnabledArgs {
    pub enabled: bool,
}

/// Makes this computer the main one (or stops serving tills).
#[tauri::command]
pub async fn set_lan_server(app: AppHandle, args: EnabledArgs) -> Result<LanStatus> {
    let ctx = main_only(&app).await?;
    let actor = auth::require(&ctx, "settings.system").await?;
    if args.enabled {
        server::start(&app, server::DEFAULT_PORT)?;
    } else {
        state(&app).stop();
    }
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES ('lan_server', $1, $2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(if args.enabled { "1" } else { "0" })
    .bind(now())
    .execute(&ctx.pool)
    .await?;
    audit::log(&ctx, Some(actor.user_id), "setting", "settings", None, &serde_json::json!({ "lan_server": if args.enabled { "1" } else { "0" } })).await?;
    lan_status(app).await
}

#[tauri::command]
pub async fn new_pairing_code(app: AppHandle) -> Result<LanStatus> {
    let ctx = main_only(&app).await?;
    auth::require(&ctx, "settings.system").await?;
    if !state(&app).running() {
        return Err(Error::Rule("server_off"));
    }
    state(&app).new_code();
    lan_status(app).await
}

#[derive(Debug, Deserialize)]
pub struct TillIdArgs {
    pub id: i64,
}

/// The till can't reach this computer any more; it has to be paired again.
#[tauri::command]
pub async fn revoke_till(app: AppHandle, args: TillIdArgs) -> Result<()> {
    let ctx = main_only(&app).await?;
    let actor = auth::require(&ctx, "settings.system").await?;
    let done = sqlx::query("UPDATE tills SET revoked_at = $1 WHERE id = $2 AND revoked_at IS NULL")
        .bind(now())
        .bind(args.id)
        .execute(&ctx.pool)
        .await?;
    if done.rows_affected() == 0 {
        return Err(Error::NotFound);
    }
    state(&app).forget(args.id);
    audit::log(&ctx, Some(actor.user_id), "till.revoke", "tills", Some(args.id), &serde_json::Value::Null).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairArgs {
    pub address: String,
    pub code: String,
    pub name: String,
}

/// Makes this computer an extra till of the main computer at `address`,
/// then restarts into that mode. On a computer that is already set up,
/// only someone who may change system settings can do this: the shop's
/// data on this computer stays here but is no longer used.
#[tauri::command]
pub async fn pair_with_main(app: AppHandle, args: PairArgs) -> Result<()> {
    let ctx = main_only(&app).await?;
    if !auth::needs_setup(&ctx).await? {
        auth::require(&ctx, "settings.system").await?;
    }
    if state(&app).running() {
        return Err(Error::Rule("is_main"));
    }
    let server = remote::with_port(&args.address);
    let version = app.package_info().version.to_string();
    let name = args.name.trim();
    if name.is_empty() {
        return Err(Error::Rule("till_name_required"));
    }
    let reply = remote::pair(&server, &args.code, name, &version).await?;
    remote::save(&app, Some(&remote::Link { server, token: reply.token, till_id: reply.till_id, name: name.to_owned() }))?;
    app.restart();
}

/// Back to a computer of its own (its own data, as it was before pairing).
#[tauri::command]
pub async fn leave_main(app: AppHandle) -> Result<()> {
    let Some(client) = remote::client(&app) else {
        return Err(Error::Rule("not_a_till"));
    };
    // Whoever is signed in must be allowed; if the main computer can't be
    // reached at all, leaving is still possible (it may be gone for good)
    match client.whoami().await {
        Ok(actor) if actor.can("settings.system") => {}
        Ok(_) => return Err(Error::Forbidden("settings.system".into())),
        Err(Error::Rule("main_unreachable")) => {}
        Err(e) => return Err(e),
    }
    remote::save(&app, None)?;
    app.restart();
}

/// Is there a main computer at this address? For the pairing form.
#[tauri::command]
pub async fn probe_main(args: ProbeArgs) -> Result<serde_json::Value> {
    remote::ping(&remote::with_port(&args.address)).await
}

#[derive(Debug, Deserialize)]
pub struct ProbeArgs {
    pub address: String,
}
