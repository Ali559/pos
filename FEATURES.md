# Features roadmap

The wider backlog. Committed, prioritized work lives in `PLAN.md` / `PLAN2.md` and takes precedence over this list; open work is tracked in `TODO.md`.

Ideas worth adding, roughly in the order they'd pay off for a small Kurdish shop running this on one or two tills.

**Status key:** ✅ done · 🟡 partly done · ⬜ not started (checked against the code 2026-11-09)
Legend: **S** small (a day or less) · **M** a few days · **L** a week or more

---

## 1. Make it real (foundations)

| Status | Feature | Size | Notes |
| --- | --- | --- | --- |
| ✅ | Data layer over SQLite | M | Migrations and typed commands in Rust (`PLAN.md` §1.1). |
| ✅ | Real sign-in | M | `argon2` hashes in Rust; session lives in Rust memory. |
| ✅ | PIN quick-switch | S | Password or PIN on the login screen. |
| ✅ | Checkout that writes a sale | M | One transaction: `sales` + `sale_items` + `payments`. |
| ✅ | Product create / edit | M | With multiple barcodes, packs, icons and photos. |
| ✅ | Settings persisted | S | Shop profile and settings in the database. |

## 2. At the register

| Status | Feature | Size | Notes |
| --- | --- | --- | --- |
| ✅ | Hardware barcode scanner focus | S | Typing anywhere with nothing focused goes to the scan field. |
| ✅ | Keyboard shortcuts | S | F1–F12 and Esc, with key hints on wide screens (Phase 13). |
| ✅ | Cash tendered & change due | S | Banknote buttons, change due, both stored on the payment. |
| ⬜ | Dual currency (IQD + USD) | M | Parked: IQD only for now (decided 2026-11-07). `src/lib/format.ts` is the single place money is formatted. |
| ✅ | Split payments | S | Cash plus card on one sale, when card payments are on (Settings → Payments; off by default). |
| ✅ | Hold / recall sales | S | F4 hold, F5 held sales. |
| ✅ | Line discounts & cart discount | S | Permission-gated, manager PIN otherwise. |
| ✅ | Weighed items | M | Weight dialog and price/weight EAN-13 scale labels (Phase 8). |
| ✅ | Quick keys | S | Pinned products in a ★ *Quick* chip (Phase 13). |
| ✅ | Receipt printing | M | 58/80 mm, rendered as an image; print dialog, installed printer or network printer. |
| ✅ | Cash drawer kick | S | ESC/POS drawer pulse in the print job (`printing.rs`). |
| ✅ | Returns & refunds | M | By receipt number, per line, cash, card or onto a customer's account. |

## 3. Stock

| Status | Feature | Size | Notes |
| --- | --- | --- | --- |
| ✅ | Low-stock alerts | S | Notifications reconciled in Rust. |
| ✅ | Stock adjustments with reasons | S | Receive / remove stock with batches. |
| ✅ | Receiving / purchase orders | M | Receive delivery from a supplier, reorder list (Phase 10). |
| ✅ | Stock-take mode | M | Inventory → Stock count (Phase 10). |
| ✅ | Expiry dates | M | Per-batch expiry, soonest-expiring sold first, `block_expired` setting. |
| ✅ | Import / export CSV | S | CSV, Excel and ODS import; CSV export (Phase 12). |
| ✅ | Barcode label printing | M | A4 sheets or receipt-printer labels (Phase 12). |

## 4. Customers

| Status | Feature | Size | Notes |
| --- | --- | --- | --- |
| ✅ | Attach customer to sale | S | By phone or name at the till (Phase 9). |
| ✅ | Store credit / tab (قەرز) | M | `customer_ledger`, credit limits, repayments, statements (Phase 9). |
| ✅ | Loyalty points | S | On by default; earn on money paid, spend at the till, given back on returns (`loyalty.rs`). |

## 5. Shift & money control

| Status | Feature | Size | Notes |
| --- | --- | --- | --- |
| ✅ | Open / close shift | M | Opening float and closing count with over/short. |
| ✅ | End-of-day (Z) report | S | Analytics → *End of day*: by payment method and by cashier with drawer over/short; printable. The shift report prints too. |
| ✅ | Roles & permissions enforced | M | Fully dynamic RBAC (`PLAN.md` §2.1). |
| ✅ | Manager override | S | PIN prompt on the spot, logged to `audit_log`. |
| ✅ | Audit log viewer | S | Settings → Activity, in the shop's language (`src/lib/activity.ts`). |

## 6. Reporting

| Status | Feature | Size | Notes |
| --- | --- | --- | --- |
| ✅ | Real analytics queries | M | |
| ✅ | Date range picker | S | Presets, custom range, compare with previous period. |
| ✅ | Profit & margin view | S | Gross margin and net profit after expenses. |
| ✅ | Busiest hours heatmap | S | Weekday × hour. |
| ✅ | Export to PDF / Excel | S | CSV, .xlsx (Rust, `rust_xlsxwriter`) and PDF (A4 page through the print dialog). |

## 7. Platform & reliability

| Status | Feature | Size | Notes |
| --- | --- | --- | --- |
| ✅ | Automatic backups | S | Daily, with rotation and restore (Phase 7). |
| ✅ | Auto-update | S | `tauri-plugin-updater` and release workflow. The update path still needs a second release to test (see `TODO.md`). |
| ✅ | Fullscreen / kiosk mode | S | F11, a setting per computer, close guard; window 1280×800, min width 1024. |
| 🟡 | Multi-till sync | L | Done on the LAN: one main computer serves paired tills (`server.rs`, `remote.rs`). Not done: selling offline on an extra till, or syncing to a hosted database. |
| ✅ | Arabic locale | S | `ar` added; native-speaker review still open (see `TODO.md`). |
| ✅ | Locale-aware numbers & dates | S | Decided: Latin digits in every language, matching barcodes and labels (`src/lib/format.ts`). |
| ⬜ | Touch-screen pass | S | Minimum 44 px targets everywhere (the register mostly meets this; tables and settings don't). |
| ✅ | Remove unused deps | S | `flowbite` and `@tanstack/vue-table` removed 2026-11-09. |
| ✅ | Tests | M | `bun test` for money paths: allocation, discounts, partial-return refunds. |
