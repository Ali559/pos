//! The main computer: serves the other tills on the shop's network.
//!
//! Switched on in Settings → Tills (`lan_server`). It listens on
//! `0.0.0.0:7420` and answers only paired tills: each request carries the
//! till's secret, whose SHA-256 is in `tills`. A till is paired once with a
//! six-digit code shown on this computer for ten minutes. Every till gets
//! its own sign-in session; the commands are the same ones the app window
//! uses (`commands::dispatch`), so every permission check still happens here.
//!
//! Everything on the network is TLS (`tls.rs`): the port takes encrypted
//! connections and passes them to a plain HTTP server that only listens on
//! this computer (127.0.0.1). The tills pin this computer's certificate.
//! A till from before encryption gets a `version_mismatch` reply in plain
//! HTTP, so it says it needs updating.

use std::collections::HashMap;
use std::io::Read;
use std::net::SocketAddr;
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
use crate::tls::{self, Ed25519Group, Spake2};

pub const DEFAULT_PORT: u16 = 7420;
const PAIRING_TTL: Duration = Duration::from_secs(10 * 60);
const PAIRING_TRIES: u32 = 5;
/// Commands a till may not ask the main computer to run
const REFUSED: &[&str] = &["list_printers", "print_raster", "seed_demo_data"];

struct Pairing {
    code: String,
    expires: Instant,
    tries_left: u32,
    /// A till that has started pairing (SPAKE2, see `tls.rs`)
    pending: Option<Pending>,
}

struct Pending {
    id: String,
    spake: Spake2<Ed25519Group>,
    /// The till's SPAKE2 message
    msg: Vec<u8>,
}

#[derive(Default)]
pub struct LanServer {
    server: Mutex<Option<Arc<Server>>>,
    port: Mutex<u16>,
    pairing: Mutex<Option<Pairing>>,
    sessions: Mutex<HashMap<i64, Arc<SessionState>>>,
    /// The TLS listener on the network port
    front: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// SHA-256 of this computer's certificate
    fingerprint: Mutex<String>,
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
        *self.pairing.lock().unwrap() = Some(Pairing { code: code.clone(), expires: Instant::now() + PAIRING_TTL, tries_left: PAIRING_TRIES, pending: None });
        (code, PAIRING_TTL.as_secs())
    }

    /// The current code and seconds left, if there is one.
    pub fn current_code(&self) -> Option<(String, u64)> {
        let p = self.pairing.lock().unwrap();
        p.as_ref()
            .filter(|p| p.expires > Instant::now())
            .map(|p| (p.code.clone(), p.expires.duration_since(Instant::now()).as_secs()))
    }

    /// A till starts pairing with its SPAKE2 message; this computer answers
    /// with its own. Nothing is learned about the code here.
    fn start_pair(&self, msg: Vec<u8>) -> Option<(String, Vec<u8>)> {
        let mut p = self.pairing.lock().unwrap();
        let pairing = p.as_mut().filter(|p| p.expires > Instant::now())?;
        let (spake, out) = tls::spake_main(&pairing.code);
        let id = uuid::Uuid::new_v4().simple().to_string();
        pairing.pending = Some(Pending { id: id.clone(), spake, msg });
        Some((id, out))
    }

    /// The till proves it has the code and sees this computer's certificate.
    /// The code is single use; a few wrong tries and it's gone. Returns the
    /// shared key.
    fn finish_pair(&self, id: &str, confirm: &str, fingerprint: &str) -> Option<Vec<u8>> {
        let mut p = self.pairing.lock().unwrap();
        let pairing = p.as_mut()?;
        if pairing.expires <= Instant::now() {
            *p = None;
            return None;
        }
        let key = pairing.pending.take().filter(|x| x.id == id).and_then(|x| x.spake.finish(&x.msg).ok());
        match key {
            Some(k) if tls::same(confirm, &tls::proof(&k, &[b"till", fingerprint.as_bytes()])) => {
                *p = None;
                Some(k)
            }
            _ => {
                pairing.tries_left -= 1;
                if pairing.tries_left == 0 {
                    *p = None;
                }
                None
            }
        }
    }

    pub fn stop(&self) {
        if let Some(f) = self.front.lock().unwrap().take() {
            f.abort();
        }
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
    let identity = tls::load_or_create(&app.path().app_config_dir()?)?;
    let config = tls::server_config(&identity)?;
    let front = std::net::TcpListener::bind(("0.0.0.0", port)).map_err(|e| {
        eprintln!("[server] could not listen on {port}: {e}");
        Error::Rule("port_in_use")
    })?;
    front.set_nonblocking(true)?;
    // The HTTP side only answers this computer; the network gets TLS
    let server = Arc::new(Server::http("127.0.0.1:0").map_err(|e| Error::Invalid(format!("server: {e}")))?);
    let inner = server.server_addr().to_ip().ok_or_else(|| Error::Invalid("server address".into()))?;
    *state.port.lock().unwrap() = port;
    *state.fingerprint.lock().unwrap() = identity.fingerprint.clone();
    *state.server.lock().unwrap() = Some(server.clone());
    *state.front.lock().unwrap() = Some(tauri::async_runtime::spawn(serve_tls(front, config, inner, version(app))));
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

/// Takes connections on the network port: TLS goes through to the HTTP
/// server; anything else is a till from before encryption.
async fn serve_tls(listener: std::net::TcpListener, config: Arc<rustls::ServerConfig>, inner: SocketAddr, version: String) {
    let listener = match tokio::net::TcpListener::from_std(listener) {
        Ok(l) => l,
        Err(e) => return eprintln!("[server] {e}"),
    };
    let acceptor = tokio_rustls::TlsAcceptor::from(config);
    loop {
        let stream = match listener.accept().await {
            Ok((s, _)) => s,
            Err(e) => {
                // Out of file handles and the like: don't spin
                eprintln!("[server] {e}");
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
        };
        let acceptor = acceptor.clone();
        let version = version.clone();
        tauri::async_runtime::spawn(async move {
            let _ = connection(stream, acceptor, inner, &version).await;
        });
    }
}

async fn within<T>(secs: u64, f: impl std::future::Future<Output = std::io::Result<T>>) -> std::io::Result<T> {
    tokio::time::timeout(Duration::from_secs(secs), f).await.map_err(|_| std::io::ErrorKind::TimedOut)?
}

async fn connection(stream: tokio::net::TcpStream, acceptor: tokio_rustls::TlsAcceptor, inner: SocketAddr, version: &str) -> std::io::Result<()> {
    let mut first = [0u8; 1];
    if within(10, stream.peek(&mut first)).await? == 0 {
        return Ok(());
    }
    // 0x16: the first byte of a TLS handshake
    if first[0] != 0x16 {
        return refuse_plain(stream, version).await;
    }
    let mut tls = within(10, acceptor.accept(stream)).await?;
    let mut back = tokio::net::TcpStream::connect(inner).await?;
    tokio::io::copy_bidirectional(&mut tls, &mut back).await?;
    Ok(())
}

/// A till from before encryption: tell it, in a way it understands, that it
/// needs updating.
async fn refuse_plain(mut stream: tokio::net::TcpStream, version: &str) -> std::io::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buf = vec![0u8; 16 * 1024];
    let _ = within(2, stream.read(&mut buf)).await;
    let body = serde_json::to_vec(&Error::RuleAbout("version_mismatch", version.to_owned())).unwrap_or_default();
    let head = format!(
        "HTTP/1.1 426 Upgrade Required\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.shutdown().await
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

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

fn b64(v: Option<&Value>) -> Result<Vec<u8>> {
    v.and_then(Value::as_str).and_then(|s| B64.decode(s).ok()).ok_or_else(|| Error::Invalid("msg".into()))
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
        (Method::Post, "/pair/start") => {
            same_version(app, req.version.as_deref())?;
            let v: Value = serde_json::from_slice(&req.body)?;
            let msg = b64(v.get("msg"))?;
            let (id, out) = state.start_pair(msg).ok_or(Error::Rule("pairing_code"))?;
            Ok(json(200, &serde_json::json!({ "pairId": id, "msg": B64.encode(out) })))
        }
        (Method::Post, "/pair/finish") => {
            same_version(app, req.version.as_deref())?;
            let v: Value = serde_json::from_slice(&req.body)?;
            let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
            let name = v.get("name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Till").to_owned();
            let fingerprint = state.fingerprint.lock().unwrap().clone();
            let key = state.finish_pair(&text("pairId"), &text("confirm"), &fingerprint).ok_or(Error::Rule("pairing_code"))?;
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
            Ok(json(
                200,
                &serde_json::json!({
                    "token": token,
                    "tillId": id,
                    "version": version(app),
                    "shop": shop,
                    "confirm": tls::proof(&key, &[b"main", fingerprint.as_bytes()]),
                }),
            ))
        }
        (Method::Post, "/tls/adopt") => {
            // A till paired before encryption learns this computer's
            // certificate: only the shop's database can make this proof
            let v: Value = serde_json::from_slice(&req.body)?;
            let till = v.get("tillId").and_then(Value::as_i64).ok_or(Error::Rule("till_not_paired"))?;
            let nonce = v.get("nonce").and_then(Value::as_str).filter(|n| (16..=128).contains(&n.len())).ok_or(Error::Invalid("nonce".into()))?;
            let pool = crate::db::pool(app).await?;
            let row: Option<(String,)> = sqlx::query_as("SELECT token_hash FROM tills WHERE id = $1 AND revoked_at IS NULL")
                .bind(till)
                .fetch_optional(&pool)
                .await?;
            let (token_hash,) = row.ok_or(Error::Rule("till_not_paired"))?;
            let fingerprint = state.fingerprint.lock().unwrap().clone();
            Ok(json(200, &serde_json::json!({ "proof": tls::proof(token_hash.as_bytes(), &[b"adopt", nonce.as_bytes(), fingerprint.as_bytes()]) })))
        }
        (Method::Get, p) if p.starts_with("/media/") => {
            // The till fetches with its secret in the header; older screens
            // put it in the query
            let token = req.token.clone().or_else(|| req.url.split_once("?t=").map(|(_, t)| t.to_owned()));
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

    /// What a till does (`remote::pair`), against the pairing state.
    fn till_proof(s: &LanServer, code: &str, fp: &str) -> (String, String) {
        let (till, msg) = tls::spake_till(code);
        let (id, reply) = s.start_pair(msg).expect("a code is showing");
        let key = till.finish(&reply).unwrap();
        (id, tls::proof(&key, &[b"till", fp.as_bytes()]))
    }

    #[test]
    fn pairing_codes_are_single_use_and_give_up_after_wrong_tries() {
        let s = LanServer::default();
        let (code, _) = s.new_code();
        assert_eq!(code.len(), 6);
        let (id, confirm) = till_proof(&s, &code, "fp");
        assert!(s.finish_pair(&id, &confirm, "fp").is_some());
        assert!(s.start_pair(vec![]).is_none(), "used once already");

        let (code, _) = s.new_code();
        for _ in 0..PAIRING_TRIES {
            let (id, confirm) = till_proof(&s, "000000", "fp");
            assert!(s.finish_pair(&id, &confirm, "fp").is_none());
        }
        assert!(s.start_pair(vec![]).is_none(), "too many wrong tries");
        let _ = code;
    }

    #[test]
    fn pairing_fails_through_a_machine_in_the_middle() {
        // Right code, but the till saw another certificate than ours
        let s = LanServer::default();
        let (code, _) = s.new_code();
        let (id, confirm) = till_proof(&s, &code, "someone-else");
        assert!(s.finish_pair(&id, &confirm, "ours").is_none());
    }

    #[test]
    fn the_network_port_speaks_tls_to_the_pinned_certificate_only() {
        tauri::async_runtime::block_on(async {
            let dir = std::env::temp_dir().join(format!("pos-lan-{}", uuid::Uuid::new_v4()));
            let id = tls::load_or_create(&dir).unwrap();
            // The HTTP side, answering every request with "ok"
            let inner = Arc::new(Server::http("127.0.0.1:0").unwrap());
            let inner_addr = inner.server_addr().to_ip().unwrap();
            let i = inner.clone();
            std::thread::spawn(move || {
                for req in i.incoming_requests() {
                    let _ = req.respond(Response::from_string("ok"));
                }
            });
            let front = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            front.set_nonblocking(true).unwrap();
            let port = front.local_addr().unwrap().port();
            let task = tauri::async_runtime::spawn(serve_tls(front, tls::server_config(&id).unwrap(), inner_addr, "9.9.9".into()));
            let url = format!("https://127.0.0.1:{port}/ping");

            let (pinned, pin) = crate::remote::test_client(Some(id.fingerprint.clone()));
            let res = pinned.get(&url).send().await.expect("pinned certificate is accepted");
            assert_eq!(res.text().await.unwrap(), "ok");
            assert!(!pin.mismatched());

            let (other, pin) = crate::remote::test_client(Some("0".repeat(64)));
            assert!(other.get(&url).send().await.is_err(), "any other certificate is refused");
            assert!(pin.mismatched());

            // A till from before encryption hears it must update
            let plain = reqwest::get(format!("http://127.0.0.1:{port}/ping")).await.unwrap();
            assert_eq!(plain.status().as_u16(), 426);
            let body: Value = plain.json().await.unwrap();
            assert_eq!(body["code"], "version_mismatch");
            assert_eq!(body["detail"], "9.9.9");

            task.abort();
            inner.unblock();
            let _ = std::fs::remove_dir_all(dir);
        });
    }

    /// The real pairing and adopt replies (as `route` builds them) behind
    /// real TLS, driven by the till's own code in `remote.rs`.
    #[test]
    fn a_till_pairs_and_an_old_till_adopts_the_certificate() {
        tauri::async_runtime::block_on(async {
            let dir = std::env::temp_dir().join(format!("pos-pair-{}", uuid::Uuid::new_v4()));
            let id = tls::load_or_create(&dir).unwrap();
            let lan = Arc::new(LanServer::default());
            *lan.fingerprint.lock().unwrap() = id.fingerprint.clone();
            let (code, _) = lan.new_code();
            let inner = Arc::new(Server::http("127.0.0.1:0").unwrap());
            let inner_addr = inner.server_addr().to_ip().unwrap();
            let (i, l) = (inner.clone(), lan.clone());
            std::thread::spawn(move || {
                for req in i.incoming_requests() {
                    let (req, parts) = Incoming::read(req);
                    let v: Value = serde_json::from_slice(&parts.body).unwrap_or(Value::Null);
                    let fp = l.fingerprint.lock().unwrap().clone();
                    let reply = match parts.url.as_str() {
                        "/pair/start" => {
                            let (pid, out) = l.start_pair(b64(v.get("msg")).unwrap()).unwrap();
                            json(200, &serde_json::json!({ "pairId": pid, "msg": B64.encode(out) }))
                        }
                        "/pair/finish" => match l.finish_pair(v["pairId"].as_str().unwrap(), v["confirm"].as_str().unwrap(), &fp) {
                            Some(key) => json(200, &serde_json::json!({ "token": "secret", "tillId": 7, "confirm": tls::proof(&key, &[b"main", fp.as_bytes()]) })),
                            None => json(400, &Error::Rule("pairing_code")),
                        },
                        "/tls/adopt" => json(
                            200,
                            &serde_json::json!({ "proof": tls::proof(hash_token("secret").as_bytes(), &[b"adopt", v["nonce"].as_str().unwrap().as_bytes(), fp.as_bytes()]) }),
                        ),
                        _ => json(200, &Value::Null),
                    };
                    let _ = req.respond(reply);
                }
            });
            let front = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            front.set_nonblocking(true).unwrap();
            let server = format!("127.0.0.1:{}", front.local_addr().unwrap().port());
            let task = tauri::async_runtime::spawn(serve_tls(front, tls::server_config(&id).unwrap(), inner_addr, "1".into()));

            let bad = crate::remote::pair(&server, "999999x", "Till 2", "1").await;
            assert!(matches!(bad, Err(Error::Remote { ref code, .. }) if code == "pairing_code"), "{bad:?}");
            let done = crate::remote::pair(&server, &code, "Till 2", "1").await.unwrap();
            assert_eq!((done.token.as_str(), done.till_id), ("secret", 7));
            assert_eq!(done.fingerprint, id.fingerprint);

            // A till paired before encryption: no fingerprint yet
            let link = crate::remote::Link { server: server.clone(), token: "secret".into(), till_id: 7, name: "Till 2".into(), fingerprint: None };
            let client = crate::remote::Client::new(link.clone(), "1".into(), None);
            client.hello().await.unwrap();
            // ...and one that holds the wrong secret can't be fooled into trusting
            let wrong = crate::remote::Client::new(crate::remote::Link { token: "other".into(), ..link }, "1".into(), None);
            assert!(matches!(wrong.hello().await, Err(Error::Rule("main_not_trusted"))));

            task.abort();
            inner.unblock();
            let _ = std::fs::remove_dir_all(dir);
        });
    }

    #[test]
    fn addresses_get_the_default_port() {
        assert_eq!(crate::remote::with_port("192.168.1.20"), "192.168.1.20:7420");
        assert_eq!(crate::remote::with_port(" http://192.168.1.20:8000/ "), "192.168.1.20:8000");
    }
}
