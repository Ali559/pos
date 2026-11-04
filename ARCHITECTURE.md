# Architecture

A desktop point-of-sale app for small shops. It runs offline on the till machine as a native window (Tauri), with a Vue 3 front end and a local SQLite database. The interface comes in Kurdish Sorani (default, right-to-left), Arabic (right-to-left) and English (left-to-right); each shop picks one at setup and keeps it.

> Status: phases 1–5 of `PLAN.md` are in: SQLite data layer, one language per shop, Rust media pipeline, shop profile, sign-in (password or PIN), dynamic roles enforced in Rust, employees, shifts and schedules, the category tree and product editor with an offline icon library and barcode lookup, checkout with discounts, manager approval, cash change, returns, receipts and drawer counts, stock batches with expiry, alerts, expenses, and analytics with CSV export. `FEATURES.md` is the wider backlog.

## Stack

| Layer | Choice | Notes |
| --- | --- | --- |
| Desktop shell | Tauri 2 (Rust) | `src-tauri/`. All writes, auth and permission checks, media pipeline. |
| UI framework | Vue 3.5, `<script setup>` + TypeScript | Single-page app mounted in `index.html`. |
| Build | Vite 6 + `bun` | Dev server on fixed port `1420` (Tauri requires it). |
| Styling | Tailwind CSS v4 (`@tailwindcss/vite`) | Design tokens are CSS variables in `src/index.css`, mapped into Tailwind with `@theme inline`. |
| Fonts | Schibsted Grotesk, Vazirmatn (`@fontsource-variable`), Rudaw (bundled, see `src/assets/fonts/FONTS.md`) | Self-hosted so the till renders correctly offline. |
| Components | shadcn-vue (new-york style) on reka-ui | Copied into `src/components/ui/`; edit them freely, they are owned code. |
| UI icons | lucide-vue-next | Interface chrome (buttons, nav). |
| Catalog icons | Tabler via `@iconify-json/tabler` + `@iconify/vue/offline` | Categories and products; drawn offline, never fetched. |
| Charts | Chart.js 4 | Wrapped by `src/components/data/Chart.vue`. |
| Routing | vue-router 5, memory history | No URLs exposed; navigation is purely in-app. |
| i18n | vue-i18n 11 (composition mode) | One file per language in `src/utils/i18n/locales/`. |
| Database | SQLite via sqlx, owned by `tauri-plugin-sql` | Plugin used from Rust only (migrations + pool); the WebView has no plugin access. |
| Auth | `argon2` hashes for passwords and PINs | Session lives in Rust memory. |
| Images | `image` (decode/orient/resize), `webp` (libwebp, lossy q80), `blurhash` | Rust only. |

`flowbite` and `@tanstack/vue-table` are installed but unused; candidates for removal.

## Directory layout

```
pos/
├── index.html
├── db.sql                        Legacy schema dump; migrations are the source of truth
├── src/
│   ├── main.ts                   Applies theme, installs router + i18n, mounts
│   ├── App.vue                   Root: LocaleProvider → <router-view>
│   ├── index.css                 Tailwind, design tokens, fonts per language
│   ├── router/index.ts           Routes, meta.permission, first-run + sign-in guards
│   ├── layouts/MainLayout.vue    Nav rail + top bar + scrolling content
│   ├── views/
│   │   ├── SetupView.vue         First run: shop details + owner account
│   │   ├── LoginView.vue         Password or PIN (on-screen keypad)
│   │   ├── SalesView.vue         The register
│   │   ├── InventoryView.vue
│   │   ├── AnalyticsView.vue     Filter bar (in the route), Overview / Products / Employees / Profit
│   │   ├── ExpensesView.vue      Expenses and expense types
│   │   ├── StaffView.vue         Employees, Schedule, Roles tabs
│   │   └── SettingsView.vue      General (theme, tax, sample data), Shop
│   ├── components/
│   │   ├── menus/                Sidebar (nav rail by permission, user menu), Header (shop, user, shift), NotificationBell
│   │   ├── EditSheet.vue         Side panel used by every create/edit form
│   │   ├── catalog/              CategoriesTab, ProductSheet (barcode-first editor), QuickAddTab (scan-and-next), ProductThumb, StockSheet
│   │   ├── sales/                CashDialog, DiscountDialog, ManagerPinDialog, ReturnDialog, EndShiftDialog, PrintRoot (receipt)
│   │   ├── pickers/              DateInput, TimeInput (replace native date/time inputs)
│   │   ├── expenses/             ExpensesTab, ExpenseTypesTab
│   │   ├── reports/RangePicker.vue  Date presets + custom range, shared by Expenses and Analytics
│   │   ├── icons/                AppIcon (renders "tabler:…" keys), IconPicker (suggest + browse)
│   │   ├── staff/                EmployeesTab, RolesTab, ScheduleTab
│   │   ├── shop/                 ShopForm, LogoField
│   │   ├── media/                MediaImage (blurhash → WebP), MediaGallery (ordered photos, cover first)
│   │   ├── providers/LocaleProvider.vue
│   │   └── ui/                   shadcn-vue primitives
│   ├── db/                       Read queries only (SQL lives here, never in views)
│   │   ├── client.ts             select()/selectOne() → Rust `select` command
│   │   ├── catalog.ts, sales.ts (history, sale detail), settings.ts, shop.ts, staff.ts, stock.ts, expenses.ts, notifications.ts
│   │   └── analytics.ts          Report queries over sale lines (local-time buckets)
│   ├── stores/
│   │   ├── auth.ts               Session mirror, can(), login/logout, MAIN_ROUTES, landingPath()
│   │   └── shop.ts               Shop profile, currency, tax rate
│   ├── hooks/                    useTimeManager, useTheme, useShiftStatus
│   ├── lib/
│   │   ├── ipc.ts                call(command, args): Tauri invoke, or the dev bridge in a browser
│   │   ├── errors.ts             errorMessage(): translates Rust error codes
│   │   ├── format.ts, locale.ts  Money, dates, times; Kurdish fallbacks
│   │   ├── media.ts              uploadMedia(), mediaUrl()
│   │   ├── icons.ts              Lazy icon set + index, suggestIcons(), searchIcons(), recent icons
│   │   ├── dateRange.ts          Presets, previous period, local-date / UTC conversions
│   │   ├── exportCsv.ts          Export via Rust save dialog (or a download in the browser preview)
│   │   ├── iconSearch.ts         Pure tokenize/normalize/score (tested with bun)
│   │   └── categoryColor.ts, utils.ts
│   ├── data/
│   │   ├── icon-index.json       Generated: Tabler names, categories, tags (scripts/build-icon-index.mjs)
│   │   └── iconSynonyms.ts       Kurdish/Arabic/English shop words → icon keywords
│   └── utils/i18n/               languages table, locales/{en,ku,ar}.ts
├── scripts/build-icon-index.mjs  Regenerates icon-index.json from @tabler/icons metadata
├── tests/                        bun tests (icon suggestions)
└── src-tauri/
    ├── migrations/               0001_init … 0007_register
    ├── seeds/demo.sql            Sample catalog (IQD), debug builds only
    ├── src/lib.rs                Builder: sql plugin + migrations, read pool, session, commands
    ├── src/commands.rs           The command list → #[tauri::command]s + dispatch() for the bridge
    ├── src/ctx.rs                Ctx { pool, read_pool, session, media_dir }; now()
    ├── src/auth.rs               Hashing, sessions, require(permission), manager override, setup, login, end shift
    ├── src/staff.rs              Roles and employees, anti-escalation rules
    ├── src/catalog.rs            Categories (tree, order, archive) and products (barcodes, photos, initial stock)
    ├── src/sales.rs              Checkout (prices from the database, discounts, cash change), void_line
    ├── src/returns.rs            Returns and refunds against a sale
    ├── src/lookup.rs             Barcode lookup: catalog, then Open Food Facts
    ├── src/seeds.rs              Default names per language, demo catalog
    ├── src/stock.rs              Batches, receive/remove stock, soonest-expiry-first consumption
    ├── src/notifications.rs      Alert reconciliation, OS notifications
    ├── src/expenses.rs           Expenses, types, recurring drafts
    ├── src/export.rs             CSV with BOM; Rust opens the save dialog
    ├── src/background.rs         Hourly: recurring drafts + alert refresh
    ├── src/shifts.rs             Clock in/out, cash drawer and shift summary, schedules, copy week
    ├── src/settings.rs           Settings keys and shop profile writes
    ├── src/query.rs              Read-only `select` for the WebView
    ├── src/media.rs              Image pipeline, upload_media
    ├── src/audit.rs              audit_log writes
    ├── src/bridge.rs             Dev-only HTTP bridge (debug builds)
    ├── src/db.rs, src/error.rs
    ├── tauri.conf.json           Window, DB preload, asset protocol scope
    └── capabilities/default.json core + opener only (no SQL)
```

## Runtime flow

```
Tauri process (Rust)
  ├── sql plugin: opens pos.db and runs migrations before the window loads
  ├── setup: opens a second, read-only pool; starts the dev bridge (debug)
  └── WebView loads Vite build (dev: http://localhost:1420)
        └── LocaleProvider  ← language from settings, direction derived
              └── <router-view>  guards: setup → sign-in → permission
                    ├── /setup   SetupView     (no shop or no users yet)
                    ├── /        LoginView
                    └── /main    MainLayout
                                  ├── point-of-sale  sales.sell
                                  ├── inventory      inventory.view
                                  ├── analytics      analytics.view
                                  ├── staff          staff.view
                                  └── settings       (sections gated individually)
```

## Security model

The WebView is treated as untrusted: anything it can ask for, Rust checks.

- **No SQL from the WebView.** The capability file grants only `core` and `opener`. `tauri-plugin-sql`'s own `select` would run any statement (writes included), so the WebView never gets it.
- **Reads** go through the `select` command (`query.rs`), which runs on a second pool opened with SQLite's read-only flag. Any write fails inside SQLite ("attempt to write a readonly database"), however the SQL is disguised. A test covers `WITH … DELETE`.
- **Writes** are specific commands (`commands.rs`). Each starts with `auth::require(ctx, "<permission>")`, which re-reads the user's role and permissions from the database every time. Role edits apply immediately, and a deactivated user is signed out on their next action.
- **Escalation rules** (`staff.rs`): a non-owner can't grant a permission they don't hold, either in a role or by assigning a role; can't edit someone whose role outranks theirs; only owners hand out the Owner role. The last active owner can't be demoted or deactivated, and nobody can deactivate themselves.
- **Secrets**: passwords and PINs are argon2 hashes. A PIN alone signs someone in, so PINs are unique (checked against every other hash). Read queries never select hash columns.
- **Audit**: sign-ins, sign-outs, shifts, role, employee, schedule, settings and shop changes are written to `audit_log`.
- The UI mirrors permissions (`useAuth().can()`, `meta.permission`, hidden nav items), but that is convenience only.

Errors cross the boundary as `{ code, message, detail? }`. The UI shows `errors.<code>` from the locale files (`lib/errors.ts`); `forbidden` names the missing permission.

## Sign-in, sessions and shifts

- First run (`needsSetup`: no users, or no shop profile): `complete_setup` saves the shop, creates the Owner and signs them in. It is refused once any user exists. A shop saved by an older version is prefilled.
- `login` (username + password; usernames are case-insensitive) or `login_pin`. One session per app instance.
- Signing in starts the user's shift, or resumes their open one. It links the schedule that covers now or starts within the hour.
- *Switch user* (`logout`) keeps the shift open so cashiers can swap at the till. *End shift* (`end_shift`) clocks out, then signs out.
- The top bar shows the user and "Shift ends 18:00" (amber in the last 15 minutes), or time on shift when unscheduled.
- Shift and schedule times are RFC 3339 UTC strings written by Rust, so they compare as text. The schedule grid works in local time (weeks start Saturday) and converts at the edge. An end time before the start means the shift runs past midnight.

## Language and direction

Language is the setting; direction follows from the `languages` table in `src/utils/i18n/index.ts` (`ku`/`ar` → rtl, `en` → ltr). `LocaleProvider` sets the vue-i18n locale, writes `lang`/`dir` on `<html>`, and saves the choice with `set_setting("language")`. The shop picks it on the setup screen and it is locked after that: Rust refuses `language` once an owner exists, because every name in the shop (products, categories, roles, expense types) is stored once, in that language. `localStorage` only caches it for the first paint.

Fonts follow the language: `:lang(ku)` and `:lang(ar)` Rudaw with Vazirmatn fallback, `:lang(en)` Schibsted Grotesk. Inputs whose content is always one script set their own `lang`/`dir`. Default roles and expense types are renamed into the shop's language at setup (`seeds.rs`).

Chromium (and so WebView2 on Windows) has no Sorani locale data. `src/lib/locale.ts` detects that and uses Arabic-Iraq number rules plus hand-written Kurdish month and weekday names. Digits stay Western everywhere for now.

vue-i18n splits message keys on dots, so permission labels are nested (`permissions.sales.refund` → `permissions: { sales: { refund } }`).

Layout uses logical utilities (`ms-*`, `start-0`, `text-start`, `border-s`) so it mirrors automatically. Side panels open from the reading end. Wrap data that may be in another script in `<bdi>`; inside a translated sentence, wrap interpolated values in Unicode isolates (`\u2068…\u2069`) instead, as the alert and error messages do. reka-ui gets the direction from a `ConfigProvider` in `LocaleProvider`; without it, its menus and selects render left-to-right.

## Styling and theming

Colors, radii and fonts are CSS custom properties on `:root` (light) and `.dark`, exposed to Tailwind via `@theme inline`.

| Token | Light | Role |
| --- | --- | --- |
| `background` | `#F3F4F0` | till-paper app background |
| `card` | `#FFFFFF` | panels, the receipt |
| `foreground` | `#17201C` | ink |
| `primary` / `brand` | `#1E5B43` | pine; `brand` stays deep in dark mode |
| `sun` | `#F4B728` | the Charge action and "today" highlights |
| `destructive` | `#C8432B` | brick |

Category colors come from `categories.color` and are mixed with theme tokens by `categoryColor.ts`. Radii follow hierarchy (controls `rounded-md`, panels `rounded-xl`). `.num` gives lining figures and deliberately avoids tabular ones: Schibsted's tabular set widens `.` and `:`.

## Data

### Migrations

`sqlite:pos.db` lives in the app config directory (`~/Library/Application Support/com.macbookshop.pos/` on macOS). `tauri.conf.json` preloads it, so pending migrations run at startup. Migrations are SQL files listed in `db::migrations()`; never edit a shipped one, add a new one. `0003_people` rebuilds `users` (role text → `role_id`, adds `pin_hash` and employee fields) and replaces `user_permissions` with `permissions` / `roles` / `role_permissions`.

```
permissions ──< role_permissions >── roles ──< users
users ──< shifts >── shift_schedules (user's planned rota)
categories ──< products >─< product_barcodes;  suppliers ──< product_suppliers
customers ──< sales >── users;  sales ──< sale_items, payments, returns
users ──< stock_adjustments, cash_drawers, held_sales
shop_profile (single row) ──> media_files;  settings (key/value);  audit_log
```

Seeded roles: **Owner** (system role, implicitly every permission, can't be edited or deleted), **Manager** (everything except `roles.manage`, `settings.system`, `staff.edit`) and **Cashier** (`sales.sell`, `sales.reprint`, `inventory.view`).

### Media pipeline

`upload_media` takes raw bytes (header `x-file-name`). It requires a signed-in user, except during first-run setup. Steps: reject empty files and files over 25 MB, dedupe by SHA-256, decode, apply EXIF orientation, cap the longest side at 1600 px, encode lossy WebP q80, compute a 4×3 blurhash, write `<app_data_dir>/media/<uuid>.webp`, insert `media_files`. `<MediaImage>` paints the blurhash, then fades in the file via the asset protocol.

## Catalog and icons

- **Categories** form a two-level tree (`parent_id`; Rust refuses a third level or moving a parent under another). Names are unique per parent, case-insensitive. Every category has an `icon_key`; subcategories may leave `color` empty to use the parent's. Order is `display_order` within siblings, changed with `move_category`. Categories with products or children can't be deleted, only archived (hides them and their subcategories from the register and pickers).
- **Products** have one `name` (in the shop's language), optional `icon_key`, ordered photos in `product_media` (position 0 is the cover), a primary barcode on `products.barcode` and extras in `product_barcodes`. Barcodes are unique across both. A blank SKU becomes `P-00042`. Stock is set once at creation and logged as a `stock_adjustments` row; later changes are a separate permission (`inventory.adjust_stock`). Products not for sale (`is_active = 0`) are hidden from the register and scanner.
- **Barcode first**: the product form and Quick add start at the barcode. `lookup_barcode` (`lookup.rs`) checks the catalog, then (if `barcode_lookup` is on) Open Food Facts for name, brand, size, category tags and a photo, which goes through the media pipeline. Only the barcode leaves the machine; offline simply returns nothing. `lib/barcodeLookup.ts` guesses a category from the tags with the icon keyword index.
- **What a product looks like** (`ProductThumb`): cover photo → its icon → its category's icon → initials.
- **Icons**: icon keys are `"<set>:<name>"` (today only `tabler:`). The drawing data (~2 MB) and the tag index (~450 KB) are lazy chunks loaded in the background at startup; `AppIcon` keeps its box until they arrive. `@iconify/vue/offline` never calls the Iconify API.
- **Suggestions** (`lib/iconSearch.ts`): the name, in any of the three languages, is tokenized (sizes like `330ml` and the Arabic `ال` dropped, letter variants such as ي/ی and ك/ک folded). Tokens are mapped to English keywords through `iconSynonyms.ts` by prefix, so Kurdish suffixes still match, and icons are scored on name and tag matches. The parent category's icon category breaks ties. Loose prefix-only matches are dropped when there are three or more solid ones. Grow `iconSynonyms.ts` when shops name things it misses; `bun test` checks a set of real names.

## Money: sales, stock, alerts, expenses, reports

- **Checkout** (`sales.rs`) receives product ids, quantities, discount amounts, the payment method and cash tendered; prices, costs, tax and stock come from the database. The cart discount is spread over the lines in proportion, so `sale_items.subtotal` is always after every discount and tax is charged on that. Discounts need `sales.discount` or a manager's override token; voids (`void_line`) need `sales.void_line` or one and are only audited, since the cart isn't stored. It refuses inactive products, fractional quantities of piece/pack/box items, and more than is on hand (naming the product). One transaction writes `sales` (with the cashier's open `shift_id`), `sale_items` (snapshotting name, price and cost), and `payments`. The existing trigger lowers stock and logs the movement. Receipt numbers are `R-000123`. IQD totals are rounded to whole dinars.
- **Manager approval** (`auth::authorize_override`): another active user with the permission types their PIN; Rust keeps a single-use token for 5 minutes in the session state (cleared on sign-in/out) and the action passes it. Every attempt is audited; `sales.approved_by` / `returns.approved_by` record who approved.
- **Returns** (`returns.rs`): per sale line, up to what's left after earlier returns. The refund is the line's charged amount pro rata plus its tax, and the last unit of a line refunds exactly the remainder. Optional restock (a batch with no expiry). The refund is a negative `payments` row tied to the return, and `sales.payment_status` becomes `partially_refunded`/`refunded`.
- **Cash drawer**: a `cash_drawers` row per shift, opened with `opening_float`. `shift_summary` = float + cash payments − cash refunds of the shift (payments carry `shift_id`); `end_shift` stores the count and difference.
- **Receipts**: `PrintRoot.vue` renders the sale into `#print-root` (Code 128 from `lib/code128.ts`); print CSS hides everything else and `print_receipt` asks the webview to print (`window.print()` in a browser). Width 58/80 mm and auto-print are settings.
- **Batches** (`product_batches`): receiving stock adds a batch with optional expiry and cost; product creation records its opening stock as one. Sales and removals take from the soonest-expiring batch first. Stock older than batches is simply untracked for expiry; `products.stock_quantity` stays the count.
- **Alerts** (`notifications.rs`): `refresh` recomputes every condition from the data — low stock (≤ reorder level), out of stock, batches expiring within `expiry_warn_days` (default 7) or expired, recurring-expense drafts — and reconciles the table: one open row per condition, closed (`resolved_at`) when it stops being true, hidden (`dismissed_at`) if dismissed. It runs after every stock change, at startup and hourly (`background.rs`). New out-of-stock and expired alerts also become OS notifications (`os_notifications` setting), in the UI language. The bell shows only kinds the user can act on.
- **Expenses**: types with icons; amounts on local dates. A *repeating* expense is a template that produces a draft each week/month after its date; drafts appear as alerts and on the Expenses screen to confirm, edit or skip.
- **Analytics** (`db/analytics.ts`) works from sale lines, so later product edits never change history. Return lines join the same `lines` CTE as negative quantities and amounts on the return's date (no `sale_id`, so they don't count as transactions); the payment split works on payment dates, refunds included. Buckets and the busiest-hours heatmap use the till's local time (`'localtime'`); weeks start Saturday. The compare toggle uses the same-length period just before. Hours worked come from shifts, clipped to the range. Net profit = gross margin − confirmed expenses, shown only without staff/category/payment filters. The filter bar lives in the route query.
- **Export**: `save_csv` checks `analytics.export`, opens the save dialog in Rust (the WebView never supplies a path), and writes UTF-8 with a BOM so Excel shows Kurdish and Arabic.

## Developing in a browser: the dev bridge

`bun run tauri dev` also starts `bridge.rs` on `127.0.0.1:1430` (debug builds only; not compiled into release). Vite proxies `/__bridge/*` to it. A normal browser tab on `http://localhost:1420` then calls the same Rust commands on the same database and session as the app window (`lib/ipc.ts` picks the route automatically). The bridge only answers requests with an `x-pos-bridge: 1` header from that origin. Plain GETs of `/media/*` are allowed for `<img>` tags. This is how the UI is checked end to end without driving the native window.

## Conventions

- One screen per file in `views/`; extract a component when it is reused or the view passes ~300 lines.
- Every user-visible string goes through `$t()`, added to `en`, `ku` and `ar` together; the three files keep identical structure.
- New write → a function in the right Rust module that starts with `auth::require`, an entry in `commands!`, a line in `generate_handler!` (a test fails if you forget), and a Rust test.
- New read → a function in `src/db/*`. Never select password or PIN hashes.
- Dev builds compile dependencies with `opt-level = 2` (`Cargo.toml`): image processing and argon2 are ~100× slower unoptimized.
- Money through `formatCurrency()`; dates and times through `src/lib/locale.ts`.
- Design tokens only; logical spacing for anything that mirrors.
- Path alias `@/` → `src/`.

## Commands

```bash
bun install                  # install JS deps
bun run tauri dev            # desktop app + dev bridge (needs ports 1420 and 1430 free)
bun run build                # type-check (vue-tsc) and build the web bundle
bun run test                 # icon suggestion/search tests
cd src-tauri && cargo test   # auth, RBAC, shifts, catalog, checkout, batches, alerts, expenses, export, media, migrations
bun run icons:index          # rebuild src/data/icon-index.json after upgrading the icon packages
bun run tauri build          # native installer
```

With the app running, open http://localhost:1420 in a browser to use the UI there. In a debug build, Settings → General → *Load sample data* fills an empty catalog and adds ~60 days of sales by three people, their shifts and some expenses, so analytics has something to show. The two sample cashiers get random passwords.
