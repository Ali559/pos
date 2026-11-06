# Plan, part 2

What comes after `PLAN.md` §1–6 (all done). Phase numbers carry on from there, so **Phase 7 is the first phase of this document**. `FEATURES.md` stays the wider backlog; anything here takes priority over it.

Written 2026-11-06 from a comparison of the app against what grocery / mini-market POS systems usually offer, plus what shops in Kurdistan tend to need (customer debt, IQD + USD).

**Status key:** ✅ done · 🔜 in progress · ⬜ not started
**Size:** S a day or less · M a few days · L a week or more

---

## Order of work

```
7 backups ──► 8 selling units ──► 9 customers & debt ──► 10 suppliers & stock count
                                                              │
              13 till comfort (any time) ◄── 12 catalog upkeep ◄── 11 pricing & promotions
```

1. **7** first: today, losing the disk loses the business.
2. **8 → 9**: the two things a Kurdish mini-market asks for in its first week (cartons/weighed goods, and قەرز).
3. **10 → 11 → 12**: back office.
4. **13** is a set of independent small items; pick them up between phases.

---

## 7. Backups ✅

Built 2026-11-07 (`src-tauri/src/backup.rs`, Settings → Backups, migration `0009`). The automatic backup has run in the real app. Restoring has unit tests (unpack, check, swap at startup) but hasn't been clicked through in the app yet.

| Item | Size | Status |
| --- | --- | --- |
| Automatic daily backup; old ones deleted once today's is done | S | ✅ |
| Back up now | S | ✅ |
| Backup folder chosen by the owner (USB / second disk) | S | ✅ |
| List of backups and restore | M | ✅ |
| Alert when backups stop working | S | ✅ |

**How it works**
- A backup is one `.zip` file, `pos-backup-YYYYMMDD-HHMMSS.zip`, holding `pos.db` (a consistent snapshot taken with SQLite `VACUUM INTO`, safe while the till is in use) and `media/` (logo, product, category, employee and receipt photos), plus a `backup.json` manifest (app version, schema version, shop name, created at, row counts).
- **Where:** a folder the owner picks with a folder dialog opened by Rust (the WebView never names paths). A chosen folder must already exist when a backup runs: if the USB stick is out, the backup fails and raises an alert instead of quietly writing to the computer's own disk. Until one is picked, backups go to `<app data>/backups`, and Settings says plainly that this won't survive a broken disk.
- **When:** the hourly background job runs a backup when the last good one is more than 24 hours old (so also right at startup after a day off), and *Back up now* runs one on demand. Settings `backup_auto` (default on).
- **Old backups are deleted once today's is done.** After a new backup is written *and* reads back cleanly (the zip opens, the database passes its check), the older ones are deleted, so the folder normally holds just the latest. If the new backup fails or doesn't check out, the old ones stay. `backup_keep` (default 1, up to 365) lets a shop keep more days if it wants a history. Only files matching the backup name pattern are ever deleted. Backups taken before a restore are named `pos-before-restore-…` and are never deleted this way.
- **Restore:** pick a backup from the list (or any `.zip` through a file dialog). Rust checks it (has `pos.db` and a manifest, `PRAGMA integrity_check` passes, schema not newer than this app) and shows when it was taken, the shop name and how many sales it holds. On confirm: a safety backup of the current data is taken, the backup is staged, and the app restarts. At the next start, before the database is opened, the staged files replace `pos.db` and `media/`; the normal migrations then bring an older backup up to date.
- **Alerts:** a `backup_failed` notification when the last attempt failed (e.g. the USB stick was unplugged), and `backup_overdue` when there has been no good backup for 3 days. Both clear themselves after the next good backup.
- **Permission:** `settings.system` for everything here. Every backup, restore and folder change is in the audit log.

**Later**
- Encrypting backups with a password.
- A copy to a cloud drive when online.

---

## 8. Selling units ✅

Built 2026-11-07 (migration `0010`, `src-tauri/src/scale.rs`, checkout in `sales.rs`). Packs, the misc item and price check were checked in the app; scale labels and the weight dialog have unit tests, but nobody has scanned a real scale label yet.

| Item | Size | Status |
| --- | --- | --- |
| Packs and cartons | M | ✅ |
| Decimal quantities | S | ✅ |
| Scale barcodes | M | ✅ |
| Open-price item | S | ✅ |
| Price check | S | ✅ |

**How it works**
- **Packs.** `product_units` (name, how many, barcode, price for the whole pack), edited under *Pack sizes* in the product form. Scanning a pack's barcode adds the pack; a cart line can be switched between single and a pack. Stock is still counted in the product's own unit, so a carton of 24 takes 24 off and singles and cartons share the same stock. `sale_items` keeps the pack's name and size as a snapshot (`unit_name`, `unit_factor`); `quantity` stays in the product's unit, so returns, batches and reports work unchanged. Receipts and sales history show "1 × Carton". Receiving stock can be entered in packs (quantity and cost per pack).
- **Weighed items.** Products sold by kg or liter ask for the amount (weight dialog) when tapped, and the amount can be changed from the cart line; lowering it counts as a void.
- **Scale labels.** EAN-13 codes that start with the shop's prefix (default `20`), then the product's scale code (PLU, set in the product form for weighed products), then the weight or the price, then a valid check digit. Layout in Settings → Scale labels. Checkout decodes the label again in Rust, so a price label charges exactly what is printed and the screen can't change it. One lookup (`resolve_scan`) handles product barcodes, pack barcodes and scale labels.
- **Misc item.** Created automatically after setup (`sku` `MISC`, in the shop's language): the price is typed at the till with an optional note ("Misc item: batteries" on the receipt). Any product can be set to *price typed at the till* and/or *don't count stock*. Typing a price needs the new `sales.open_price` permission (given to Manager and Cashier; others need a manager's PIN). Products that don't count stock never block a sale, raise stock alerts or get restocked by returns.
- **Price check.** A button by the scan bar opens a box that shows price, pack prices, stock, and what a scale label would charge, without touching the cart.

## 9. Customers and debt ✅

Built 2026-11-07 (migration `0011`, `src-tauri/src/customers.rs`, Customers screen). **IQD only** for now (decided 2026-11-07); a second currency is left for later.

| Item | Size | Status |
| --- | --- | --- |
| Customers screen | S | ✅ |
| Attach a customer to a sale | S | ✅ |
| Debt (قەرز) | M | ✅ |
| Loyalty points | S | ⬜ later, if shops ask for it |
| IQD + USD | M | ⬜ later |

**How it works**
- **Balance = the customer's ledger.** `customer_ledger` rows are only ever added: `sale` (left on account), `payment` (paid back), `refund` (a return onto the account), `adjustment` (an opening balance from the paper notebook, or a correction, always with a reason). Positive means they owe the shop.
- **At the till.** *Add customer* under the receipt title finds a customer by phone or name, or adds one on the spot. With a customer on the sale, a third way to pay appears: **On account**. It asks how much is paid in cash now (if anything) and puts the rest on their account. The sale's payments still add up to its total: the part on account is a `credit` payment, so reports, returns and receipts keep working. The receipt shows the customer and what they owe now.
- **Credit limit.** Optional per customer. Going over it needs a manager's PIN (`customers.over_limit`).
- **Repayments.** *Take payment* (Customers screen or the customer's account) records cash or card. It's a payment with no sale, linked to the ledger row, so cash repayments count in the drawer at end of shift (shown on their own line), but not as sales in analytics. Can't pay more than is owed.
- **Returns.** A sale with a customer can be refunded *onto the account* (less debt) as well as in cash or by card.
- **Account sheet.** Balance, limit, every entry with the running balance, *Take payment*, *Adjust balance*, *Print statement* (through the print dialog, on the receipt paper width).
- **Reports.** End of shift shows debt paid back in cash and what was sold on account. Analytics → payments shows "On account" as a method, plus debt paid back in the range and what customers owe right now.
- **Permissions.** `customers.view`, `customers.credit` (sell on account, take repayments, add a customer at the till) for Manager and Cashier; `customers.edit` (edit, set limits, adjust balances) and `customers.over_limit` for Manager. A cashier's new customer gets no credit limit.

## 10. Suppliers and stock ✅

Built 2026-11-07 (migration `0012`, `src-tauri/src/suppliers.rs` and `stockcount.rs`, Suppliers screen, Inventory → Stock count). The screens were checked in the app; receiving, paying and applying a count are covered by tests but haven't been done on real data yet.

| Item | Size | Status |
| --- | --- | --- |
| Suppliers screen | S | ✅ |
| Receiving against a supplier | M | ✅ |
| Reorder suggestions | S | ✅ |
| Stock count | M | ✅ |
| Cost method | S | ✅ last cost |

**How it works**
- **Suppliers** (new item in the side rail): name, contact person, phone, notes, how many products they supply, last delivery, and what the shop owes them. Their account shows every delivery and payment with a running balance; *Pay* records a payment from **the till** (comes out of the drawer, shown at end of shift), **other cash** or **bank**. *Adjust* is for old unpaid invoices from before the app.
- **Receive delivery**: pick the supplier (or none, for stock bought at the market, which must be paid in full), invoice number and an optional photo of the invoice, then scan or search the products. Each line has quantity (singles or packs), cost and expiry. Saving adds stock as batches (so expiry alerts work), sets each product's cost to this delivery's cost (**last cost**), links the product to the supplier (the first one becomes its main supplier), and puts whatever wasn't paid on the supplier's account. Deliveries are listed with their lines.
- **Reorder**: everything at or under its reorder level, grouped by its main supplier, with a suggested quantity that covers two weeks of sales (at least twice the reorder level), rounded up to whole packs, and an estimated cost. *Copy order* puts a ready message on the clipboard for WhatsApp/Viber; *Export* writes a CSV.
- **Stock count** (Inventory tab): name it, optionally limit it to a category, then scan (each scan adds one; pack barcodes add the pack) or type counts. Filters for not counted / differences / all. It can be left and continued later. *Apply* moves stock by counted − what the system said when it was counted, so sales during the count aren't counted twice; losses come out of batches, finds go in as a batch, all logged as corrections.
- **Money stays right**: payments to suppliers from the till are negative cash payments, so expected cash at end of shift goes down and they're shown on their own line; they're kept out of the sales payment split in analytics.
- **Permissions**: `suppliers.view`, `suppliers.edit` (suppliers, receiving from them), `suppliers.pay` (dangerous: paying and adjusting) for Manager; `inventory.count` for Manager and Cashier. Applying a count needs `inventory.adjust_stock`.

## 11. Pricing and promotions ✅

Built 2026-11-07 (migration `0013`, `src-tauri/src/pricing.rs`, `src/lib/pricing.ts`). A promotion and the wholesale switch were checked at the till in the app; nothing was sold with them yet.

| Item | Size | Status |
| --- | --- | --- |
| Price levels | M | ✅ retail + wholesale |
| Quantity prices | S | ✅ |
| Promotions | M | ✅ |
| Tax-free items | S | ✅ per product |
| Bulk price edit | S | ✅ |
| Price history | S | ✅ |

**How it works**
- **Wholesale.** Products can have a wholesale price (blank = same as retail). The register has a *Retail / Wholesale* switch; a customer can be set to wholesale so their sales start there. Selling at wholesale needs `sales.wholesale` or a manager's PIN, and the sale remembers its level. Packs keep their own price at both levels.
- **Quantity prices.** "From 6, 700 each": retail only, the cheapest tier the line's quantity reaches, never above the normal price. Repeated lines of the same product are added up first, so two scans of 3 count as 6.
- **Promotions** (Inventory → Promotions): *percent off*, *buy X get Y free* or *N for a price*, on products and/or whole categories (a parent category covers its subcategories), with optional start/end dates. Checkout gives each line the best promotion running today; no discount permission is needed. They apply to retail sales of single items, not packs, scale labels or typed prices. The receipt and sales history show the promotion's name and amount; `sale_items.promo_discount` keeps it apart from discounts someone gave. A used promotion is switched off instead of deleted.
- **Tax-free products**: tax is worked out only on the taxable lines; returns use each line's own rate.
- **Bulk price change** (Inventory → *Change prices*): raises or lowers every product the current filter shows by a percentage or an amount, rounded to 50/100/250/500/1000 or not, with a preview of every new price first. Permission `catalog.bulk_prices` (dangerous).
- **Price history**: every change to price, wholesale price or cost (from editing, a bulk change or a delivery) is recorded with who and when, and listed in the product form.
- **One set of rules in two places.** Rust (checkout) is what's charged; `src/lib/pricing.ts` mirrors it for the register's preview, with the same test cases on both sides (`tests/pricing.test.ts`, `sales::tests::wholesale_quantity_prices_promotions_and_tax_free`).

## 12. Catalog upkeep ✅

Built 2026-11-07 (`src-tauri/src/import.rs`, `src/lib/labels.ts`, `src/lib/ean13.ts`; Inventory → *More*). Labels were checked on screen (thermal and A4 layouts); importing has unit tests but the file dialog hasn't been clicked through in the desktop app yet, and nothing has been printed on paper.

| Item | Size | Status |
| --- | --- | --- |
| Import from a spreadsheet | M | ✅ |
| Export the catalog | S | ✅ |
| Shelf and price labels | M | ✅ |

**How it works**
- **Import** (Inventory → More → *Import from a spreadsheet*): CSV (comma, semicolon or tab, with or without a BOM), Excel (.xlsx/.xls) or LibreOffice (.ods), first sheet, first row as headers. Rust opens the file dialog and reads it; columns are matched to fields by their names in English, Kurdish or Arabic (the export's own headers always match), and every match can be changed. *Check* shows what would be new, updated, skipped or refused (with the row number and why) without writing anything; *Import* does it in one transaction.
  - Products are matched by barcode (any of the product's barcodes), then SKU.
  - Numbers may use Eastern Arabic digits (٠١٢ / ۰۱۲), thousands separators and currency words; units may be written in any of the three languages.
  - New products need a name and a price; their stock comes in as a batch. Existing products get their name, prices, category, unit and reorder level from the file (blank cells are left alone); stock is never changed by an import (use a stock count). Price changes go into the price history.
  - Missing categories are created (with a tag icon) for people who may manage categories; for others those rows are refused.
- **Export** (More → *Export the catalog*): the products the list shows as CSV with the import's column names, so a file can be exported, edited in Excel and imported back. Uses the `analytics.export` permission.
- **Labels** (More → *Print labels*): every product the list shows, or every product whose price changed since a date (for re-labelling shelves after a price change), any number of copies. Name, price (per kg for weighed products) and a barcode: EAN-13 when the product has a valid 12/13-digit code, else Code 128 of its barcode or SKU. Printed as an A4 sheet (3 across) through the print dialog, or one label at a time on the receipt printer (each cut off), drawn at 203 dpi like receipts so Kurdish and Arabic are shaped correctly.

## 13. Till comfort and platform ✅ (except auto-update)

Built 2026-11-07 (migration `0014`, `src/lib/customerDisplay.ts`, `src/components/display/`). Shortcuts, quick keys and the customer display were checked in the browser preview; the display as a real second window on a second monitor, fullscreen and the close guard need the desktop app.

| Item | Size | Status |
| --- | --- | --- |
| Keyboard shortcuts | S | ✅ |
| Quick keys | S | ✅ |
| Customer display | M | ✅ |
| Fullscreen / kiosk mode | S | ✅ |
| Auto-update | S | ⬜ needs a signing key and a place to host releases |
| Tests for money paths | M | ✅ (and found a refund rounding bug) |

**How it works**
- **Keyboard** (register): F2 scan field, F3 price check, F4 hold, F5 held sales, F6 customer, F7 discount on the sale, F8 pay cash, F9 pay card, F10 on account, F12 charge, Esc twice to clear. F1 shows the list. Typing anywhere with nothing focused goes to the scan field, so a scanner never types into the void. Keys are ignored while a dialog is open; hints are shown on the buttons on wide screens.
- **Quick keys**: *Quick key* in the product form pins a product to the front of the register; a ★ *Quick* chip comes first and is selected at start when anything is pinned. People who edit the catalog can move quick keys earlier/later right on the tiles.
- **Customer display**: the screen icon by the receipt title opens a second window (full screen on another monitor when there is one) showing the shop's logo and name between customers, the lines and the total while ringing up, and *thank you* with the change (and anything put on account) after the sale. It only listens to the till (`display:state` events); it never reads the database. In the browser dev preview it's a second tab over a BroadcastChannel.
- **Fullscreen**: Settings → Register → *Full screen* opens the app full screen on that computer; F11 switches any time. Closing the window with a sale on the till asks first.
- **Money-path tests**: `allocate` always adds up exactly; returning a discounted line one piece at a time refunds exactly what was paid. That second test caught a real bug: each partial return rounded its own tax, so three returns could refund 1 IQD more than the sale took. Refunds now use each line's share of the tax the sale actually charged, and the last piece gets exactly what's left (the return dialog's preview mirrors it).

**Auto-update — needs your decision**
- Tauri's updater checks a JSON file at a URL you choose (e.g. a GitHub release of `Ali559/pos`), downloads the new version and checks its signature against a public key built into the app.
- It needs: a signing key pair (the private key stays with you, never in the repo), the place releases are published, and building/publishing a release for each version.

## Carried over from PLAN.md

- ⬜ Native-speaker review of the `ku` and `ar` strings.
- ⬜ HEIC on Windows/Linux.
- ⬜ Direct printing on the shop's real thermal printer.

## Open questions

- ~~**Currency:**~~ IQD only for now (decided 2026-11-07).
- **Scale barcodes:** which scale does the shop use, and does it print weight or price in the barcode?
- **Loyalty:** wanted, or is debt enough?
- **Auto-update:** where will releases be hosted?
