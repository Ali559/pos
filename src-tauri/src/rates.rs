//! The US dollar rate: IQD per 100 USD, as the market quotes it.
//!
//! The Central Bank's rate (about 1,500) isn't what shops use; the market
//! (bourse) rate is (about 1,690 in Nov 2026) and moves several times a day.
//! No API publishes it, so the main computer reads public Telegram channels
//! that post the bourse prices per city (`t.me/s/<channel>`, the channel's
//! public web page). A rate read this way is only *offered*: a manager
//! accepts it (or types one) before the till uses it. Every rate used is
//! kept in `exchange_rates`.
//!
//! Settings: `usd_enabled` (take dollars, default off), `rate_fetch` (read
//! Telegram, default on), `rate_city` (which market, default Erbil).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::audit;
use crate::auth;
use crate::ctx::{now, Ctx};
use crate::error::{Error, Result};
use crate::settings;

/// Channels to read, best first. All post "city" on one line and the price
/// of 100 USD on the next.
pub const CHANNELS: &[&str] = &["dollariraqi", "uuuurrr9", "dollar_price"];
pub const CITIES: &[&str] = &["erbil", "sulaymaniyah", "duhok", "kirkuk", "baghdad", "mosul", "basra"];
const EVERY: Duration = Duration::from_secs(20 * 60);
/// A post older than this isn't today's rate any more
const FRESH_HOURS: i64 = 12;
/// What a rate per 100 USD may be. Far outside today's ~150,000–170,000, so
/// it only catches a misread number, not a market move.
pub const BAND: (f64, f64) = (100_000.0, 300_000.0);

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Rate {
    pub id: i64,
    pub per_100: f64,
    pub source: String,
    pub city: Option<String>,
    pub posted_at: Option<String>,
    pub set_by: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub per_100: f64,
    pub city: String,
    /// telegram:<channel>
    pub source: String,
    pub posted_at: String,
    pub fetched_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateStatus {
    pub enabled: bool,
    pub current: Option<Rate>,
    /// A newer rate read from Telegram, waiting for a manager
    pub offer: Option<Offer>,
    pub fetch: bool,
    pub city: String,
    pub last_fetch: Option<String>,
    pub last_error: Option<String>,
    pub history: Vec<Rate>,
}

pub async fn usd_enabled(ctx: &Ctx) -> Result<bool> {
    Ok(settings::get(ctx, "usd_enabled", "0").await? == "1")
}

/// Dollars at a rate, in whole dinars.
pub fn to_iqd(usd: f64, per_100: f64) -> f64 {
    (usd * per_100 / 100.0).round()
}

const SELECT: &str = "SELECT r.id, CAST(r.per_100 AS REAL) AS per_100, r.source, r.city, r.posted_at, u.full_name AS set_by, r.created_at
     FROM exchange_rates r LEFT JOIN users u ON u.id = r.user_id";

pub async fn current(ctx: &Ctx) -> Result<Option<Rate>> {
    Ok(sqlx::query_as::<_, Rate>(&format!("{SELECT} ORDER BY r.id DESC LIMIT 1")).fetch_optional(&ctx.pool).await?)
}

/// The rate a payment uses: dollars must be switched on and a rate set, and
/// the screen must have shown the rate that is current now.
pub async fn rate_for_payment(ctx: &Ctx, shown: Option<f64>) -> Result<f64> {
    if !usd_enabled(ctx).await? {
        return Err(Error::Rule("usd_off"));
    }
    let rate = current(ctx).await?.ok_or(Error::Rule("no_rate"))?;
    match shown {
        Some(s) if (s - rate.per_100).abs() < 0.5 => Ok(rate.per_100),
        _ => Err(Error::RuleAbout("rate_changed", rate.per_100.to_string())),
    }
}

async fn offer(ctx: &Ctx) -> Result<Option<Offer>> {
    let raw = settings::get(ctx, "rate_offer", "").await?;
    Ok(serde_json::from_str(&raw).ok())
}

async fn put(ctx: &Ctx, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, $3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .bind(now())
    .execute(&ctx.pool)
    .await?;
    Ok(())
}

/// For the till (everyone signed in) and Settings.
pub async fn exchange_rate(ctx: &Ctx) -> Result<RateStatus> {
    auth::current(ctx).await?;
    let history = sqlx::query_as::<_, Rate>(&format!("{SELECT} ORDER BY r.id DESC LIMIT 30")).fetch_all(&ctx.pool).await?;
    let opt = |s: String| Some(s).filter(|s| !s.is_empty());
    Ok(RateStatus {
        enabled: usd_enabled(ctx).await?,
        current: history.first().cloned(),
        offer: offer(ctx).await?,
        fetch: settings::get(ctx, "rate_fetch", "1").await? == "1",
        city: settings::get(ctx, "rate_city", "erbil").await?,
        last_fetch: opt(settings::get(ctx, "rate_last_fetch", "").await?),
        last_error: opt(settings::get(ctx, "rate_last_error", "").await?),
        history,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRateArgs {
    pub per_100: f64,
}

async fn insert(ctx: &Ctx, user_id: i64, per_100: f64, source: &str, city: Option<&str>, posted_at: Option<&str>) -> Result<i64> {
    if !(BAND.0..=BAND.1).contains(&per_100) {
        return Err(Error::Rule("rate_range"));
    }
    let id = sqlx::query("INSERT INTO exchange_rates (per_100, source, city, posted_at, user_id, created_at) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(per_100.round())
        .bind(source)
        .bind(city)
        .bind(posted_at)
        .bind(user_id)
        .bind(now())
        .execute(&ctx.pool)
        .await?
        .last_insert_rowid();
    // Whatever was offered is now settled
    put(ctx, "rate_offer", "").await?;
    audit::log(ctx, Some(user_id), "rate.set", "exchange_rates", Some(id), &serde_json::json!({ "per100": per_100.round(), "source": source, "city": city })).await?;
    Ok(id)
}

/// A rate typed by a manager.
pub async fn set_rate(ctx: &Ctx, args: SetRateArgs) -> Result<RateStatus> {
    let actor = auth::require(ctx, "settings.rates").await?;
    insert(ctx, actor.user_id, args.per_100, "manual", None, None).await?;
    exchange_rate(ctx).await
}

/// Uses the rate read from Telegram. The screen says which number it showed,
/// so a newer offer that arrived meanwhile isn't accepted unseen.
pub async fn accept_rate_offer(ctx: &Ctx, args: SetRateArgs) -> Result<RateStatus> {
    let actor = auth::require(ctx, "settings.rates").await?;
    let o = offer(ctx).await?.ok_or(Error::NotFound)?;
    if (o.per_100 - args.per_100).abs() >= 0.5 {
        return Err(Error::RuleAbout("rate_changed", o.per_100.to_string()));
    }
    insert(ctx, actor.user_id, o.per_100, &o.source, Some(&o.city), Some(&o.posted_at)).await?;
    exchange_rate(ctx).await
}

pub async fn dismiss_rate_offer(ctx: &Ctx) -> Result<RateStatus> {
    auth::require(ctx, "settings.rates").await?;
    // Remember what was turned down, so the same post isn't offered again
    if let Some(o) = offer(ctx).await? {
        put(ctx, "rate_dismissed", &o.posted_at).await?;
    }
    put(ctx, "rate_offer", "").await?;
    exchange_rate(ctx).await
}

/// "Check now" in Settings.
pub async fn fetch_rate_now(ctx: &Ctx) -> Result<RateStatus> {
    auth::require(ctx, "settings.rates").await?;
    fetch(ctx).await?;
    exchange_rate(ctx).await
}

// ------------------------------------------------------------- fetching

/// Reads the channels and offers a rate when there's a new one.
pub async fn fetch(ctx: &Ctx) -> Result<()> {
    let city = settings::get(ctx, "rate_city", "erbil").await?;
    let at = now();
    put(ctx, "rate_last_fetch", &at).await?;
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (compatible; pos-rate-check)")
        .build()
        .map_err(|e| Error::Invalid(e.to_string()))?;
    let mut found = None;
    let mut errors = Vec::new();
    for channel in CHANNELS {
        let page = match http.get(format!("https://t.me/s/{channel}")).send().await {
            Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
            Ok(r) => {
                errors.push(format!("{channel}: HTTP {}", r.status()));
                continue;
            }
            Err(e) => {
                errors.push(format!("{channel}: {e}"));
                continue;
            }
        };
        match newest(&page, &city) {
            Some(p) if fresh(&p.posted_at, &at) => {
                found = Some(Offer { per_100: p.per_100, city: city.clone(), source: format!("telegram:{channel}"), posted_at: p.posted_at, fetched_at: at.clone() });
                break;
            }
            Some(_) => errors.push(format!("{channel}: no post in the last {FRESH_HOURS} hours")),
            None => errors.push(format!("{channel}: no rate for {city}")),
        }
    }
    let Some(o) = found else {
        put(ctx, "rate_last_error", &errors.join("; ")).await?;
        return Ok(());
    };
    put(ctx, "rate_last_error", "").await?;
    let cur = current(ctx).await?;
    let dismissed = settings::get(ctx, "rate_dismissed", "").await?;
    if worth_offering(&o, cur.as_ref(), offer(ctx).await?.as_ref(), &dismissed) {
        put(ctx, "rate_offer", &serde_json::to_string(&o)?).await?;
    }
    Ok(())
}

/// Offer a rate only when it's news: a later post than what's in use,
/// offered or turned down, with a different number from the one in use.
fn worth_offering(o: &Offer, current: Option<&Rate>, offered: Option<&Offer>, dismissed: &str) -> bool {
    if o.posted_at.as_str() <= dismissed {
        return false;
    }
    if offered.is_some_and(|x| x.posted_at >= o.posted_at) {
        return false;
    }
    match current {
        None => true,
        Some(c) => (c.per_100 - o.per_100).abs() >= 0.5 && c.posted_at.as_deref().is_none_or(|p| p < o.posted_at.as_str()),
    }
}

fn fresh(posted_at: &str, now: &str) -> bool {
    let (Ok(p), Ok(n)) = (chrono::DateTime::parse_from_rfc3339(posted_at), chrono::DateTime::parse_from_rfc3339(now)) else {
        return false;
    };
    n.signed_duration_since(p).num_hours() < FRESH_HOURS
}

/// Every 20 minutes on the main computer, while dollars and fetching are on.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            if let Ok(ctx) = Ctx::from_app(&app).await {
                let on = usd_enabled(&ctx).await.unwrap_or(false) && settings::get(&ctx, "rate_fetch", "1").await.ok().as_deref() == Some("1");
                if on {
                    if let Err(e) = fetch(&ctx).await {
                        eprintln!("[rates] {e}");
                    }
                }
            }
            tokio::time::sleep(EVERY).await;
        }
    });
}

// -------------------------------------------------------------- parsing

#[derive(Debug, PartialEq)]
pub struct Posted {
    pub per_100: f64,
    /// RFC 3339, from the post's <time datetime>
    pub posted_at: String,
}

/// How each city may be written (Arabic, with Persian/Kurdish letters too).
fn city_names(city: &str) -> &'static [&'static str] {
    match city {
        "erbil" => &["اربيل", "هولير"],
        "sulaymaniyah" => &["السليمانيه", "سليمانيه"],
        "duhok" => &["دهوك"],
        "kirkuk" => &["كركوك"],
        "baghdad" => &["بغداد"],
        "mosul" => &["الموصل", "موصل"],
        "basra" => &["البصره", "بصره"],
        _ => &[],
    }
}

/// Arabic-script letters only, with the spellings that vary made the same
/// (أ/إ/آ → ا, Persian ی/ک → ي/ك, ة → ه).
fn letters(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            'أ' | 'إ' | 'آ' | 'ٱ' => Some('ا'),
            'ی' | 'ى' | 'ې' | 'ێ' => Some('ي'),
            'ک' => Some('ك'),
            'ة' | 'ە' => Some('ه'),
            c if ('\u{0621}'..='\u{064A}').contains(&c) || ('\u{0671}'..='\u{06D3}').contains(&c) => Some(c),
            _ => None,
        })
        .collect()
}

/// "169,550" / "169.550" / "١٦٩٬٥٥٠" → 169550. A per-dollar number (1,695)
/// is taken as per 100.
fn amount(s: &str) -> Option<f64> {
    let digits: String = s
        .trim()
        .chars()
        .filter_map(|c| match c {
            '0'..='9' => Some(c),
            '٠'..='٩' => char::from_u32(c as u32 - '٠' as u32 + '0' as u32),
            '۰'..='۹' => char::from_u32(c as u32 - '۰' as u32 + '0' as u32),
            ',' | '.' | '٬' | '٫' | '،' | ' ' | '\u{a0}' => None,
            _ => Some('x'),
        })
        .collect();
    if digits.is_empty() || digits.contains('x') {
        return None;
    }
    let n: f64 = digits.parse().ok()?;
    let n = if (1_000.0..=3_000.0).contains(&n) { n * 100.0 } else { n };
    (BAND.0..=BAND.1).contains(&n).then_some(n)
}

/// A post's text: line breaks kept, tags and emoji images dropped.
fn text_of(html: &str) -> String {
    let html = html.replace("<br/>", "\n").replace("<br>", "\n").replace("<br />", "\n");
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&nbsp;", " ").replace("&#036;", "$").replace("&quot;", "\"").replace("&#39;", "'")
}

/// The rate for `city` in one post: a line that is just the city's name,
/// and the price of 100 USD on the same line after it or on the next one.
pub fn rate_in(text: &str, city: &str) -> Option<f64> {
    let names = city_names(city);
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    for (i, line) in lines.iter().enumerate() {
        let (head, tail) = match line.find(|c: char| c.is_ascii_digit() || ('٠'..='٩').contains(&c)) {
            Some(at) => line.split_at(at),
            None => (*line, ""),
        };
        if !names.contains(&letters(head).as_str()) {
            continue;
        }
        if let Some(n) = amount(tail) {
            return Some(n);
        }
        if tail.trim().is_empty() {
            if let Some(n) = lines.get(i + 1).and_then(|next| amount(next)) {
                return Some(n);
            }
        }
    }
    None
}

/// The newest post on a channel page that has a rate for `city`.
pub fn newest(page: &str, city: &str) -> Option<Posted> {
    let mut best: Option<Posted> = None;
    for block in page.split("tgme_widget_message_wrap").skip(1) {
        let Some(start) = block.find("tgme_widget_message_text") else { continue };
        let body = &block[start..];
        let body = &body[body.find('>').map_or(0, |i| i + 1)..];
        let body = &body[..body.find("</div>").unwrap_or(body.len())];
        let Some(per_100) = rate_in(&text_of(body), city) else { continue };
        let Some(posted_at) = block.split("<time datetime=\"").nth(1).and_then(|t| t.split('"').next()) else { continue };
        if best.as_ref().is_none_or(|b| b.posted_at.as_str() < posted_at) {
            best = Some(Posted { per_100, posted_at: posted_at.to_owned() });
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = include_str!("fixtures/telegram_rates.html");

    #[test]
    fn reads_the_newest_bourse_post_per_city() {
        let p = newest(PAGE, "erbil").unwrap();
        assert_eq!(p, Posted { per_100: 169_550.0, posted_at: "2026-11-14T09:24:07+00:00".into() });
        assert_eq!(newest(PAGE, "sulaymaniyah").unwrap().per_100, 169_650.0);
        assert_eq!(newest(PAGE, "baghdad").unwrap().per_100, 169_450.0);
        assert_eq!(newest(PAGE, "duhok").unwrap().per_100, 169_650.0);
        assert_eq!(newest(PAGE, "nowhere"), None);
    }

    #[test]
    fn numbers_and_names_in_their_usual_spellings() {
        assert_eq!(amount("169,550"), Some(169_550.0));
        assert_eq!(amount("169.400"), Some(169_400.0));
        assert_eq!(amount("١٦٩٬٥٥٠"), Some(169_550.0));
        assert_eq!(amount("1,695"), Some(169_500.0), "per dollar");
        assert_eq!(amount("955,000 دينار"), None, "not a dollar price");
        assert_eq!(amount("12"), None);
        assert_eq!(rate_in("🏛️ اربیل\n169,550", "erbil"), Some(169_550.0));
        assert_eq!(rate_in("▣ أربيل : 169,300", "erbil"), Some(169_300.0));
        // A remittance price list that names Erbil in a sentence isn't a rate
        assert_eq!(rate_in("💰اسعار الحوالات في اربیل:\nدبی کاش : +104", "erbil"), None);
    }

    /// Today's pages, saved by hand: `POS_RATE_PAGES=<dir with <channel>.html>
    /// cargo test channels_today -- --ignored --nocapture`. For checking the
    /// channels still post in a shape the parser reads.
    #[test]
    #[ignore]
    fn channels_today() {
        let dir = std::env::var("POS_RATE_PAGES").expect("POS_RATE_PAGES");
        for channel in CHANNELS {
            let page = std::fs::read_to_string(format!("{dir}/{channel}.html")).unwrap_or_default();
            for city in CITIES {
                println!("{channel} {city}: {:?}", newest(&page, city));
            }
        }
    }

    fn rate(per_100: f64, posted_at: Option<&str>) -> Rate {
        Rate { id: 1, per_100, source: "manual".into(), city: None, posted_at: posted_at.map(Into::into), set_by: None, created_at: String::new() }
    }

    #[test]
    fn offers_only_news() {
        let o = Offer { per_100: 169_550.0, city: "erbil".into(), source: "telegram:x".into(), posted_at: "2026-11-14T09:24:07+00:00".into(), fetched_at: String::new() };
        assert!(worth_offering(&o, None, None, ""), "no rate yet");
        assert!(worth_offering(&o, Some(&rate(169_000.0, None)), None, ""));
        assert!(!worth_offering(&o, Some(&rate(169_550.0, None)), None, ""), "same number");
        assert!(!worth_offering(&o, Some(&rate(168_000.0, Some("2026-11-14T10:00:00+00:00"))), None, ""), "older than the one in use");
        assert!(!worth_offering(&o, None, Some(&o), ""), "already offered");
        assert!(!worth_offering(&o, None, None, "2026-11-14T09:24:07+00:00"), "turned down");
        assert!(fresh("2026-11-14T09:24:07+00:00", "2026-11-14T15:00:00+00:00"));
        assert!(!fresh("2026-11-13T09:24:07+00:00", "2026-11-14T15:00:00+00:00"));
    }

    /// Dollars at the till and to a dollar supplier, through the drawer.
    #[test]
    fn dollars_through_the_till_and_the_drawer() {
        use crate::auth::test_helpers::setup_owner;
        use crate::catalog::create_product;
        use crate::ctx::test_support;
        use crate::sales::checkout;
        use crate::sales::tests::{product, sell};
        use crate::shifts::shift_summary;
        use crate::suppliers::*;
        tauri::async_runtime::block_on(async {
            let ctx = test_support::ctx().await;
            setup_owner(&ctx).await;
            // Cans at 750
            let cola = create_product(&ctx, product("Cola", "1", 1000.0)).await.unwrap();
            let start = shift_summary(&ctx).await.unwrap().unwrap().expected_cash;

            // Off, then no rate
            let usd = |n: f64, rate: Option<f64>| crate::sales::CheckoutArgs { tendered_usd: n, rate, ..sell(cola, 200.0, "cash") };
            assert!(matches!(checkout(&ctx, usd(100.0, Some(169_550.0))).await, Err(Error::Rule("usd_off"))));
            sqlx::query("INSERT INTO settings (key, value) VALUES ('usd_enabled', '1')").execute(&ctx.pool).await.unwrap();
            assert!(matches!(checkout(&ctx, usd(100.0, Some(169_550.0))).await, Err(Error::Rule("no_rate"))));
            set_rate(&ctx, SetRateArgs { per_100: 169_550.0 }).await.unwrap();
            assert!(matches!(set_rate(&ctx, SetRateArgs { per_100: 1_695.0 }).await, Err(Error::Rule("rate_range"))));
            // A screen still showing an older rate
            assert!(matches!(checkout(&ctx, usd(100.0, Some(168_000.0))).await, Err(Error::RuleAbout("rate_changed", _))));

            // 10 cans = 7,500: $5 (8,478) is enough, 978 back in dinars
            let sale = checkout(&ctx, crate::sales::CheckoutArgs { tendered_usd: 5.0, rate: Some(169_550.0), ..sell(cola, 10.0, "cash") }).await.unwrap();
            assert_eq!((sale.total, sale.usd_value, sale.change), (7_500.0, 8_478.0, Some(978.0)));
            // 200 cans = 150,000: $100 (169,550) and 19,550 back
            let sale = checkout(&ctx, usd(100.0, Some(169_550.0))).await.unwrap();
            assert_eq!(sale.change, Some(19_550.0));
            // Not enough, or not needed at all
            let short = crate::sales::CheckoutArgs { tendered_usd: 1.0, rate: Some(169_550.0), ..sell(cola, 10.0, "cash") };
            assert!(matches!(checkout(&ctx, short).await, Err(Error::Rule("tendered_short"))));
            let spare = crate::sales::CheckoutArgs { tendered: Some(10_000.0), tendered_usd: 1.0, rate: Some(169_550.0), ..sell(cola, 10.0, "cash") };
            assert!(matches!(checkout(&ctx, spare).await, Err(Error::Rule("usd_not_needed"))));
            // 5,000 dinars and $2 (3,391) for 7,500: 891 back
            let mixed = crate::sales::CheckoutArgs { tendered: Some(5_000.0), tendered_usd: 2.0, rate: Some(169_550.0), ..sell(cola, 10.0, "cash") };
            assert_eq!(checkout(&ctx, mixed).await.unwrap().change, Some(891.0));

            let sum = shift_summary(&ctx).await.unwrap().unwrap();
            assert_eq!(sum.usd_in, 107.0);
            assert_eq!(sum.usd_change, 978.0 + 19_550.0 + 891.0);
            assert_eq!(sum.expected_usd, 107.0);
            // The dinars: +5,000 handed over, −change given
            assert_eq!(sum.expected_cash, start + 5_000.0 - sum.usd_change);
            // Sales still add up to what was sold
            let (paid,): (f64,) = sqlx::query_as("SELECT CAST(SUM(amount) AS REAL) FROM payments WHERE sale_id IS NOT NULL").fetch_one(&ctx.pool).await.unwrap();
            assert_eq!(paid, 7_500.0 * 2.0 + 150_000.0);

            // A dollar supplier: cans at $0.30 → 509 dinars each at 169,550
            let sup = create_supplier(&ctx, SupplierInput { name: "Asia Drinks".into(), is_active: true, currency: Some("USD".into()), ..Default::default() }).await.unwrap();
            let r = receive_delivery(&ctx, DeliveryArgs {
                supplier_id: Some(sup),
                lines: vec![DeliveryLine { product_id: cola, quantity: 100.0, cost: 0.3, ..Default::default() }],
                paid: 10.0,
                paid_method: Some("till".into()),
                rate: Some(169_550.0),
                ..Default::default()
            })
            .await
            .unwrap();
            assert_eq!((r.total, r.currency.as_str(), r.owed), (30.0, "USD", 20.0));
            let (cost,): (f64,) = sqlx::query_as("SELECT CAST(cost_price AS REAL) FROM products WHERE id = $1").bind(cola).fetch_one(&ctx.pool).await.unwrap();
            assert_eq!(cost, 508.65);
            // $10 left the dollars; then $5 paid with dinars (8,478) from the till
            pay_supplier(&ctx, SupplierPaymentArgs { supplier_id: sup, amount: 5.0, method: "till".into(), note: None, paid_in: Some("IQD".into()), rate: Some(169_550.0) }).await.unwrap();
            let after = shift_summary(&ctx).await.unwrap().unwrap();
            assert_eq!(after.expected_usd, 97.0);
            assert_eq!(after.paid_to_suppliers_usd, -10.0);
            assert_eq!(after.expected_cash, sum.expected_cash - 8_478.0);
            // Its currency is fixed while something is owed
            let edit = UpdateSupplierArgs { id: sup, supplier: SupplierInput { name: "Asia Drinks".into(), is_active: true, currency: Some("IQD".into()), ..Default::default() } };
            assert!(matches!(update_supplier(&ctx, edit).await, Err(Error::Rule("currency_has_balance"))));
        });
    }

    #[test]
    fn dollars_to_dinars() {
        assert_eq!(to_iqd(100.0, 169_550.0), 169_550.0);
        assert_eq!(to_iqd(1.0, 169_550.0), 1_696.0);
        assert_eq!(to_iqd(20.0, 169_550.0), 33_910.0);
    }
}
