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

- ~~**Currency:**~~ IQD only for now (decided 2026-11-07).
- **Scale barcodes:** which scale does the shop use, and does it print weight or price in the barcode?
- **Loyalty:** wanted, or is debt enough?
- **Auto-update:** where will releases be hosted?
