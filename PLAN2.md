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

## 9. Customers and debt ⬜

`customers`, `sales.customer_id` and `customers.loyalty_points` exist in the schema with no screens.

| Item | Size | Notes |
| --- | --- | --- |
| Customers screen | S | List, search by name or phone, create/edit, purchase history. |
| Attach a customer to a sale | S | Search by phone at the register; shown on the receipt. |
| Debt (قەرز) | M | Pay later or part-pay a sale as debt. A `customer_ledger` (sale, repayment, adjustment). Repayments from the register or the customer's page, a "who owes what" list, per-customer limit, printable statement, debt in end-of-shift and analytics. Permission `customers.credit`. |
| Loyalty points | S | Earn/redeem rules in settings. Optional; ask shops whether they want it. |
| IQD + USD | M | Show prices in both, take payment in either at a rate the shop sets (with history of rate changes), give change in IQD. Receipts show both totals. See open questions. |

---

## 10. Suppliers and stock ⬜

`suppliers` and `product_suppliers` exist with no screens.

| Item | Size | Notes |
| --- | --- | --- |
| Suppliers screen | S | Name, phone, what they supply, balance owed to them. |
| Receiving against a supplier | M | One delivery = many lines (product, quantity or cartons, cost, expiry). Updates cost price and stock in one step; optional invoice photo. |
| Reorder suggestions | S | Items under `min_stock_level`, grouped by supplier, with a suggested quantity from recent sales. Export/print as an order to send. |
| Stock count | M | Scan a shelf or category, compare with the system, apply the differences in one batch with a reason. Can be paused and resumed. |
| Cost method | S | Decide between last cost and average cost for margins once receiving exists. |

---

## 11. Pricing and promotions ⬜

| Item | Size | Notes |
| --- | --- | --- |
| Price levels | M | Retail and wholesale (and more if needed); a customer can be set to a level; the cashier can switch level for a sale with permission. |
| Quantity prices | S | Cheaper per item from N items (e.g. 3+). |
| Promotions | M | Buy X get Y, N for a fixed price, percent off a product or category; start and end dates; shown on the receipt and reported like discounts. |
| Tax-free items | S | Tax is shop-wide today; a per-product (or per-category) tax class. |
| Bulk price edit | S | Raise/lower prices by % or amount for a category or a selection, with a preview. |
| Price history | S | Every price and cost change recorded, shown on the product. |

---

## 12. Catalog upkeep ⬜

| Item | Size | Notes |
| --- | --- | --- |
| Import from a spreadsheet | M | CSV/Excel with column mapping, a preview of new vs updated items, and a dry run. |
| Export the catalog | S | Products, stock and prices to CSV (UTF-8 with BOM, like analytics). |
| Shelf and price labels | M | Print labels with name, price and a barcode (Code 128 / EAN-13) for selected items, or every item whose price changed since a date. Same raster path as receipts, or A4 sheets. |

---

## 13. Till comfort and platform ⬜

| Item | Size | Notes |
| --- | --- | --- |
| Keyboard shortcuts | S | F2 scan, F4 hold, F8 cash, F9 card, F12 charge, Esc clear (with confirm); small key hints on the buttons. |
| Quick keys | S | The manager pins the 12–20 fastest sellers to the top of the register grid. |
| Customer display | M | A second window on a second monitor showing the cart, total and change. |
| Fullscreen / kiosk mode | S | No window decorations, no accidental close during a sale. |
| Auto-update | S | `tauri-plugin-updater`; needs a signing key and somewhere to host releases. |
| Tests for money paths | M | Cart totals, discounts spread over lines, returns, exchanges, rounding, currency. |

---

## Carried over from PLAN.md

- ⬜ Native-speaker review of the `ku` and `ar` strings.
- ⬜ HEIC on Windows/Linux.
- ⬜ Direct printing on the shop's real thermal printer.

## Open questions

- **Currency:** IQD only, or IQD + USD (§9)? Who sets the rate, and how often?
- **Scale barcodes:** which scale does the shop use, and does it print weight or price in the barcode?
- **Loyalty:** wanted, or is debt enough?
- **Auto-update:** where will releases be hosted?
