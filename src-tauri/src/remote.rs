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

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};

use crate::error::{Error, Result};

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
}

pub struct Client {
    pub link: Link,
    http: reqwest::Client,
    version: String,
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
    TillLink(link.map(|link| Arc::new(Client::new(link, app.package_info().version.to_string()))))
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
    let a = address.trim().trim_start_matches("http://").trim_end_matches('/');
    if a.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) {
        a.to_owned()
    } else {
        format!("{a}:{}", crate::server::DEFAULT_PORT)
    }
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap_or_default()
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
    pub fn new(link: Link, version: String) -> Self {
        Self { link, http: http(), version }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.link.server)
    }

    fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.http
            .post(self.url(path))
            .bearer_auth(&self.link.token)
            .header("x-pos-version", &self.version)
    }

    pub async fn invoke(&self, name: &str, args: Value) -> Result<Value> {
        let res = self.post(&format!("/invoke/{name}")).json(&serde_json::json!({ "args": args })).send().await.map_err(unreachable)?;
        reply(res).await
    }

    pub async fn upload(&self, bytes: Vec<u8>, file_name: Option<String>) -> Result<Value> {
        let mut req = self.post("/upload_media").body(bytes);
        if let Some(n) = file_name {
            req = req.header("x-file-name", n);
        }
        reply(req.send().await.map_err(unreachable)?).await
    }

    /// A fresh start: the main computer forgets who was signed in here.
    pub async fn hello(&self) -> Result<Value> {
        reply(self.post("/hello").send().await.map_err(unreachable)?).await
    }

    /// Where this till loads product photos and the logo from.
    pub fn media_base(&self) -> String {
        format!("{}/media/", self.url(""))
    }

    /// Who is signed in, as the main computer sees it.
    pub async fn whoami(&self) -> Result<crate::auth::Actor> {
        let v = self.invoke("whoami", Value::Null).await?;
        Ok(serde_json::from_value(v)?)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairReply {
    pub token: String,
    pub till_id: i64,
}

/// Asks the main computer at `server` to pair with the code it shows.
pub async fn pair(server: &str, code: &str, name: &str, version: &str) -> Result<PairReply> {
    let res = http()
        .post(format!("http://{server}/pair"))
        .header("x-pos-version", version)
        .json(&serde_json::json!({ "code": code.trim(), "name": name.trim() }))
        .send()
        .await
        .map_err(unreachable)?;
    Ok(serde_json::from_value(reply(res).await?)?)
}

/// What a main computer says about itself (no secret needed).
pub async fn ping(server: &str) -> Result<Value> {
    let res = http().get(format!("http://{server}/ping")).send().await.map_err(unreachable)?;
    reply(res).await
}
