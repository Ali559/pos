# Plan

The committed scope for the next stretch of work. `FEATURES.md` stays as the wider backlog; anything here takes priority over it. Read `ARCHITECTURE.md` first for how the app is put together today.

**Status key:** ✅ done · 🔜 next · ⬜ not started

---

## 0. UI issues (reported 2026-11-04)

| Issue | Status | Cause and fix |
| --- | --- | --- |
| Analytics: *Sales overview* and *Top products* charts overlap each other | ✅ | `Chart.vue` declared `responsive?: boolean`; Vue casts an absent boolean prop to `false`, so Chart.js never resized and the canvas was stretched by CSS (`w-full h-full`). Now defaults to `true`, the CSS sizing is removed, and the chart grid cells have `min-w-0` so they can shrink. |
| Login page: receipt prices render as `$1 . 50` | ✅ | Schibsted Grotesk's tabular-figure set also widens `.` and `:`. The `.num` utility no longer turns on tabular figures. |
| Sidebar (and login panel) don't reach the bottom of the window | ✅ | Layouts relied on `100vh`, which can disagree with the real window height. The app root (`RTLProvider`) is now pinned with `fixed inset-0` and layouts fill it with `h-full`. |

---

## 1. Foundations ✅

Everything later depends on these. Done 2026-11-04; see ARCHITECTURE.md for how it works.

**Left over from Phase 1**
- ⬜ Native-speaker review of `ku` and `ar` strings.
- ⬜ Media cleanup job for files no row references (nothing can orphan a file yet except replacing the shop logo).
- ⬜ HEIC decoding (iPhone photos) — not supported; JPEG/PNG/WebP/GIF/BMP are.
- ⬜ Upload and image display verified only by Rust unit tests and type-checking; try a logo upload in `bun run tauri dev`.

### 1.1 Data layer and migrations
- Move schema into versioned migrations registered with `tauri_plugin_sql::Builder::add_migrations` in `src-tauri/src/lib.rs`. `db.sql` becomes migration `0001`.
- Typed query modules in `src/db/` (one per area: `catalog.ts`, `sales.ts`, `staff.ts`, …). Views never write SQL.
- Anything that needs to be trusted (password hashing, permission checks, file writes, image processing) is a Rust `#[tauri::command]`, not WebView code.
- Delete `src/data/catalog.ts` once the register and inventory read from SQLite.

### 1.2 Languages: Kurdish default, add Arabic
- Default locale becomes `ku` (Sorani, RTL). First launch no longer guesses from the browser language.
- Add `ar` (Arabic, RTL) with a full message set.
- Decouple language from direction: today `RTLProvider` derives language *from* direction. Replace with a `language` setting and a lookup `{ en: "ltr", ku: "rtl", ar: "rtl" }`.
- Language picker lists each language in its own script: English, کوردی, العربية.
- Bundle an Arabic-script face that covers both Sorani and Arabic glyphs well (Vazirmatn or Noto Naskh Arabic) next to Sarchia Makka, self-hosted for offline use.
- Persist the choice in the `settings` table (not just `localStorage`) so it survives a WebView storage reset.
- Translations need review by a native speaker for both `ku` and `ar` before release.

### 1.3 Media pipeline (Rust)
All uploaded images (shop logo, category images, product images, expense receipts) go through one command.

```
upload_media(path | bytes, kind) -> MediaFile
  1. decode            image crate (jpeg, png, webp, heic via libheif if needed)
  2. orient + resize   apply EXIF rotation; cap longest side (e.g. 1600 px)
  3. encode            WebP, lossy, quality 80   (`webp` crate; the `image`
                       crate's WebP encoder is lossless-only)
  4. blurhash          `blurhash` crate on a ~32 px thumbnail, 4×3 components
  5. store             <app_data_dir>/media/<uuid>.webp
  6. insert row        media_files
```

```sql
CREATE TABLE media_files (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  uuid          TEXT UNIQUE NOT NULL,
  path          TEXT NOT NULL,          -- relative to app data dir
  mime          TEXT NOT NULL DEFAULT 'image/webp',
  width         INTEGER NOT NULL,
  height        INTEGER NOT NULL,
  bytes         INTEGER NOT NULL,
  blurhash      TEXT NOT NULL,
  sha256        TEXT NOT NULL,          -- dedupe identical uploads
  original_name TEXT,
  created_by    INTEGER REFERENCES users(id),
  created_at    TEXT DEFAULT CURRENT_TIMESTAMP
);
```

- Front end: a `<MediaImage :media>` component paints the blurhash to a small canvas (`blurhash` npm package) and fades the real image in once loaded. Files are served with Tauri's asset protocol (`convertFileSrc`), which needs the media folder added to the asset scope in `tauri.conf.json`.
- Deleting a product or category doesn't delete media immediately; a cleanup job removes files with no references.

### 1.4 Shop profile
Shop details shown in the top bar and on receipts.

```sql
CREATE TABLE shop_profile (
  id              INTEGER PRIMARY KEY CHECK (id = 1),   -- single row
  name_en         TEXT NOT NULL,
  name_ku         TEXT NOT NULL,
  name_ar         TEXT,
  logo_media_id   INTEGER REFERENCES media_files(id),
  phone           TEXT,
  phone_alt       TEXT,
  address_en      TEXT,
  address_ku      TEXT,
  address_ar      TEXT,
  tax_number      TEXT,
  receipt_footer_en TEXT,
  receipt_footer_ku TEXT,
  receipt_footer_ar TEXT,
  currency        TEXT NOT NULL DEFAULT 'IQD',
  updated_at      TEXT DEFAULT CURRENT_TIMESTAMP
);
```

- New *Shop* tab in Settings with logo upload (through 1.3) and per-language fields.
- Top bar: logo + shop name in the current language, replacing the generic receipt icon in the nav rail. Fallback to `name_en` when a translation is empty.
- First-run setup screen asks for shop name and logo before anything else.

---

## 2. People 🔜

### 2.1 RBAC, fully dynamic
Replace the `users.role` text column and `user_permissions` with real roles and permissions.

```sql
CREATE TABLE permissions (          -- seeded by migrations, not editable in UI
  key         TEXT PRIMARY KEY,     -- e.g. 'sales.refund'
  group_key   TEXT NOT NULL,        -- 'sales', 'inventory', 'staff', …
  is_dangerous INTEGER DEFAULT 0    -- shown with a warning in the editor
);
CREATE TABLE roles (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  name_en TEXT NOT NULL, name_ku TEXT NOT NULL, name_ar TEXT,
  description TEXT,
  is_system   INTEGER DEFAULT 0,    -- Owner: can't be deleted or stripped
  created_at  TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE role_permissions (
  role_id        INTEGER REFERENCES roles(id) ON DELETE CASCADE,
  permission_key TEXT REFERENCES permissions(key),
  PRIMARY KEY (role_id, permission_key)
);
-- users.role_id INTEGER REFERENCES roles(id) replaces users.role
```

- Permission catalog (initial): `sales.sell`, `sales.discount`, `sales.void_line`, `sales.refund`, `sales.reprint`, `inventory.view`, `inventory.edit`, `inventory.adjust_stock`, `catalog.categories`, `staff.view`, `staff.edit`, `staff.shifts`, `roles.manage`, `expenses.view`, `expenses.edit`, `analytics.view`, `analytics.export`, `settings.shop`, `settings.system`.
- Roles CRUD in Settings → Roles: create, rename, duplicate, delete (blocked while users are assigned; offer to reassign), and a permission matrix grouped by area with select-all per group.
- Seed roles: Owner (system, all permissions), Manager, Cashier.
- Enforcement in two places:
  - **Rust:** every command that changes data checks the session user's permissions. This is the real gate.
  - **UI:** `usePermissions().can('sales.refund')`, a `v-can` directive to hide/disable controls, and `meta.permission` on routes with a router guard. Nav rail items hide when not permitted.
- Manager override: an action the cashier can't do opens a PIN prompt; a user with the permission can approve it once. Logged to `audit_log`.
- Guard rails: the last Owner can't be deleted, deactivated, or demoted.

### 2.2 Employees CRUD
- Employees are `users` (one table): full name, username, PIN, phone, role, photo (media), hire date, hourly/monthly wage (optional), active flag.
- Settings → Users becomes a full *Staff* screen: list with search and role filter, create/edit sheet, deactivate instead of delete when the employee has sales history.
- Password and PIN hashing with `argon2` in Rust.

### 2.3 Shifts
```sql
CREATE TABLE shift_schedules (       -- the planned rota
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL REFERENCES users(id),
  starts_at TEXT NOT NULL, ends_at TEXT NOT NULL,
  note TEXT
);
CREATE TABLE shifts (                -- what actually happened
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL REFERENCES users(id),
  schedule_id INTEGER REFERENCES shift_schedules(id),
  cash_drawer_id INTEGER REFERENCES cash_drawers(id),
  clocked_in_at TEXT NOT NULL,
  clocked_out_at TEXT
);
```

- Sign-in starts (or resumes) the employee's shift; if they have a scheduled shift, it's linked.
- Top bar shows the signed-in employee's name and when their shift ends ("Shift ends 18:00", turning amber in the last 15 minutes). No schedule → shows time on shift.
- Closing a shift runs the cash count (`cash_drawers`) and shows over/short.
- Schedule editor: a weekly grid per employee, copy last week.

### 2.4 Employee performance
- Per-employee analytics page with the same filter bar as global analytics (§4.3): sales total, transactions, average basket, items per sale, refunds, discounts given, hours worked (from `shifts`), sales per hour.
- Presets: this week / this month / this year / custom range; compare with previous period.
- Charts mirror the general analytics (sales over time, top products sold by this employee) plus a leaderboard on the Employees tab.
- Export to CSV (uses §4.3).

---

## 3. Catalog ⬜

### 3.1 Categories and subcategories
- `categories.parent_id` already exists; limit depth to two levels (category → subcategory) in the UI.
- Add `name_en`, `name_ku`, `name_ar`, and **required** `icon` (`icon_key TEXT NOT NULL`, from the library in 3.3). Optional `image_media_id` and the existing `color`.
- Categories screen: tree list, drag to reorder (`display_order`), create/edit sheet with icon picker.
- Register filter chips become two rows: category, then its subcategories.

### 3.2 Items: icon and images
- Products get optional `icon_key` and optional images:
  ```sql
  CREATE TABLE product_media (
    product_id INTEGER REFERENCES products(id) ON DELETE CASCADE,
    media_id   INTEGER REFERENCES media_files(id),
    position   INTEGER DEFAULT 0,       -- 0 = cover
    PRIMARY KEY (product_id, media_id)
  );
  ```
- Product names in three languages (`name_en`, `name_ku`, `name_ar`).
- What the register tile shows, in order: cover image (with blurhash placeholder) → icon → category icon → initials.
- Product editor: image gallery with reorder and set-cover, icon picker, category/subcategory select.

### 3.3 Icon library with suggestions
- **Library:** bundle an open-licensed icon set *offline* through Iconify JSON packages. Primary: Tabler Icons (`@iconify-json/tabler`, MIT, ~5,900 icons, good coverage of food, drink, household goods). Optionally add Phosphor (`@iconify-json/ph`, MIT) or Material Symbols (`@iconify-json/material-symbols`, Apache-2.0) for more coverage. Store icons as `"<set>:<name>"` keys, e.g. `tabler:bottle`. Render with `@iconify/vue` in offline mode (icon data loaded from the bundled JSON, never the API).
- **Picker:** searchable grid, filter by set, recent icons, keyboard navigation.
- **Suggestions** on create/update, while the name is typed:
  1. Normalize the name (lowercase, strip sizes/units like "330ml", "1kg").
  2. Translate Kurdish/Arabic words to English keywords with a curated synonym map (`src/data/icon-synonyms.json`, e.g. `شیر → milk`, `نان → bread`, `خبز → bread`). Start with ~500 common shop words.
  3. Score icons against their names and the tags shipped in the Iconify metadata; boost icons used by the parent category.
  4. Show the top 8 as one-click suggestions above the full picker.
- Later, if keyword matching isn't good enough: precomputed text embeddings for icon tags, still offline.

---

## 4. Money ⬜

### 4.1 Notifications
```sql
CREATE TABLE notifications (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL,              -- 'stock_low', 'stock_out', 'expiring', 'expired', 'shift_ending'
  ref_table TEXT, ref_id INTEGER,
  payload TEXT,                    -- JSON for rendering
  created_at TEXT DEFAULT CURRENT_TIMESTAMP,
  read_at TEXT,
  dismissed_at TEXT
);
CREATE TABLE product_batches (     -- needed for expiry
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  product_id INTEGER NOT NULL REFERENCES products(id),
  quantity REAL NOT NULL,
  expiry_date TEXT,
  received_at TEXT DEFAULT CURRENT_TIMESTAMP
);
```

- Triggers: low/out of stock created by a SQL trigger when `stock_quantity` crosses `min_stock_level` or 0 (no duplicate while one is unread). Expiry checked at startup and hourly: "expires in ≤ N days" (setting, default 7) and "expired".
- Bell in the top bar with unread count, a panel grouped by kind, mark read / dismiss, click to open the product.
- Native OS notifications through `tauri-plugin-notification` for stock-out and expired (configurable).
- Who sees what follows RBAC (`inventory.view`).

### 4.2 Expenses
```sql
CREATE TABLE expense_types (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name_en TEXT NOT NULL, name_ku TEXT NOT NULL, name_ar TEXT,
  icon_key TEXT,
  color TEXT,
  is_active INTEGER DEFAULT 1
);
CREATE TABLE expenses (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  type_id INTEGER NOT NULL REFERENCES expense_types(id),
  amount REAL NOT NULL,
  currency TEXT NOT NULL DEFAULT 'IQD',
  spent_at TEXT NOT NULL,
  note TEXT,
  receipt_media_id INTEGER REFERENCES media_files(id),
  recurring TEXT,                  -- null | 'monthly' | 'weekly'
  created_by INTEGER REFERENCES users(id),
  created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
```

- New *Expenses* screen in the nav rail: list with date filter and type filter, create/edit sheet, receipt photo upload.
- Expense types CRUD (rent, electricity, generator, salaries, supplies, …) with icon from the library.
- Expense analytics: totals by type (bar), over time (line), month vs previous month, and **net profit** = gross margin from sales − expenses for the same range.
- Recurring expenses generate a draft on their due date for confirmation.

### 4.3 Analytics overhaul
- **One filter bar shared by every analytics view:** date presets (today, yesterday, this week, last week, this month, last month, this year, custom range) + compare-to-previous-period toggle + filters for employee, category/subcategory, payment method.
- Filters live in the route query so a view can be reopened exactly as it was.
- **Sections:**
  - Overview: revenue, transactions, average basket, gross margin, refunds, discounts, net profit (with expenses), each with change vs previous period.
  - Sales over time with automatic bucket (hour/day/week/month by range length).
  - Busiest hours: weekday × hour heatmap.
  - Products: top/bottom sellers by revenue, quantity, margin; slow movers (no sales in N days).
  - Categories: revenue and margin share.
  - Payments: cash vs card split.
  - Employees: leaderboard (links to 2.4).
  - Expenses: from 4.2.
- **Export:** every table and chart has *Export CSV* that writes exactly the filtered rows. Implemented in Rust (`csv` crate) with a save dialog (`tauri-plugin-dialog`). UTF-8 with BOM so Excel shows Kurdish/Arabic correctly. Permission: `analytics.export`.
- Queries run in Rust or as parameterized SQL in `src/db/analytics.ts`; add indexes as needed (`sales(sale_date, user_id)`, `sale_items(product_id, sale_id)`).

---

## Order of work

```
1.1 data layer ─┬─ 1.2 languages
                ├─ 1.3 media ── 1.4 shop profile ── 3.2 item images
                └─ 2.1 RBAC ── 2.2 employees ── 2.3 shifts ── 2.4 performance
                                                                  │
3.3 icon library ── 3.1 categories ── 3.2 item icons              │
                                                                  ▼
4.1 notifications      4.2 expenses ─────────────────────── 4.3 analytics
```

1. **1.1 → 1.2 → 1.3 → 1.4** — after this, the app runs on real data in Kurdish and Arabic and shows the shop's own brand.
2. **2.1 → 2.2 → 2.3** — sign-in is real and every action is permission-checked.
3. **3.3 → 3.1 → 3.2** — the icon library comes first because categories require an icon.
4. **4.1, 4.2** — independent of each other.
5. **2.4 + 4.3** — analytics last, once sales, shifts, and expenses produce real data.

## Decisions (confirmed 2026-11-04)

- **Icon library:** Tabler Icons via `@iconify-json/tabler`, bundled offline. Suggestions use a hand-made Kurdish/Arabic → English keyword map matched against Tabler's tags.
- **Image encoding:** lossy WebP at quality 80 with the `webp` crate (libwebp); the `image` crate is used only to decode, orient and resize.
- **Expiry:** tracked per batch in a `product_batches` table (§4.1).
- **Languages:** language is the setting; text direction is derived from it (`en` → ltr, `ku`/`ar` → rtl).

## Open questions

- **Currency:** IQD only, or IQD + USD with an exchange rate? Until decided, Phase 1 stores one shop currency in `shop_profile.currency` (default `IQD`) and every price is formatted through `src/lib/format.ts`, so a second currency can be added later without touching views.
- **Product names:** do products need all three languages, or one name plus an optional translation?
- **Shifts:** is a scheduled rota needed, or only clock-in/out?
- **Wages:** should employee performance include labour cost (sales per wage-hour)?
