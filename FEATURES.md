# Features roadmap

The wider backlog. Committed, prioritized work lives in `PLAN.md` and takes precedence over this list.

Ideas worth adding, roughly in the order they'd pay off for a small Kurdish shop running this on one or two tills. Each item notes what already exists in `db.sql` so you can see how much is schema-ready.

Legend: **S** small (a day or less) · **M** a few days · **L** a week or more

---

## 1. Make it real (foundations)

These turn the UI shell into a working till. Nothing below them matters until these are done.

| Feature | Size | Notes |
| --- | --- | --- |
| Data layer over SQLite | M | `src/db/*.ts` typed query modules, migrations via `tauri_plugin_sql` (see ARCHITECTURE.md → Intended wiring). Replace `src/data/catalog.ts` and every hard-coded `ref([...])`. |
| Real sign-in | M | Hash passwords in Rust (`argon2`), never in the WebView. Add a route guard so `/main/*` needs a session. `users.password_hash` and `last_login` already exist. |
| PIN quick-switch | S | Cashiers swap with a 4–6 digit PIN instead of full login. `users.pin` exists. Big on-screen keypad, works with a touch screen. |
| Checkout that writes a sale | M | One transaction: insert `sales` + `sale_items` + `payments`. The `after_sale_item_insert` trigger already decrements stock and logs the movement. |
| Product create / edit | M | Sheet or dialog from Inventory's "Add product" and row pencil. Multiple barcodes per product via `product_barcodes`. |
| Settings persisted | S | Store name, tax rate, currency, theme in the `settings` table instead of local refs. Tax rate on the register currently hard-coded to 10%. |

## 2. At the register

| Feature | Size | Notes |
| --- | --- | --- |
| Hardware barcode scanner focus | S | Keep the scan field focused after every action; treat a burst of keystrokes ending in Enter as a scan even if focus drifted. |
| Keyboard shortcuts | S | `F2` focus scan, `F4` hold, `F8` cash, `F9` card, `F12` charge, `Esc` clear (with confirm). Show them as small key hints on the buttons. |
| Cash tendered & change due | S | After choosing Cash, show a keypad with quick-amount buttons (exact, next 5, next 10, common banknotes) and the change to hand back in large type. |
| Dual currency (IQD + USD) | M | Very common in Kurdistan. Store prices in one base currency, show both on the receipt, accept payment in either at a configurable exchange rate. Replace the hard-coded `USD` in `src/lib/format.ts`. |
| Split payments | S | Part cash, part card. `payments` already supports multiple rows per sale. |
| Hold / recall sales | S | The "Hold sale" button exists; wire it to `held_sales` and add a "Held (3)" chip to recall. |
| Line discounts & cart discount | S | Percent or fixed, permission-gated. `sale_items.discount_amount` and `sales.discount_amount` exist. |
| Weighed items | M | Produce sold by kg: price-embedded EAN-13 barcodes (prefix 2x) or a scale integration. `products.unit` exists. |
| Quick keys | S | Let the manager pin the 12–20 fastest-moving items to the product grid instead of showing everything. |
| Receipt printing | M | 58/80 mm ESC/POS thermal printers via a Rust command. Kurdish needs the receipt rendered as an image (most printers have no Arabic-script font). |
| Cash drawer kick | S | Send the ESC/POS drawer pulse on cash sales. |
| Returns & refunds | M | Look up a sale by receipt number, pick lines to return, restock. `returns` / `return_items` exist. |

## 3. Stock

| Feature | Size | Notes |
| --- | --- | --- |
| Low-stock alerts | S | Badge on the Inventory rail item and the Analytics card, driven by `min_stock_level`. The UI already colors low/out stock. |
| Stock adjustments with reasons | S | Damaged, expired, count correction. Writes `stock_adjustments`. |
| Receiving / purchase orders | M | Receive a delivery from a supplier, update cost price and stock in one step. `suppliers`, `product_suppliers` exist. |
| Stock-take mode | M | Scan everything on a shelf, compare to system counts, apply differences in one batch. |
| Expiry dates | M | Per-batch expiry for dairy and bakery, with a "expiring this week" list. Needs a `product_batches` table. |
| Import / export CSV | S | Bulk-load a catalog from a supplier spreadsheet. |
| Barcode label printing | M | Print shelf/price labels for items without a manufacturer barcode. |

## 4. Customers

| Feature | Size | Notes |
| --- | --- | --- |
| Attach customer to sale | S | Search by phone at the register. `customers`, `sales.customer_id`, and the purchases trigger exist. |
| Store credit / tab (قەرز) | M | Many neighborhood shops let regulars pay later. Track balance per customer, partial repayments, and a "who owes what" list. Needs a `customer_ledger` table. |
| Loyalty points | S | `customers.loyalty_points` exists; define earn/redeem rules in settings. |

## 5. Shift & money control

| Feature | Size | Notes |
| --- | --- | --- |
| Open / close shift | M | Count the float at open, count the drawer at close, show over/short. `cash_drawers` has every column needed. |
| End-of-day (Z) report | S | Totals by payment method, refunds, discounts, per cashier. Printable. |
| Roles & permissions enforced | M | Settings already lists roles; enforce them (refunds, discounts, price edits, voiding lines). `user_permissions` exists. |
| Manager override | S | A cashier action that needs approval prompts for a manager PIN on the spot. |
| Audit log viewer | S | `audit_log` exists; add a filterable list under Settings. |

## 6. Reporting

| Feature | Size | Notes |
| --- | --- | --- |
| Real analytics queries | M | Replace mock numbers: sales by hour/day, top products, margin (from `sale_items.cost_price`). |
| Date range picker | S | The range select exists; make it drive the queries and add custom ranges. |
| Profit & margin view | S | Revenue minus snapshot cost price per item, by category. |
| Busiest hours heatmap | S | Day-of-week × hour grid; helps with staffing. |
| Export to PDF / Excel | S | For the accountant. |

## 7. Platform & reliability

| Feature | Size | Notes |
| --- | --- | --- |
| Automatic backups | S | Copy `pos.db` daily to a chosen folder / USB with rotation. Losing the DB loses the business. |
| Auto-update | S | `tauri-plugin-updater`. |
| Fullscreen / kiosk mode | S | Tauri window `fullscreen`, hide decorations, prevent accidental close during a sale. Also raise the default 800×600 window size. |
| Multi-till sync | L | Several registers sharing one catalog. Start with one machine as the server on the LAN, or sync to a hosted Postgres when online. |
| Arabic locale | S | The RTL plumbing is generic; adding `ar` is mostly translation. |
| Locale-aware numbers & dates | S | Header clock and money are formatted with `en-US` even in Kurdish. Decide whether to show Eastern Arabic numerals (٠١٢٣) in `ku`. |
| Touch-screen pass | S | Minimum 44 px targets everywhere (the register mostly meets this; tables and settings don't). |
| Remove unused deps | S | `flowbite` and `@tanstack/vue-table` aren't used. |
| Tests | M | Vitest for cart math, tax, rounding, and currency conversion. These are where money bugs hide. |
