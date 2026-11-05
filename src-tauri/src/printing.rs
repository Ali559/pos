//! Receipts straight to a thermal printer, without the print dialog.
//!
//! The WebView draws the receipt (it already shapes Kurdish and Arabic
//! text) and sends it as a 1-bit raster; this wraps it in ESC/POS and sends
//! it to the printer named by the `receipt_printer` setting:
//! `system:<name>` for an installed printer (raw job via CUPS or the
//! Windows spooler), or `tcp:<host>:<port>` for a network printer (usually
//! port 9100). Empty means the system print dialog, handled by the WebView.

use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::settings;

const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
/// Rows per GS v 0 block; some printers choke on taller images
const BAND_ROWS: usize = 256;
const TCP_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterInfo {
    /// What to store in the setting after "system:"
    pub system_name: String,
    pub name: String,
}

/// Installed printers, for the settings screen.
pub async fn list_printers(ctx: &Ctx) -> Result<Vec<PrinterInfo>> {
    auth::require(ctx, "settings.system").await?;
    Ok(printers::get_printers()
        .into_iter()
        .map(|p| PrinterInfo { system_name: p.system_name, name: p.name })
        .collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterArgs {
    /// Bytes per row (width in dots / 8)
    pub width_bytes: usize,
    pub height: usize,
    /// Base64 rows, top to bottom, most significant bit leftmost, 1 = black
    pub data: String,
    /// Also pulse the cash drawer (connected to the printer)
    #[serde(default)]
    pub open_drawer: bool,
    /// Print on this target instead of the setting (the settings test page)
    pub target: Option<String>,
}

/// The ESC/POS job: reset, the image in bands, feed, cut.
pub fn escpos_job(width_bytes: usize, height: usize, bits: &[u8], open_drawer: bool) -> Result<Vec<u8>> {
    if width_bytes == 0 || width_bytes > 255 || bits.len() != width_bytes * height {
        return Err(Error::Invalid("raster size doesn't match its data".into()));
    }
    let mut out = vec![ESC, b'@'];
    if open_drawer {
        // Pulse pin 2: on 50 ms, off 500 ms
        out.extend_from_slice(&[ESC, b'p', 0, 25, 250]);
    }
    for band in bits.chunks(width_bytes * BAND_ROWS) {
        let rows = band.len() / width_bytes;
        out.extend_from_slice(&[GS, b'v', b'0', 0]);
        out.extend_from_slice(&[(width_bytes & 0xff) as u8, (width_bytes >> 8) as u8]);
        out.extend_from_slice(&[(rows & 0xff) as u8, (rows >> 8) as u8]);
        out.extend_from_slice(band);
    }
    // Feed past the cutter, then a partial cut
    out.extend_from_slice(&[ESC, b'd', 4, GS, b'V', 66, 0]);
    Ok(out)
}

fn send(target: &str, job: &[u8]) -> Result<()> {
    let fail = |e: String| Error::RuleAbout("printer_unreachable", e);
    if let Some(addr) = target.strip_prefix("tcp:") {
        let addr = addr
            .to_socket_addrs()
            .map_err(|e| fail(format!("{addr}: {e}")))?
            .next()
            .ok_or_else(|| fail(addr.to_owned()))?;
        let mut stream = TcpStream::connect_timeout(&addr, TCP_TIMEOUT).map_err(|e| fail(format!("{addr}: {e}")))?;
        stream.set_write_timeout(Some(TCP_TIMEOUT)).ok();
        stream.write_all(job).map_err(|e| fail(e.to_string()))?;
        return Ok(());
    }
    if let Some(name) = target.strip_prefix("system:") {
        let printer = printers::get_printer_by_name(name).ok_or_else(|| fail(name.to_owned()))?;
        let options = printers::common::base::job::PrinterJobOptions {
            name: Some("Receipt"),
            raw_properties: &[("raw", "true")],
            converter: printers::common::converters::Converter::None,
        };
        printer.print(job, options).map_err(|e| fail(format!("{e:?}")))?;
        return Ok(());
    }
    Err(Error::Invalid(format!("unknown printer target {target}")))
}

pub async fn print_raster(ctx: &Ctx, args: RasterArgs) -> Result<()> {
    auth::current(ctx).await?;
    let target = match args.target {
        Some(t) => {
            auth::require(ctx, "settings.system").await?;
            t
        }
        None => settings::get(ctx, "receipt_printer", "").await?,
    };
    if target.is_empty() {
        return Err(Error::Rule("no_receipt_printer"));
    }
    let bits = base64::engine::general_purpose::STANDARD
        .decode(args.data.as_bytes())
        .map_err(|e| Error::Invalid(e.to_string()))?;
    let job = escpos_job(args.width_bytes, args.height, &bits, args.open_drawer)?;
    // Printing blocks (network, spooler); keep it off the async runtime
    tauri::async_runtime::spawn_blocking(move || send(&target, &job))
        .await
        .map_err(|e| Error::Invalid(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_is_banded_and_ends_with_a_cut() {
        let bits = vec![0xffu8; 2 * 300];
        let job = escpos_job(2, 300, &bits, true).unwrap();
        assert_eq!(&job[..2], &[ESC, b'@']);
        assert_eq!(&job[2..7], &[ESC, b'p', 0, 25, 250]);
        // First band: 256 rows of 2 bytes
        assert_eq!(&job[7..15], &[GS, b'v', b'0', 0, 2, 0, 0, 1]);
        let second = 15 + 2 * 256;
        assert_eq!(&job[second..second + 8], &[GS, b'v', b'0', 0, 2, 0, 44, 0]);
        assert_eq!(&job[job.len() - 4..], &[GS, b'V', 66, 0]);
        assert!(escpos_job(2, 3, &[0; 5], false).is_err());
    }
}
