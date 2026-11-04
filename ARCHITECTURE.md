# Architecture

A desktop point-of-sale app for small shops. It runs offline on the till machine as a native window (Tauri), with a Vue 3 front end and a local SQLite database. The interface is trilingual: Kurdish Sorani (default, right-to-left), Arabic (right-to-left), and English (left-to-right).

> Status: Phases 1 and 2 of `PLAN.md` are in: SQLite data layer, three languages, Rust media pipeline, shop profile, real sign-in (password or PIN), dynamic roles and permissions enforced in Rust, employees, shifts and schedules. Checkout and product editing are still stubs, and analytics still shows sample numbers. `FEATURES.md` is the wider backlog.

## Stack

| Layer | Choice | Notes |
| --- | --- | --- |
| Desktop shell | Tauri 2 (Rust) | `src-tauri/`. All writes, auth and permission checks, media pipeline. |
| UI framework | Vue 3.5, `<script setup>` + TypeScript | Single-page app mounted in `index.html`. |
| Build | Vite 6 + `bun` | Dev server on fixed port `1420` (Tauri requires it). |
| Styling | Tailwind CSS v4 (`@tailwindcss/vite`) | Design tokens are CSS variables in `src/index.css`, mapped into Tailwind with `@theme inline`. |
| Fonts | Schibsted Grotesk, Vazirmatn (`@fontsource-variable`), Sarchia Makka (bundled) | Self-hosted so the till renders correctly offline. |
| Components | shadcn-vue (new-york style) on reka-ui | Copied into `src/components/ui/`; edit them freely, they are owned code. |
| Icons | lucide-vue-next | |
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
│   │   ├── AnalyticsView.vue     (sample data until Phase 4)
│   │   ├── StaffView.vue         Employees, Schedule, Roles tabs
│   │   └── SettingsView.vue      General (theme, tax, sample data), Shop
│   ├── components/
│   │   ├── menus/                Sidebar.vue (nav rail filtered by permission, user menu), Header.vue (shop, user, shift)
│   │   ├── staff/                EmployeesTab, RolesTab, ScheduleTab, EditSheet (side panel for forms)
│   │   ├── shop/                 ShopForm, LogoField
│   │   ├── media/MediaImage.vue  Blurhash placeholder → fades in the stored WebP
│   │   ├── providers/LocaleProvider.vue
│   │   ├── LanguageSwitcher.vue
│   │   └── ui/                   shadcn-vue primitives
│   ├── db/                       Read queries only (SQL lives here, never in views)
│   │   ├── client.ts             select()/selectOne() → Rust `select` command
│   │   ├── catalog.ts, settings.ts, shop.ts, staff.ts
│   ├── stores/
│   │   ├── auth.ts               Session mirror, can(), login/logout, MAIN_ROUTES, landingPath()
│   │   └── shop.ts               Shop profile, currency, tax rate
│   ├── hooks/                    useTimeManager, useTheme, useShiftStatus
│   ├── lib/
│   │   ├── ipc.ts                call(command, args): Tauri invoke, or the dev bridge in a browser
│   │   ├── errors.ts             errorMessage(): translates Rust error codes
│   │   ├── format.ts, locale.ts  Money, dates, times; Kurdish fallbacks
│   │   ├── media.ts              uploadMedia(), mediaUrl()
│   │   └── categoryColor.ts, utils.ts
│   └── utils/i18n/               languages table, localized(), locales/{en,ku,ar}.ts
└── src-tauri/
    ├── migrations/               0001_init, 0002_shop_media_i18n, 0003_people
    ├── seeds/demo.sql            Sample catalog (IQD), debug builds only
    ├── src/lib.rs                Builder: sql plugin + migrations, read pool, session, commands
    ├── src/commands.rs           The command list → #[tauri::command]s + dispatch() for the bridge
    ├── src/ctx.rs                Ctx { pool, read_pool, session, media_dir }; now()
    ├── src/auth.rs               Hashing, sessions, require(permission), setup, login, shifts on sign-in
    ├── src/staff.rs              Roles and employees, anti-escalation rules
    ├── src/shifts.rs             Clock in/out, schedules, copy week
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

Language is the setting; direction follows from the `languages` table in `src/utils/i18n/index.ts` (`ku`/`ar` → rtl, `en` → ltr). `LocaleProvider` sets the vue-i18n locale, writes `lang`/`dir` on `<html>`, and saves the choice with `set_setting("language")`. That key is the one setting anyone may change, so it works on the login screen. `localStorage` only caches it for the first paint.

Fonts follow the language: `:lang(ku)` Sarchia Makka with Vazirmatn fallback, `:lang(ar)` Vazirmatn, `:lang(en)` Schibsted Grotesk. Inputs whose content is always one script set their own `lang`/`dir`. `localized(row, "name")` picks `name_<lang>`, then `name`, then `name_en`.

Chromium (and so WebView2 on Windows) has no Sorani locale data. `src/lib/locale.ts` detects that and uses Arabic-Iraq number rules plus hand-written Kurdish month and weekday names. Digits stay Western everywhere for now.

vue-i18n splits message keys on dots, so permission labels are nested (`permissions.sales.refund` → `permissions: { sales: { refund } }`).

Layout uses logical utilities (`ms-*`, `start-0`, `text-start`, `border-s`) so it mirrors automatically. Side panels open from the reading end. Wrap data that may be in another script in `<bdi>`.

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

## Developing in a browser: the dev bridge

`bun run tauri dev` also starts `bridge.rs` on `127.0.0.1:1430` (debug builds only; not compiled into release). Vite proxies `/__bridge/*` to it. A normal browser tab on `http://localhost:1420` then calls the same Rust commands on the same database and session as the app window (`lib/ipc.ts` picks the route automatically). The bridge only answers requests with an `x-pos-bridge: 1` header from that origin. Plain GETs of `/media/*` are allowed for `<img>` tags. This is how the UI is checked end to end without driving the native window.

## Conventions

- One screen per file in `views/`; extract a component when it is reused or the view passes ~300 lines.
- Every user-visible string goes through `$t()`, added to `en`, `ku` and `ar` together; the three files keep identical structure.
- New write → a function in the right Rust module that starts with `auth::require`, an entry in `commands!`, a line in `generate_handler!` (a test fails if you forget), and a Rust test.
- New read → a function in `src/db/*`. Never select password or PIN hashes.
- Money through `formatCurrency()`; dates and times through `src/lib/locale.ts`.
- Design tokens only; logical spacing for anything that mirrors.
- Path alias `@/` → `src/`.

## Commands

```bash
bun install                  # install JS deps
bun run tauri dev            # desktop app + dev bridge (needs ports 1420 and 1430 free)
bun run build                # type-check (vue-tsc) and build the web bundle
cd src-tauri && cargo test   # auth, RBAC, shifts, settings, read-only select, media, migrations
bun run tauri build          # native installer
```

With the app running, open http://localhost:1420 in a browser to use the UI there. In a debug build, Settings → General → *Load sample data* fills an empty catalog.
