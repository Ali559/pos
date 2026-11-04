# Architecture

A desktop point-of-sale app for small shops. It runs offline on the till machine as a native window (Tauri), with a Vue 3 front end and a local SQLite database. The interface is trilingual: Kurdish Sorani (default, right-to-left), Arabic (right-to-left), and English (left-to-right).

> Status: Phase 1 of `PLAN.md` is in. The register and inventory read from SQLite, the shop profile and settings are stored there, images go through the Rust media pipeline, and language/direction are persisted. Login, checkout, and product editing are still stubs, and analytics still shows sample numbers. `FEATURES.md` is the wider backlog.

## Stack

| Layer | Choice | Notes |
| --- | --- | --- |
| Desktop shell | Tauri 2 (Rust) | `src-tauri/`. Window, SQL plugin, media pipeline, asset protocol for stored images. |
| UI framework | Vue 3.5, `<script setup>` + TypeScript | Single-page app mounted in `index.html`. |
| Build | Vite 6 + `bun` | Dev server on fixed port `1420` (Tauri requires it). |
| Styling | Tailwind CSS v4 (`@tailwindcss/vite`) | Design tokens are CSS variables in `src/index.css`, mapped into Tailwind with `@theme inline`. |
| Fonts | Schibsted Grotesk, Vazirmatn (`@fontsource-variable`), Sarchia Makka (bundled) | Self-hosted so the till renders correctly offline. |
| Components | shadcn-vue (new-york style) on reka-ui | Copied into `src/components/ui/`; edit them freely, they are owned code. |
| Icons | lucide-vue-next | |
| Charts | Chart.js 4 | Wrapped by `src/components/data/Chart.vue`. |
| Routing | vue-router 5, memory history | No URLs exposed; navigation is purely in-app. |
| i18n | vue-i18n 11 (composition mode) | One file per language in `src/utils/i18n/locales/`. |
| Persistence | `tauri-plugin-sql` (SQLite via sqlx) | Versioned migrations in `src-tauri/migrations/`. |
| Images | `image` (decode/orient/resize), `webp` (libwebp, lossy q80), `blurhash` | Rust only. |

`flowbite` and `@tanstack/vue-table` are installed but unused; candidates for removal.

## Directory layout

```
pos/
├── index.html                    Vite entry; loads src/main.ts
├── db.sql                        Legacy schema dump; migrations are the source of truth
├── src/
│   ├── main.ts                   Applies theme, installs router + i18n, mounts
│   ├── App.vue                   Root: LocaleProvider → <router-view>
│   ├── index.css                 Tailwind, design tokens, fonts per language
│   ├── router/index.ts           Routes + first-run guard (no shop profile → /setup)
│   ├── layouts/MainLayout.vue    Nav rail + top bar + scrolling content
│   ├── views/
│   │   ├── SetupView.vue         First run: shop name, logo, currency
│   │   ├── LoginView.vue
│   │   ├── SalesView.vue         The register (scan, cart, charge)
│   │   ├── InventoryView.vue
│   │   ├── AnalyticsView.vue     (sample data until Phase 4)
│   │   └── SettingsView.vue      General (theme, tax), Shop, Roles, Users
│   ├── components/
│   │   ├── menus/                Sidebar.vue (nav rail), Header.vue (shop logo + name, title, clock, language)
│   │   ├── providers/LocaleProvider.vue  Language state; direction derived from it
│   │   ├── LanguageSwitcher.vue  Lists each language in its own script
│   │   ├── shop/                 ShopForm.vue, LogoField.vue (shared by setup and settings)
│   │   ├── media/MediaImage.vue  Blurhash placeholder → fades in the stored WebP
│   │   ├── data/Chart.vue        Thin Chart.js wrapper
│   │   └── ui/                   shadcn-vue primitives
│   ├── db/                       All SQL lives here; views never write SQL
│   │   ├── client.ts             getDb(): plugin-sql in the app, sql.js in a plain browser (dev)
│   │   ├── browser.ts            Dev-only in-memory SQLite running the real migrations + demo seed
│   │   ├── catalog.ts            Categories, products, barcode lookup, stock state
│   │   ├── settings.ts           Key/value settings
│   │   └── shop.ts               Shop profile (single row) with its logo
│   ├── stores/shop.ts            Shared shop profile, currency, tax rate
│   ├── hooks/                    useTimeManager (clock), useTheme (light/dark/system)
│   ├── lib/
│   │   ├── format.ts             formatCurrency(): shop currency + UI language
│   │   ├── locale.ts             Intl locale per language, Kurdish fallbacks
│   │   ├── media.ts              uploadMedia(), mediaUrl()
│   │   ├── categoryColor.ts      Tile tint/ink mixed from a category's stored color
│   │   └── utils.ts              `cn()` class merger
│   └── utils/
│       ├── dateUtils.ts
│       └── i18n/                 index.ts (languages table, localized()), locales/{en,ku,ar}.ts
└── src-tauri/
    ├── migrations/               0001_init.sql, 0002_shop_media_i18n.sql, …
    ├── seeds/demo.sql            Sample catalog (IQD), dev only
    ├── src/lib.rs                Builder: sql plugin + migrations, commands
    ├── src/db.rs                 DB_URL, migration list, shared pool access, seed command
    ├── src/media.rs              upload_media / get_media_dir, image processing + tests
    ├── src/error.rs              Command error type (serialized as a message)
    ├── tauri.conf.json           Window, DB preload, asset protocol scope
    └── capabilities/default.json core, opener, sql (incl. execute)
```

## Runtime flow

```
Tauri process (Rust)
  ├── sql plugin: preloads sqlite:pos.db and runs migrations before the window loads
  └── WebView loads Vite build (dev: http://localhost:1420)
        └── main.ts → App.vue
              └── LocaleProvider  ← language (settings table), direction derived
                    └── <router-view>   guard: shop profile missing → /setup
                          ├── "/setup"  SetupView
                          ├── "/"       LoginView
                          └── "/main"   MainLayout (rail + header)
                                          ├── point-of-sale → SalesView
                                          ├── inventory     → InventoryView
                                          ├── analytics     → AnalyticsView
                                          └── settings      → SettingsView
```

### Routing

`createMemoryHistory()` is used because the app is a desktop window. All signed-in screens are children of `/main` so they share `MainLayout`; each carries `meta.titleKey` for the header title. A global `beforeEach` loads the shop store and sends a fresh install to `/setup` until a shop profile exists. Login still just pushes to the register (Phase 2).

### Language and direction

Language is the setting; direction follows from it via the `languages` table in `src/utils/i18n/index.ts` (`ku` → rtl, `ar` → rtl, `en` → ltr). `LocaleProvider` sets the vue-i18n locale, writes `lang`/`dir` on `<html>`, and persists the choice to the `settings` table (`language`, default `ku`). `localStorage` is only a cache so the first paint is already in the right language; the database value wins once loaded.

Fonts follow the language, not the direction: `:lang(ku)` uses Sarchia Makka with Vazirmatn as fallback for Sorani letters, `:lang(ar)` uses Vazirmatn, `:lang(en)` uses Schibsted Grotesk. Inputs whose content is always one script (e.g. "shop name in Arabic") set their own `lang`/`dir`.

Rows with per-language columns are read with `localized(row, "name")`, which tries `name_<lang>`, then `name`, then `name_en`, and skips empty values.

Chromium (and therefore WebView2 on Windows) has no Sorani locale data, so `Intl` silently falls back to English for `ckb`. `src/lib/locale.ts` detects that once and substitutes Arabic-Iraq number rules (same `د.ع` dinar sign) and hand-written Kurdish month and weekday names. Digits stay Western in every language for now.

Layout code uses logical utilities (`ms-*`, `pe-*`, `start-0`, `text-start`, `border-s`) so it mirrors automatically; `rtl:` variants only for things that must flip, like directional icons. Wrap data that may be in another script (product names) in `<bdi>`.

### Styling and theming

All colors, radii, and fonts are CSS custom properties on `:root` (light) and `.dark` (dark) in `src/index.css`. The `@theme inline` block exposes them to Tailwind. shadcn components consume only these tokens.

| Token | Light | Role |
| --- | --- | --- |
| `background` | `#F3F4F0` | till-paper app background |
| `card` | `#FFFFFF` | panels, the receipt |
| `foreground` | `#17201C` | ink |
| `primary` / `brand` | `#1E5B43` | pine; `brand` stays deep in dark mode for the login panel |
| `sun` | `#F4B728` | reserved for the Charge action |
| `destructive` | `#C8432B` | brick |

Category colors come from the database (`categories.color`, one hex). `categoryTint()` / `categoryInk()` mix it with `--card` / `--foreground` using `color-mix`, so the same stored color works in light and dark mode.

Radii follow hierarchy: controls `rounded-md`, panels `rounded-xl`. Chart.js can't read Tailwind classes, so `AnalyticsView` resolves token values with `getComputedStyle`. Dark mode is class-based; `useTheme()` persists light/dark/system and toggles `.dark` on `<html>`.

Mark numbers with `.num` (lining figures). It deliberately avoids tabular figures: Schibsted's tabular set widens `.` and `:`.

## Data layer

### Database and migrations

The database is `sqlite:pos.db` in the app config directory (`~/Library/Application Support/com.macbookshop.pos/` on macOS). `tauri.conf.json` preloads it, so the plugin opens it and runs pending migrations at startup, before any screen queries it. Migrations are plain SQL files in `src-tauri/migrations/`, listed in `db::migrations()`; sqlx records applied versions in `_sqlx_migrations`. Never edit a shipped migration; add a new one.

```
categories ─┐ (name, name_ku, name_ar, color)
            ├─< products >─< product_barcodes
suppliers ──┴─< product_suppliers
products ──< stock_adjustments >── users
customers ──< sales >── users
sales ──< sale_items >── products
sales ──< payments            (split payments)
sales ──< returns ──< return_items >── sale_items
users ──< user_permissions
users ──< cash_drawers        (shift open/close)
users ──< held_sales          (parked carts as JSON)
shop_profile (single row) ──> media_files (logo)
settings (key/value: language, tax_rate, …)   audit_log (JSON before/after)
```

Triggers keep derived data in sync: `after_sale_item_insert` decrements stock and logs a `sale` adjustment; `update_customer_purchases` totals customer spend; timestamp triggers bump `updated_at`. `sale_items` snapshots name and prices so receipts survive product edits.

### Who talks to the database

- **Front end** (`src/db/*`): reads and simple writes through `@tauri-apps/plugin-sql`, with typed functions per area. Views import these, never `getDb()` directly.
- **Rust** (`db::pool(app)`): borrows the same sqlx pool the plugin owns (`DbInstances`), for operations that must not be done in the WebView. Today that is media uploads; Phase 2 adds auth and permission checks here.

### Browser dev fallback

`bun run dev` in a normal browser has no Tauri. `getDb()` detects that (`isTauri()`) and, in dev builds only, loads `src/db/browser.ts`: sql.js in memory, running every file in `src-tauri/migrations/` plus `seeds/demo.sql`. Nothing persists across reloads, image upload is disabled, and the shop setup screen appears on each load. It exists so screens can be built and checked without the desktop shell. sql.js is stripped from production builds.

### Media pipeline

`upload_media` (Rust) takes the raw file bytes as the IPC body (header `x-file-name`):

1. Reject empty files and anything over 25 MB; hash the bytes (SHA-256). An identical earlier upload is returned as-is.
2. Decode (JPEG, PNG, WebP, GIF, BMP), apply EXIF orientation, resize so the longest side is at most 1600 px.
3. Encode lossy WebP at quality 80 with libwebp.
4. Blurhash (4×3 components) from a 32 px thumbnail.
5. Write `<app_data_dir>/media/<uuid>.webp`, insert a `media_files` row, return it.

The front end shows images with `<MediaImage>`: it paints the blurhash to a 32×32 canvas immediately and fades in the file, loaded via the asset protocol (`convertFileSrc`, scope `$APPDATA/media/**`).

## Conventions

- One screen per file in `views/`; extract a component when it is reused (e.g. `ShopForm`) or the view passes ~300 lines.
- Every user-visible string goes through `$t()`, with keys added to `en`, `ku` and `ar` together. The three locale files must have identical structure.
- Money always goes through `formatCurrency()`; dates through `src/lib/locale.ts`.
- Use design tokens, never raw palette classes or hex values (stored category colors are the exception, via `categoryColor.ts`).
- Use logical spacing/position utilities for anything that should mirror in RTL.
- Anything trusted (hashing, permissions, file writes, image processing) is a Rust command.
- Path alias `@/` → `src/`.

## Commands

```bash
bun install          # install JS deps
bun run tauri dev    # run the desktop app with hot reload (needs port 1420 free)
bun run dev          # web UI only, in a browser at :1420, on the in-memory dev DB
bun run build        # type-check (vue-tsc) and build the web bundle
cd src-tauri && cargo test   # media pipeline + migration tests
bun run tauri build  # produce a native installer
```

In a debug desktop build, Settings → General → *Load sample data* fills an empty catalog from `seeds/demo.sql`.
