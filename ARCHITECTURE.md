# Architecture

A desktop point-of-sale app for small shops. It runs offline on the till machine as a native window (Tauri), with a Vue 3 front end and a local SQLite database. The interface is bilingual: English (left-to-right) and Kurdish Sorani (right-to-left).

> Status: the UI shell and every screen are built, but data is still sample data in the front end (`src/data/catalog.ts` and local refs in views). The SQLite schema exists (`db.sql`) and the SQL plugin is registered, but no screen reads from or writes to the database yet. Login, checkout, and settings save are stubs. See `PLAN.md` for the committed plan and `FEATURES.md` for the wider backlog.

## Stack

| Layer | Choice | Notes |
| --- | --- | --- |
| Desktop shell | Tauri 2 (Rust) | `src-tauri/`. Window, native bundling, SQL plugin. |
| UI framework | Vue 3.5, `<script setup>` + TypeScript | Single-page app mounted in `index.html`. |
| Build | Vite 6 + `bun` | Dev server on fixed port `1420` (Tauri requires it). |
| Styling | Tailwind CSS v4 (`@tailwindcss/vite`) | Design tokens are CSS variables in `src/index.css`, mapped into Tailwind with `@theme inline`. |
| Fonts | Schibsted Grotesk (`@fontsource-variable`), Sarchia Makka (bundled) | Self-hosted so the till renders correctly offline. |
| Components | shadcn-vue (new-york style) on reka-ui | Copied into `src/components/ui/`; edit them freely, they are owned code. |
| Icons | lucide-vue-next | |
| Charts | Chart.js 4 | Wrapped by `src/components/data/Chart.vue`. |
| Routing | vue-router 5, memory history | No URLs exposed; navigation is purely in-app. |
| i18n | vue-i18n 11 (composition mode) | Messages inline in `src/utils/i18n/index.ts`. |
| Persistence | `tauri-plugin-sql` (SQLite) | Schema in `db.sql`; a dev copy lives at `pos.db`. |

`flowbite` and `@tanstack/vue-table` are installed but unused; candidates for removal.

## Directory layout

```
pos/
├── index.html                 Vite entry; loads src/main.ts
├── db.sql                     SQLite schema (tables, indexes, triggers)
├── pos.db                     Local dev database
├── test.html                  Static prototype of the whole app (reference only, not built)
├── src/
│   ├── main.ts                Creates the app, installs router + i18n
│   ├── App.vue                Root: RTLProvider → <router-view>
│   ├── index.css              Tailwind import, design tokens, fonts, RTL base rules
│   ├── router/index.ts        Route table
│   ├── layouts/MainLayout.vue App chrome: nav rail + top bar + scrolling content
│   ├── views/                 One file per screen
│   │   ├── LoginView.vue
│   │   ├── SalesView.vue      The register (scan, cart, charge)
│   │   ├── InventoryView.vue
│   │   ├── AnalyticsView.vue
│   │   └── SettingsView.vue
│   ├── components/
│   │   ├── menus/             Sidebar.vue (permanent nav rail) and Header.vue (title, clock, language)
│   │   ├── providers/         RTLProvider: direction + locale state
│   │   ├── data/Chart.vue     Thin Chart.js wrapper
│   │   ├── RTLToggle.vue      Language switcher
│   │   └── ui/                shadcn-vue primitives (button, card, input, table, tabs, select, …)
│   ├── data/catalog.ts        Sample products/categories (stand-in for SQLite)
│   ├── hooks/
│   │   ├── useTimeManager.ts  Ticking clock for the header
│   │   └── useTheme.ts        Light/dark/system, persisted, applies `.dark` on <html>
│   ├── utils/
│   │   ├── dateUtils.ts
│   │   └── i18n/index.ts      All UI strings, en + ku
│   └── lib/
│       ├── utils.ts           `cn()` class merger
│       └── format.ts          Currency formatting (single place to change currency)
└── src-tauri/
    ├── src/lib.rs             Tauri builder: registers sql + opener plugins, sample commands
    ├── src/main.rs            Binary entry → pos_lib::run()
    ├── tauri.conf.json        Window, build hooks, bundle config
    └── capabilities/default.json  Grants core, opener, sql permissions to the main window
```

## Runtime flow

```
Tauri process (Rust)
  └── WebView loads Vite build (dev: http://localhost:1420)
        └── main.ts
              └── App.vue
                    └── RTLProvider   ← provides direction, language, setDirection()
                          └── <router-view>
                                ├── "/"      LoginView
                                └── "/main"  MainLayout
                                              ├── Sidebar (nav rail, always visible)
                                              ├── Header (screen title, clock, language)
                                              └── <router-view>
                                                    ├── point-of-sale → SalesView
                                                    ├── inventory     → InventoryView
                                                    ├── analytics     → AnalyticsView
                                                    └── settings      → SettingsView
```

### Routing

`createMemoryHistory()` is used because the app is a desktop window: there is no address bar and deep links are not needed. All signed-in screens are children of `/main` so they share `MainLayout`. Each child route carries `meta.titleKey`, which `MainLayout` translates for the header title. `/main` redirects to the register. There are no route guards yet; login just pushes to `/main/point-of-sale`.

### Direction and language

`RTLProvider.vue` owns two refs, `direction` (`ltr` | `rtl`) and `language` (`en` | `ku`), and `provide`s them plus `setDirection()`. Setting direction also switches the vue-i18n locale and writes `dir`/`lang` on `<html>`, then persists both to `localStorage`. Components read the direction with `inject("direction")`.

Layout code should use logical utilities (`ms-*`, `pe-*`, `start-0`, `text-start`, `border-s`) so it mirrors automatically; reach for `rtl:` variants only for things that genuinely need to flip, like directional icons (`rtl:rotate-180`). Kurdish text uses the bundled *Sarchia Makka* face; `index.css` sets it on `[dir="rtl"]` along with taller line-height and zero letter-spacing.

### Styling and theming

All colors, radii, and fonts are CSS custom properties on `:root` (light) and `.dark` (dark) in `src/index.css`. The `@theme inline` block exposes them to Tailwind as `bg-background`, `text-muted-foreground`, `bg-sidebar`, etc. shadcn components consume only these tokens, so a re-theme is a token change, not a component rewrite.

The palette:

| Token | Light | Role |
| --- | --- | --- |
| `background` | `#F3F4F0` | till-paper app background |
| `card` | `#FFFFFF` | panels, the receipt |
| `foreground` | `#17201C` | ink |
| `primary` / `brand` | `#1E5B43` | pine; `brand` stays deep in dark mode for the logo and login panel |
| `sun` | `#F4B728` | reserved for the Charge action |
| `destructive` | `#C8432B` | brick |
| `--cat-<id>` / `--cat-<id>-ink` | pastels | per-category tile tint and dot color |

Radii follow hierarchy rather than one value: controls use `rounded-md` (8px), panels `rounded-xl` (14px). Chart.js can't read Tailwind classes, so `AnalyticsView` resolves token values with `getComputedStyle` before building chart configs.

Dark mode is class-based (`@custom-variant dark`). `useTheme()` holds the shared light/dark/system choice, persists it to `localStorage`, follows the OS when set to system, and toggles `.dark` on `<html>`; `main.ts` calls `initTheme()` before mount to avoid a flash.

Mark numbers with the `.num` utility (lining figures). It deliberately does not enable tabular figures: Schibsted's tabular set also widens `.` and `:`, so `$1.50` renders as `$1 . 50`. End-align money columns instead.

Wrap user/product data that may be in another script with `<bdi>` so English product names render correctly inside RTL layouts.

## Data layer

### Current state

Every view keeps its own `ref([...])` of sample data. There is no store (no Pinia), no service layer, and no Tauri `invoke` calls besides the template's `greet` example in `App.vue`.

### Database schema (`db.sql`)

```
categories ─┐
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
settings (key/value)   audit_log (JSON before/after)
```

Triggers keep derived data in sync:

- `after_sale_item_insert`: decrements `products.stock_quantity` and writes a `sale` row to `stock_adjustments`.
- `update_customer_purchases`: adds each sale's total to `customers.total_purchases`.
- `update_product_timestamp` / `update_customer_timestamp`: bump `updated_at`.

`sale_items` snapshots `product_name`, `unit_price`, and `cost_price` so historical receipts and margins survive product edits or deletions.

### Intended wiring

The SQL plugin is already registered in `lib.rs` and permitted in `capabilities/default.json`, so the front end can talk to SQLite directly:

```ts
import Database from "@tauri-apps/plugin-sql";
const db = await Database.load("sqlite:pos.db");
const rows = await db.select("SELECT * FROM products WHERE barcode = $1", [code]);
```

Recommended shape as this grows:

1. `src/db/` – one module per aggregate (`products.ts`, `sales.ts`, …) exporting typed query functions. Views never write SQL.
2. Migrations via `tauri_plugin_sql::Builder::add_migrations` in `lib.rs`, seeded from `db.sql`, so schema changes ship with app updates.
3. A small Pinia store (or composables) for session state: signed-in user, open cash drawer, current cart, store settings.
4. Move anything security-sensitive (password/PIN hashing, permission checks) into Rust `#[tauri::command]`s rather than trusting the WebView.

## Conventions

- One screen per file in `views/`; extract a component only when it is reused or the view passes ~300 lines.
- Every user-visible string goes through `$t()`, with both `en` and `ku` keys added together.
- Use design tokens (`bg-card`, `text-muted-foreground`), never raw palette classes (`bg-gray-100`) or hex values, so dark mode and re-theming keep working.
- Use logical spacing/position utilities for anything that should mirror in RTL.
- Path alias `@/` → `src/`.

## Commands

```bash
bun install          # install JS deps
bun run tauri dev    # run the desktop app with hot reload
bun run dev          # run only the web UI in a browser at :1420
bun run build        # type-check (vue-tsc) and build the web bundle
bun run tauri build  # produce a native installer
```
