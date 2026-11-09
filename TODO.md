# TODO

Everything still open, from `PLAN.md`, `PLAN2.md` and `FEATURES.md` (updated 2026-11-09).

## Auto-update

Built (2026-11-07): `tauri-plugin-updater` with the public key and the GitHub URL in `src-tauri/tauri.conf.json`, a bar in the app when an update is ready, Settings → About → *Check for updates*, and the release workflow `.github/workflows/release.yml`. How to release: [RELEASING.md](RELEASING.md).

- [x] **A signing key.** Generated; the public key is in `tauri.conf.json`.
- [x] **Where releases are published.** GitHub releases on `Ali559/pos` (public).
- [x] **Add the two secrets on GitHub** (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).
- [x] **First release**: [v0.2.0](https://github.com/Ali559/pos/releases/tag/v0.2.0) built on Windows and macOS (2026-11-07).
- [ ] **Second release** (e.g. `v0.2.1`) to see an installed v0.2.0 find the update and install it.
- [ ] Apple and Windows code signing, so the first install doesn't show a security warning.

## Check on real hardware

- [ ] Direct printing on the shop's real thermal printer (so far only a simulated network printer, with the ESC/POS output decoded back to an image).
- [ ] Scan a real scale label (the labels and the weight dialog only have unit tests).
- [ ] Customer display as a real second window on a second monitor.
- [ ] Fullscreen and the close guard in the desktop app (only checked in the browser preview).

## Check phase 14 for real

Built 2026-11-09 (see `PLAN2.md` §14). Checked with tests and in the browser preview against a test shop; these need the desktop app or real machines:

- [ ] **Two tills on a real network**: pair a second computer with the main one, sell on both, unplug the main one and see the till say so. So far the main computer's till server was driven over HTTP (pairing, sign-in, a sale tagged with the till, refusals, removing a till); the till side's native window wasn't run.
- [ ] **Excel export** from the desktop app (the file is written by a unit-tested function; the save dialog wasn't opened).
- [ ] **PDF export and the shift / day slips** through the real print dialog and on the receipt printer (only their contents were checked).
- [ ] **A return of a sale paid partly with points**, at the till (covered by a Rust test only).

## Languages and platform

- [ ] Native-speaker review of the `ku` and `ar` strings (phase 14 added about 250 new ones in each).
- [ ] HEIC photos on Windows/Linux (needs libheif or the Windows HEIF codec).
- [ ] Touch-screen pass: at least 44 px targets everywhere (tables and settings don't meet it yet).

## Done in phase 14 (2026-11-09)

- [x] **Activity log** (Settings → Activity): every logged action as a sentence in the shop's language, filtered by date, person, kind and text. New permission `settings.audit` (Managers have it).
- [x] **Card payments switch** (Settings → Payments), off for new shops; a shop that has already taken cards keeps them on. When on, a sale can be split between cash and card.
- [x] **Shift report** with discounts, promotions, returns and points, printed from the end-of-shift dialog (optionally as the shift ends).
- [x] **End-of-day (Z) report** (Analytics → *End of day*): by payment method and by cashier, with each drawer's over/short; printed on the receipt printer or through the print dialog.
- [x] **Analytics export** to Excel (.xlsx, right-to-left in Kurdish and Arabic) and PDF (an A4 page through the print dialog), as well as CSV.
- [x] **Loyalty points**, on by default (Settings → Payments): earned on what's paid in cash or by card, spent at the till, given back on returns, adjusted by hand with a reason.
- [x] **Several tills** (Settings → Tills): one main computer keeps the data and serves extra tills on the shop's network, with the recommended specs shown before turning it on.

## Still not built

- [ ] **Working offline on an extra till**: an extra till can't sell while the main computer is off or unreachable.
- [ ] **Encrypted till traffic**: tills talk to the main computer over plain HTTP on the local network (each till has its own secret). Fine on the shop's own network; TLS would be needed before anything crosses the internet.
- [ ] **Syncing to a hosted database** (the other option in `FEATURES.md`): not started.
- [ ] IQD + USD with an exchange rate (IQD only for now, decided 2026-11-07).

## Open questions

- [ ] Scale barcodes: which scale does the shop use, and does it print weight or price in the barcode?
