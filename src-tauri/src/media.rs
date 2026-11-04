//! Image uploads: decode → orient → resize → WebP (q80) + blurhash → store.

use std::io::Cursor;
use std::path::PathBuf;

use image::{imageops::FilterType, DynamicImage, ImageDecoder, ImageReader};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{ipc::InvokeBody, AppHandle, Manager, Runtime};

use crate::auth;
use crate::ctx::Ctx;
use crate::error::{Error, Result};

pub const WEBP_QUALITY: f32 = 80.0;
/// Longest side after resizing. Enough for a full-screen product photo.
pub const MAX_DIMENSION: u32 = 1600;
pub const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;
const BLURHASH_COMPONENTS: (u32, u32) = (4, 3);

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MediaFile {
    pub id: i64,
    pub uuid: String,
    pub path: String,
    pub mime: String,
    pub width: i64,
    pub height: i64,
    pub bytes: i64,
    pub blurhash: String,
}

pub struct Processed {
    pub webp: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub blurhash: String,
}

/// Pure image processing, kept free of Tauri so it can be unit tested.
pub fn process_image(input: &[u8]) -> Result<Processed> {
    let mut decoder = ImageReader::new(Cursor::new(input))
        .with_guessed_format()?
        .into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);

    if img.width() > MAX_DIMENSION || img.height() > MAX_DIMENSION {
        img = img.resize(MAX_DIMENSION, MAX_DIMENSION, FilterType::Lanczos3);
    }

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let webp = webp::Encoder::from_rgba(&rgba, width, height)
        .encode(WEBP_QUALITY)
        .to_vec();

    // A tiny thumbnail is plenty for a 4×3 hash and keeps this fast
    let thumb = img.thumbnail(32, 32).to_rgba8();
    let blurhash = blurhash::encode(
        BLURHASH_COMPONENTS.0,
        BLURHASH_COMPONENTS.1,
        thumb.width(),
        thumb.height(),
        thumb.as_raw(),
    )
    .map_err(|_| Error::Blurhash)?;

    Ok(Processed {
        webp,
        width,
        height,
        blurhash,
    })
}

pub fn media_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf> {
    let dir = app.path().app_data_dir()?.join("media");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Absolute folder that `media_files.path` is relative to.
#[tauri::command]
pub fn get_media_dir(app: AppHandle) -> Result<String> {
    Ok(media_dir(&app)?.to_string_lossy().into_owned())
}

/// Uploads need a signed-in user, except during first-run setup (the shop
/// logo is chosen before the owner account exists).
async fn may_upload(ctx: &Ctx) -> Result<()> {
    if auth::needs_setup(ctx).await? {
        return Ok(());
    }
    auth::current(ctx).await.map(|_| ())
}

/// Processes and stores one image. Shared by the command and the dev bridge.
pub async fn upload(ctx: &Ctx, bytes: Vec<u8>, original_name: Option<String>) -> Result<MediaFile> {
    may_upload(ctx).await?;
    if bytes.is_empty() {
        return Err(Error::Invalid("the file is empty".into()));
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(Error::Rule("file_too_large"));
    }
    let sha256 = format!("{:x}", Sha256::digest(&bytes));

    // Same file uploaded again: reuse the stored copy
    if let Some(existing) = sqlx::query_as::<_, MediaFile>(
        "SELECT id, uuid, path, mime, width, height, bytes, blurhash
         FROM media_files WHERE sha256 = $1 LIMIT 1",
    )
    .bind(&sha256)
    .fetch_optional(&ctx.pool)
    .await?
    {
        return Ok(existing);
    }

    let processed = tauri::async_runtime::spawn_blocking(move || process_image(&bytes))
        .await
        .map_err(|e| Error::Invalid(e.to_string()))??;

    let uuid = uuid::Uuid::new_v4().to_string();
    let path = format!("{uuid}.webp");
    std::fs::write(ctx.media_dir.join(&path), &processed.webp)?;

    let id = sqlx::query(
        "INSERT INTO media_files
           (uuid, path, mime, width, height, bytes, blurhash, sha256, original_name, created_by)
         VALUES ($1, $2, 'image/webp', $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(&uuid)
    .bind(&path)
    .bind(processed.width as i64)
    .bind(processed.height as i64)
    .bind(processed.webp.len() as i64)
    .bind(&processed.blurhash)
    .bind(&sha256)
    .bind(&original_name)
    .bind(ctx.session.get().map(|k| k.user_id))
    .execute(&ctx.pool)
    .await?
    .last_insert_rowid();

    Ok(MediaFile {
        id,
        uuid,
        path,
        mime: "image/webp".into(),
        width: processed.width as i64,
        height: processed.height as i64,
        bytes: processed.webp.len() as i64,
        blurhash: processed.blurhash,
    })
}

/// Takes the raw file bytes as the request body (no JSON/base64 overhead).
/// Optional header `x-file-name` (URI-encoded) is stored as `original_name`.
#[tauri::command]
pub async fn upload_media(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<MediaFile> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(Error::Invalid("expected raw file bytes".into()));
    };
    let original_name = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let ctx = Ctx::from_app(&app).await?;
    upload(&ctx, bytes.clone(), original_name).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = RgbaImage::from_fn(width, height, |x, y| {
            Rgba([(x % 256) as u8, (y % 256) as u8, 120, 255])
        });
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn encodes_lossy_webp_with_blurhash() {
        let p = process_image(&png(640, 480)).unwrap();
        assert_eq!(&p.webp[0..4], b"RIFF");
        assert_eq!(&p.webp[8..12], b"WEBP");
        // Lossy WebP uses the "VP8 " chunk; lossless would be "VP8L"
        assert_eq!(&p.webp[12..16], b"VP8 ");
        assert_eq!((p.width, p.height), (640, 480));
        // 4×3 components → 1 + 1 + 4 + 2 * (4*3 - 1) = 28 characters
        assert_eq!(p.blurhash.len(), 28);
    }

    #[test]
    fn caps_longest_side_and_keeps_aspect() {
        let p = process_image(&png(4000, 2000)).unwrap();
        assert_eq!((p.width, p.height), (MAX_DIMENSION, MAX_DIMENSION / 2));
    }

    #[test]
    fn leaves_small_images_alone() {
        let p = process_image(&png(120, 90)).unwrap();
        assert_eq!((p.width, p.height), (120, 90));
    }

    #[test]
    fn rejects_non_images() {
        assert!(process_image(b"definitely not an image").is_err());
    }
}
