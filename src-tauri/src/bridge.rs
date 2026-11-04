//! Dev-only HTTP bridge so the UI can run in a normal browser against the
//! real Rust commands and the real database.
//!
//! `bun run tauri dev` starts it on 127.0.0.1:1430 (debug builds only); Vite
//! proxies `/__bridge/*` to it, so a browser tab on http://localhost:1420
//! behaves like the app window. It accepts only requests carrying
//! `x-pos-bridge: 1` from that origin, which a third-party page in the same
//! browser can't send without a CORS preflight this server never answers.


use serde_json::Value;
use tauri::AppHandle;
use tiny_http::{Header, Method, Request, Response, Server};

use crate::commands;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::media;

pub const ADDR: &str = "127.0.0.1:1430";
const ALLOWED_ORIGIN: &str = "http://localhost:1420";

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let server = match Server::http(ADDR) {
            Ok(s) => s,
            Err(e) => return eprintln!("[bridge] could not listen on {ADDR}: {e}"),
        };
        println!("[bridge] dev bridge on http://{ADDR}");
        for mut req in server.incoming_requests() {
            let incoming = Incoming::read(&mut req);
            let app = app.clone();
            let response = tauri::async_runtime::block_on(async move { handle(&app, incoming).await });
            let _ = req.respond(response);
        }
    });
}

/// The parts of a request the handlers need, read up front so the async
/// handler owns its data.
struct Incoming {
    method: Method,
    url: String,
    trusted: bool,
    file_name: Option<String>,
    body: Vec<u8>,
}

impl Incoming {
    fn read(req: &mut Request) -> Self {
        let header = |name: &str| {
            req.headers()
                .iter()
                .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
                .map(|h| h.value.as_str().to_owned())
        };
        let trusted = header("x-pos-bridge").as_deref() == Some("1")
            && header("origin").is_none_or(|o| o == ALLOWED_ORIGIN);
        let file_name = header("x-file-name");
        let mut body = Vec::new();
        let _ = req.as_reader().read_to_end(&mut body);
        Self {
            method: req.method().clone(),
            url: req.url().to_owned(),
            trusted,
            file_name,
            body,
        }
    }
}

type Reply = Response<std::io::Cursor<Vec<u8>>>;

fn json(status: u16, body: &impl serde::Serialize) -> Reply {
    Response::from_data(serde_json::to_vec(body).unwrap_or_default())
        .with_status_code(status)
        .with_header(Header::from_bytes("content-type", "application/json").unwrap())
}

async fn handle(app: &AppHandle, req: Incoming) -> Reply {
    // <img> tags can't send the header; stored images are fine to serve
    let is_image = req.method == Method::Get && req.url.starts_with("/media/");
    if !req.trusted && !is_image {
        return json(403, &serde_json::json!({ "code": "forbidden", "message": "not allowed" }));
    }
    match route(app, req).await {
        Ok(r) => r,
        Err(e) => json(400, &e),
    }
}

async fn route(app: &AppHandle, req: Incoming) -> Result<Reply> {
    let ctx = Ctx::from_app(app).await?;
    match (&req.method, req.url.as_str()) {
        (Method::Post, "/upload_media") => {
            Ok(json(200, &media::upload(&ctx, req.body, req.file_name).await?))
        }
        (Method::Post, path) if path.starts_with("/invoke/") => {
            let name = &path["/invoke/".len()..];
            let v: Value = if req.body.is_empty() { Value::Null } else { serde_json::from_slice(&req.body)? };
            let args = v.get("args").cloned().unwrap_or(Value::Null);
            Ok(json(200, &commands::dispatch(&ctx, name, args).await?))
        }
        (Method::Get, path) if path.starts_with("/media/") => {
            let file = &path["/media/".len()..];
            if file.is_empty() || file.contains('/') || file.contains("..") {
                return Err(Error::NotFound);
            }
            let bytes = std::fs::read(ctx.media_dir.join(file)).map_err(|_| Error::NotFound)?;
            Ok(Response::from_data(bytes)
                .with_header(Header::from_bytes("content-type", "image/webp").unwrap()))
        }
        _ => Err(Error::NotFound),
    }
}
