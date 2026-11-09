//! The main computer: serves the other tills on the shop's network.
//!
//! Switched on in Settings → Tills (`lan_server`). It listens on
//! `0.0.0.0:7420` and answers only paired tills: each request carries the
//! till's secret, whose SHA-256 is in `tills`. A till is paired once with a
//! six-digit code shown on this computer for ten minutes. Every till gets
//! its own sign-in session; the commands are the same ones the app window
//! uses (`commands::dispatch`), so every permission check still happens here.
//!
//! Traffic is plain HTTP on the local network: keep the tills on the shop's
//! own network (cable or a Wi-Fi with a password), never on a public one.

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::commands;
use crate::ctx::{now, Ctx, SessionState};
use crate::error::{Error, Result};
use crate::media;

pub const DEFAULT_PORT: u16 = 7420;
const PAIRING_TTL: Duration = Duration::from_secs(10 * 60);
const PAIRING_TRIES: u32 = 5;
/// Commands a till may not ask the main computer to run
const REFUSED: &[&str] = &["list_printers", "print_raster", "seed_demo_data"];

struct Pairing {
    code: String,
    expires: Instant,
    tries_left: u32,
}

#[derive(Default)]
pub struct LanServer {
    server: Mutex<Option<Arc<Server>>>,
    port: Mutex<u16>,
    pairing: Mutex<Option<Pairing>>,
    sessions: Mutex<HashMap<i64, Arc<SessionState>>>,
}

impl LanServer {
    pub fn running(&self) -> bool {
        self.server.lock().unwrap().is_some()
    }

    pub fn port(&self) -> u16 {
        match *self.port.lock().unwrap() {
            0 => DEFAULT_PORT,
            p => p,
        }
    }

    fn session(&self, till_id: i64) -> Arc<SessionState> {
        self.sessions.lock().unwrap().entry(till_id).or_default().clone()
    }

    pub fn forget(&self, till_id: i64) {
        self.sessions.lock().unwrap().remove(&till_id);
    }

    /// A new code for pairing a till; replaces any earlier one.
    pub fn new_code(&self) -> (String, u64) {
        let n = u32::from_le_bytes(uuid::Uuid::new_v4().as_bytes()[..4].try_into().unwrap()) % 1_000_000;
        let code = format!("{n:06}");
        *self.pairing.lock().unwrap() = Some(Pairing { code: code.clone(), expires: Instant::now() + PAIRING_TTL, tries_left: PAIRING_TRIES });
        (code, PAIRING_TTL.as_secs())
    }

    /// The current code and seconds left, if there is one.
    pub fn current_code(&self) -> Option<(String, u64)> {
        let p = self.pairing.lock().unwrap();
        p.as_ref()
            .filter(|p| p.expires > Instant::now())
            .map(|p| (p.code.clone(), p.expires.duration_since(Instant::now()).as_secs()))
    }

    /// Single use; a few wrong tries and the code is gone.
    fn take_code(&self, code: &str) -> bool {
        let mut p = self.pairing.lock().unwrap();
        let Some(pairing) = p.as_mut() else { return false };
        if pairing.expires <= Instant::now() {
            *p = None;
            return false;
        }
        if pairing.code == code {
            *p = None;
            return true;
        }
        pairing.tries_left -= 1;
        if pairing.tries_left == 0 {
            *p = None;
        }
        false
    }

    pub fn stop(&self) {
        if let Some(s) = self.server.lock().unwrap().take() {
            s.unblock();
        }
        self.sessions.lock().unwrap().clear();
    }
}

pub fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

/// Starts listening. Errors (the port is taken) are returned so Settings
/// can say so.
pub fn start(app: &AppHandle, port: u16) -> Result<()> {
    let state = app.state::<Arc<LanServer>>().inner().clone();
    if state.running() {
        return Ok(());
    }
    let server = Arc::new(Server::http(("0.0.0.0", port)).map_err(|e| {
        eprintln!("[server] could not listen on {port}: {e}");
        Error::Rule("port_in_use")
    })?);
    *state.port.lock().unwrap() = port;
    *state.server.lock().unwrap() = Some(server.clone());
    let app = app.clone();
    std::thread::spawn(move || {
        println!("[server] serving tills on port {port}");
        for req in server.incoming_requests() {
            let app = app.clone();
            // One thread per request so a slow till doesn't hold up the others
            std::thread::spawn(move || {
                let incoming = Incoming::read(req);
                let (req, reply) = tauri::async_runtime::block_on(async move {
                    let (req, parts) = incoming;
                    (req, handle(&app, parts).await)
                });
                let _ = req.respond(reply);
            });
        }
    });
    Ok(())
}

/// At startup: listen if this computer was set up as the main one.
pub fn start_if_enabled(app: &AppHandle) {
    let read = app.state::<crate::db::ReadPool>().0.clone();
    let row: Option<(Option<String>,)> = tauri::async_runtime::block_on(
        sqlx::query_as("SELECT value FROM settings WHERE key = 'lan_server'").fetch_optional(&read),
    )
    .ok()
    .flatten();
    if row.and_then(|(v,)| v).as_deref() == Some("1") {
        let _ = start(app, DEFAULT_PORT);
    }
}

struct Parts {
    method: Method,
    url: String,
    token: Option<String>,
    version: Option<String>,
    file_name: Option<String>,
    body: Vec<u8>,
}

struct Incoming;

impl Incoming {
    fn read(mut req: Request) -> (Request, Parts) {
        let header = |name: &str| {
            req.headers()
                .iter()
                .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
                .map(|h| h.value.as_str().to_owned())
        };
        let token = header("authorization").and_then(|v| v.strip_prefix("Bearer ").map(str::to_owned));
        let version = header("x-pos-version");
        let file_name = header("x-file-name");
        let mut body = Vec::new();
        // Uploads are capped in media.rs; this is a backstop
        let _ = req.as_reader().take(30 * 1024 * 1024).read_to_end(&mut body);
        let parts = Parts { method: req.method().clone(), url: req.url().to_owned(), token, version, file_name, body };
        (req, parts)
    }
}

type Reply = Response<std::io::Cursor<Vec<u8>>>;

fn json(status: u16, body: &impl Serialize) -> Reply {
    Response::from_data(serde_json::to_vec(body).unwrap_or_default())
        .with_status_code(status)
        .with_header(Header::from_bytes("content-type", "application/json").unwrap())
}

async fn handle(app: &AppHandle, req: Parts) -> Reply {
    match route(app, req).await {
        Ok(r) => r,
        Err(e @ (Error::NotSignedIn | Error::Forbidden(_))) => json(403, &e),
        Err(e @ Error::Rule("till_not_paired")) => json(401, &e),
        Err(e) => json(400, &e),
    }
}

fn version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

/// The paired till a secret belongs to.
async fn till_for(app: &AppHandle, token: Option<&str>, version: Option<&str>) -> Result<i64> {
    let token = token.ok_or(Error::Rule("till_not_paired"))?;
    let pool = crate::db::pool(app).await?;
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM tills WHERE token_hash = $1 AND revoked_at IS NULL")
        .bind(hash_token(token))
        .fetch_optional(&pool)
        .await?;
    let (id,) = row.ok_or(Error::Rule("till_not_paired"))?;
    sqlx::query("UPDATE tills SET last_seen_at = $1, app_version = $2 WHERE id = $3")
        .bind(now())
        .bind(version)
        .bind(id)
        .execute(&pool)
        .await?;
    Ok(id)
}

/// Both ends must run the same version: the screens and the commands change
/// together.
fn same_version(app: &AppHandle, theirs: Option<&str>) -> Result<()> {
    let ours = version(app);
    if theirs == Some(ours.as_str()) {
        Ok(())
    } else {
        Err(Error::RuleAbout("version_mismatch", ours))
    }
}

async fn route(app: &AppHandle, req: Parts) -> Result<Reply> {
    let state = app.state::<Arc<LanServer>>().inner().clone();
    let path = req.url.split('?').next().unwrap_or_default().to_owned();
    match (&req.method, path.as_str()) {
        (Method::Get, "/ping") => {
            let pool = crate::db::pool(app).await?;
            let (shop,): (Option<String>,) = sqlx::query_as("SELECT (SELECT name FROM shop_profile WHERE id = 1)").fetch_one(&pool).await?;
            Ok(json(200, &serde_json::json!({ "app": "pos", "version": version(app), "shop": shop })))
        }
        (Method::Post, "/pair") => {
            same_version(app, req.version.as_deref())?;
            let v: Value = serde_json::from_slice(&req.body)?;
            let code = v.get("code").and_then(Value::as_str).unwrap_or_default();
            let name = v.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Till");
            if !state.take_code(code) {
                return Err(Error::Rule("pairing_code"));
            }
            let token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
            let pool = crate::db::pool(app).await?;
            let id = sqlx::query("INSERT INTO tills (name, token_hash, paired_at, app_version) VALUES ($1, $2, $3, $4)")
                .bind(name.chars().take(40).collect::<String>())
                .bind(hash_token(&token))
                .bind(now())
                .bind(req.version.as_deref())
                .execute(&pool)
                .await?
                .last_insert_rowid();
            let (shop,): (Option<String>,) = sqlx::query_as("SELECT (SELECT name FROM shop_profile WHERE id = 1)").fetch_one(&pool).await?;
            let ctx = Ctx::for_till(app, id, state.session(id)).await?;
            crate::audit::log(&ctx, None, "till.pair", "tills", Some(id), &serde_json::json!({ "name": name })).await?;
            Ok(json(200, &serde_json::json!({ "token": token, "tillId": id, "version": version(app), "shop": shop })))
        }
        (Method::Get, p) if p.starts_with("/media/") => {
            // <img> tags can't send a header: the secret comes in the query
            let token = req.url.split_once("?t=").map(|(_, t)| t.to_owned());
            till_for(app, token.as_deref(), None).await?;
            let file = &p["/media/".len()..];
            if file.is_empty() || file.contains('/') || file.contains("..") || file.contains('\\') {
                return Err(Error::NotFound);
            }
            let bytes = std::fs::read(media::media_dir(app)?.join(file)).map_err(|_| Error::NotFound)?;
            Ok(Response::from_data(bytes)
                .with_header(Header::from_bytes("content-type", "image/webp").unwrap())
                .with_header(Header::from_bytes("cache-control", "max-age=86400").unwrap())
                // So the till can draw the logo onto a receipt canvas
                .with_header(Header::from_bytes("access-control-allow-origin", "*").unwrap()))
        }
        (Method::Post, p) => {
            let till = till_for(app, req.token.as_deref(), req.version.as_deref()).await?;
            same_version(app, req.version.as_deref())?;
            let ctx = Ctx::for_till(app, till, state.session(till)).await?;
            match p {
                "/hello" => {
                    state.forget(till);
                    Ok(json(200, &Value::Null))
                }
                "/upload_media" => Ok(json(200, &media::upload(&ctx, req.body, req.file_name).await?)),
                p if p.starts_with("/invoke/") => {
                    let name = &p["/invoke/".len()..];
                    if REFUSED.contains(&name) {
                        return Err(Error::Invalid(format!("{name} runs on the till itself")));
                    }
                    let v: Value = if req.body.is_empty() { Value::Null } else { serde_json::from_slice(&req.body)? };
                    let args = v.get("args").cloned().unwrap_or(Value::Null);
                    Ok(json(200, &commands::dispatch(&ctx, name, args).await?))
                }
                _ => Err(Error::NotFound),
            }
        }
        _ => Err(Error::NotFound),
    }
}

/// This computer's addresses on the local network, for the till to type in.
pub fn local_addresses() -> Vec<String> {
    // Connecting a UDP socket sends nothing; it only picks the interface
    // that would be used, which is the one on the shop's network
    let mut out = Vec::new();
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("10.255.255.255:1").is_ok() || sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                if !addr.ip().is_loopback() && !addr.ip().is_unspecified() {
                    out.push(addr.ip().to_string());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_codes_are_single_use_and_give_up_after_wrong_tries() {
        let s = LanServer::default();
        let (code, _) = s.new_code();
        assert_eq!(code.len(), 6);
        assert!(s.take_code(&code));
        assert!(!s.take_code(&code), "used once already");

        let (code, _) = s.new_code();
        for _ in 0..PAIRING_TRIES {
            assert!(!s.take_code("nope"));
        }
        assert!(!s.take_code(&code), "too many wrong tries");
    }

    #[test]
    fn addresses_get_the_default_port() {
        assert_eq!(crate::remote::with_port("192.168.1.20"), "192.168.1.20:7420");
        assert_eq!(crate::remote::with_port(" http://192.168.1.20:8000/ "), "192.168.1.20:8000");
    }
}
