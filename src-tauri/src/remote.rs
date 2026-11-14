//! This computer as an extra till: it has no shop data of its own. Every
//! command goes to the main computer on the shop's network (`server.rs`),
//! which keeps the database, the sign-in and the permission checks. Only
//! what belongs to this machine runs here: printing to its own printer,
//! its own settings (printer, paper, full screen), files saved on it, and
//! the customer display.
//!
//! The link is `till.json` in the app's config folder: the main computer's
//! address and the secret it gave this till when they were paired. Pairing
//! or leaving restarts the app, so the mode never changes while it runs.
//!
//! The link is TLS, and the till accepts only the main computer's own
//! certificate, whose fingerprint is in `till.json` (see `tls.rs`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use crate::error::{Error, Result};
use crate::tls::{self, Pin};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

/// Commands that run on this computer even when it's an extra till.
pub const LOCAL: &[&str] = &["list_printers", "print_raster"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    /// host:port of the main computer
    pub server: String,
    pub token: String,
    pub till_id: i64,
    pub name: String,
    /// SHA-256 of the main computer's certificate. Missing on a till paired
    /// before encryption: it is learned (and proved) on first use.
    #[serde(default)]
    pub fingerprint: Option<String>,
}

pub struct Client {
    pub link: Link,
    version: String,
    /// Where `till.json` is, to save a newly learned fingerprint
    path: Option<PathBuf>,
    pin: Mutex<Option<String>>,
    conn: tokio::sync::Mutex<Option<(reqwest::Client, Arc<Pin>)>>,
}

/// Set once at startup: Some on an extra till.
pub struct TillLink(pub Option<Arc<Client>>);

pub fn client<R: Runtime>(app: &AppHandle<R>) -> Option<Arc<Client>> {
    app.try_state::<TillLink>().and_then(|s| s.0.clone())
}

fn link_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf> {
    Ok(app.path().app_config_dir()?.join("till.json"))
}

pub fn load<R: Runtime>(app: &AppHandle<R>) -> TillLink {
    let link = link_path(app)
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Link>(&b).ok());
    let path = link_path(app).ok();
    TillLink(link.map(|link| Arc::new(Client::new(link, app.package_info().version.to_string(), path))))
}

pub fn save<R: Runtime>(app: &AppHandle<R>, link: Option<&Link>) -> Result<()> {
    let path = link_path(app)?;
    match link {
        Some(l) => {
            std::fs::create_dir_all(path.parent().unwrap_or(&path))?;
            std::fs::write(path, serde_json::to_vec_pretty(l)?)?;
        }
        None => {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

/// "192.168.1.20" → "192.168.1.20:7420"
pub fn with_port(address: &str) -> String {
    let a = address.trim().trim_start_matches("http://").trim_start_matches("https://").trim_end_matches('/');
    if a.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) {
        a.to_owned()
    } else {
        format!("{a}:{}", crate::server::DEFAULT_PORT)
    }
}

/// A client that trusts only the certificate `pin` (any, when None: then
/// the returned Pin says which one it saw).
fn http(pin: Option<String>) -> Result<(reqwest::Client, Arc<Pin>)> {
    let (config, pin) = tls::client_config(pin)?;
    let client = reqwest::Client::builder()
        .use_preconfigured_tls(config)
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| Error::Invalid(format!("http: {e}")))?;
    Ok((client, pin))
}

#[cfg(test)]
pub fn test_client(pin: Option<String>) -> (reqwest::Client, Arc<Pin>) {
    http(pin).unwrap()
}

fn nonce() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// Turns a reply into the command's result: errors from the main computer
/// are passed on as they are, so the screen translates them as usual.
async fn reply(res: reqwest::Response) -> Result<Value> {
    let ok = res.status().is_success();
    let body: Value = res.json().await.map_err(|_| Error::Rule("main_unreachable"))?;
    if ok {
        return Ok(body);
    }
    let field = |k: &str| body.get(k).and_then(Value::as_str).map(str::to_owned);
    Err(Error::Remote {
        code: field("code").unwrap_or_else(|| "internal".into()),
        message: field("message").unwrap_or_default(),
        detail: field("detail"),
    })
}

fn unreachable(e: reqwest::Error) -> Error {
    eprintln!("[till] {e}");
    Error::Rule("main_unreachable")
}

impl Client {
    pub fn new(link: Link, version: String, path: Option<PathBuf>) -> Self {
        let pin = Mutex::new(link.fingerprint.clone());
        Self { link, version, path, pin, conn: tokio::sync::Mutex::new(None) }
    }

    fn url(&self, path: &str) -> String {
        format!("https://{}{path}", self.link.server)
    }

    /// The connection to the main computer, pinned to its certificate.
    async fn connection(&self) -> Result<(reqwest::Client, Arc<Pin>)> {
        let mut conn = self.conn.lock().await;
        if let Some(c) = conn.as_ref() {
            return Ok(c.clone());
        }
        let known = self.pin.lock().unwrap().clone();
        let fingerprint = match known {
            Some(f) => f,
            None => self.adopt().await?,
        };
        let c = http(Some(fingerprint))?;
        *conn = Some(c.clone());
        Ok(c)
    }

    /// Learns the main computer's certificate: it proves it holds this
    /// till's secret (its SHA-256, in the shop's database) and the
    /// certificate this till sees. Then the fingerprint is saved.
    async fn adopt(&self) -> Result<String> {
        let (client, pin) = http(None)?;
        let nonce = nonce();
        let res = client
            .post(self.url("/tls/adopt"))
            .json(&serde_json::json!({ "tillId": self.link.till_id, "nonce": nonce }))
            .send()
            .await
            .map_err(unreachable)?;
        let body = reply(res).await?;
        let seen = pin.seen().ok_or(Error::Rule("main_unreachable"))?;
        let expected = tls::proof(crate::server::hash_token(&self.link.token).as_bytes(), &[b"adopt", nonce.as_bytes(), seen.as_bytes()]);
        if !body.get("proof").and_then(Value::as_str).is_some_and(|p| tls::same(p, &expected)) {
            eprintln!("[till] the main computer at {} couldn't prove who it is", self.link.server);
            return Err(Error::Rule("main_not_trusted"));
        }
        *self.pin.lock().unwrap() = Some(seen.clone());
        if let Some(path) = &self.path {
            let mut link = self.link.clone();
            link.fingerprint = Some(seen.clone());
            std::fs::write(path, serde_json::to_vec_pretty(&link)?)?;
        }
        Ok(seen)
    }

    /// Sends a request. If the main computer shows a new certificate (e.g. it
    /// was restored onto another computer), it has to prove itself again.
    async fn send(&self, build: impl Fn(&reqwest::Client) -> reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let (client, pin) = self.connection().await?;
        match build(&client).send().await {
            Ok(r) => Ok(r),
            Err(_) if pin.mismatched() => {
                *self.conn.lock().await = None;
                *self.pin.lock().unwrap() = None;
                let (client, _) = self.connection().await?;
                build(&client).send().await.map_err(unreachable)
            }
            Err(e) => Err(unreachable(e)),
        }
    }

    fn post(&self, client: &reqwest::Client, path: &str) -> reqwest::RequestBuilder {
        client
            .post(self.url(path))
            .bearer_auth(&self.link.token)
            .header("x-pos-version", &self.version)
    }

    pub async fn invoke(&self, name: &str, args: Value) -> Result<Value> {
        let body = serde_json::json!({ "args": args });
        let path = format!("/invoke/{name}");
        reply(self.send(|c| self.post(c, &path).json(&body)).await?).await
    }

    pub async fn upload(&self, bytes: Vec<u8>, file_name: Option<String>) -> Result<Value> {
        let res = self
            .send(|c| {
                let req = self.post(c, "/upload_media").body(bytes.clone());
                match &file_name {
                    Some(n) => req.header("x-file-name", n),
                    None => req,
                }
            })
            .await?;
        reply(res).await
    }

    /// A fresh start: the main computer forgets who was signed in here.
    pub async fn hello(&self) -> Result<Value> {
        reply(self.send(|c| self.post(c, "/hello")).await?).await
    }

    /// A stored image, for the `tillmedia:` protocol the screen loads from.
    pub async fn media(&self, file: &str) -> Result<Vec<u8>> {
        let path = format!("/media/{}", file);
        let res = self.send(|c| c.get(self.url(&path)).bearer_auth(&self.link.token)).await?;
        if !res.status().is_success() {
            return Err(Error::NotFound);
        }
        Ok(res.bytes().await.map_err(unreachable)?.to_vec())
    }

    /// Who is signed in, as the main computer sees it.
    pub async fn whoami(&self) -> Result<crate::auth::Actor> {
        let v = self.invoke("whoami", Value::Null).await?;
        Ok(serde_json::from_value(v)?)
    }
}

/// Where the screen loads images from on a till: a protocol of this app
/// that fetches them from the main computer (the WebView itself wouldn't
/// trust the main computer's certificate, and the secret stays out of URLs).
pub const MEDIA_SCHEME: &str = "tillmedia";

pub fn media_base() -> String {
    if cfg!(any(windows, target_os = "android")) {
        format!("http://{MEDIA_SCHEME}.localhost/")
    } else {
        format!("{MEDIA_SCHEME}://localhost/")
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairReply {
    pub token: String,
    pub till_id: i64,
    confirm: String,
    /// The certificate the main computer showed, now proved to be its own
    #[serde(skip)]
    pub fingerprint: String,
}

/// Asks the main computer at `server` to pair with the code it shows. The
/// code never crosses the network: both ends prove they know it with
/// SPAKE2, bound to the certificate the till sees (see `tls.rs`).
pub async fn pair(server: &str, code: &str, name: &str, version: &str) -> Result<PairReply> {
    let (client, pin) = http(None)?;
    let (spake, msg) = tls::spake_till(code.trim());
    let res = client
        .post(format!("https://{server}/pair/start"))
        .header("x-pos-version", version)
        .json(&serde_json::json!({ "msg": B64.encode(msg) }))
        .send()
        .await
        .map_err(unreachable)?;
    let start = reply(res).await?;
    let seen = pin.seen().ok_or(Error::Rule("main_unreachable"))?;
    let theirs = start.get("msg").and_then(Value::as_str).and_then(|m| B64.decode(m).ok()).ok_or(Error::Rule("pairing_code"))?;
    let key = spake.finish(&theirs).map_err(|_| Error::Rule("pairing_code"))?;
    let res = client
        .post(format!("https://{server}/pair/finish"))
        .header("x-pos-version", version)
        .json(&serde_json::json!({
            "pairId": start.get("pairId"),
            "confirm": tls::proof(&key, &[b"till", seen.as_bytes()]),
            "name": name.trim(),
        }))
        .send()
        .await
        .map_err(unreachable)?;
    let mut done: PairReply = serde_json::from_value(reply(res).await?)?;
    // The main computer proves it knew the code too, for this certificate
    if pin.seen().as_deref() != Some(seen.as_str()) || !tls::same(&done.confirm, &tls::proof(&key, &[b"main", seen.as_bytes()])) {
        return Err(Error::Rule("main_not_trusted"));
    }
    done.fingerprint = seen;
    Ok(done)
}

/// What a main computer says about itself (no secret needed, nothing
/// trusted: it only fills in the pairing form).
pub async fn ping(server: &str) -> Result<Value> {
    let (client, _) = http(None)?;
    let res = client.get(format!("https://{server}/ping")).send().await.map_err(unreachable)?;
    reply(res).await
}
