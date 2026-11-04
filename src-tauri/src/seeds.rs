//! Built-in names in all three languages. A shop works in one language,
//! chosen at setup; these pick the matching column when rows are created.

use sqlx::SqlitePool;

use crate::error::Result;

/// [en, ku, ar]
type Names = [&'static str; 3];

pub fn pick(names: &Names, lang: &str) -> &'static str {
    match lang {
        "ku" => names[1],
        "ar" => names[2],
        _ => names[0],
    }
}

/// Role ids seeded by migration 0003.
const ROLES: [(i64, Names); 3] = [
    (1, ["Owner", "خاوەن", "المالك"]),
    (2, ["Manager", "بەڕێوەبەر", "المدير"]),
    (3, ["Cashier", "کاشێر", "الكاشير"]),
];

/// Expense type ids seeded by migration 0005.
const EXPENSE_TYPES: [(i64, Names); 6] = [
    (1, ["Rent", "کرێ", "إيجار"]),
    (2, ["Electricity", "کارەبا", "كهرباء"]),
    (3, ["Generator", "مۆلیدە", "مولدة"]),
    (4, ["Salaries", "مووچە", "رواتب"]),
    (5, ["Supplies", "پێداویستی", "لوازم"]),
    (6, ["Other", "هیتر", "أخرى"]),
];

/// Renames the seeded roles and expense types into the shop's language.
/// Called once, during first-run setup.
pub async fn localize_defaults(pool: &SqlitePool, lang: &str) -> Result<()> {
    for (id, names) in ROLES {
        sqlx::query("UPDATE roles SET name = $1 WHERE id = $2")
            .bind(pick(&names, lang))
            .bind(id)
            .execute(pool)
            .await?;
    }
    for (id, names) in EXPENSE_TYPES {
        sqlx::query("UPDATE expense_types SET name = $1 WHERE id = $2")
            .bind(pick(&names, lang))
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

// ------------------------------------------------- sample catalog (dev)

struct DemoCategory {
    id: i64,
    parent: Option<i64>,
    names: Names,
    color: &'static str,
    icon: &'static str,
    order: i64,
}

const DEMO_CATEGORIES: [DemoCategory; 7] = [
    DemoCategory { id: 1, parent: None, names: ["Beverages", "خواردنەوە", "مشروبات"], color: "#2d5a8f", icon: "tabler:bottle", order: 1 },
    DemoCategory { id: 2, parent: None, names: ["Snacks", "سووکەخۆراک", "وجبات خفيفة"], color: "#94600c", icon: "tabler:candy", order: 2 },
    DemoCategory { id: 3, parent: None, names: ["Dairy", "شیرەمەنی", "ألبان"], color: "#5f4996", icon: "tabler:milk", order: 3 },
    DemoCategory { id: 4, parent: None, names: ["Bakery", "نانەوایی", "مخبوزات"], color: "#9a3f26", icon: "tabler:baguette", order: 4 },
    DemoCategory { id: 5, parent: None, names: ["Produce", "میوە و سەوزە", "خضار وفواكه"], color: "#2f6b38", icon: "tabler:apple", order: 5 },
    DemoCategory { id: 6, parent: Some(1), names: ["Soft drinks", "خواردنەوەی گازی", "مشروبات غازية"], color: "#2d5a8f", icon: "tabler:bottle", order: 1 },
    DemoCategory { id: 7, parent: Some(1), names: ["Hot drinks", "خواردنەوەی گەرم", "مشروبات ساخنة"], color: "#2d5a8f", icon: "tabler:coffee", order: 2 },
];

/// (sku, names, category, barcode, cost, price, stock, min, unit), IQD
type DemoProduct = (&'static str, Names, i64, &'static str, f64, f64, f64, f64, &'static str);

const DEMO_PRODUCTS: [DemoProduct; 15] = [
    ("BEV-001", ["Coca-Cola 330ml", "کۆکاکۆلا 330ml", "كوكا كولا 330 مل"], 6, "5449000000996", 500.0, 750.0, 142.0, 24.0, "piece"),
    ("BEV-002", ["Mineral water 1.5L", "ئاوی کانزایی 1.5 لیتر", "مياه معدنية 1.5 لتر"], 1, "6281006432108", 300.0, 500.0, 88.0, 24.0, "piece"),
    ("BEV-003", ["Ayran 250ml", "دۆ 250ml", "لبن عيران 250 مل"], 1, "8690526012345", 500.0, 750.0, 9.0, 12.0, "piece"),
    ("BEV-004", ["Black tea 500g", "چای ڕەش 500 گرام", "شاي أسود 500 غ"], 7, "8690000123401", 4500.0, 6000.0, 31.0, 6.0, "piece"),
    ("SNK-001", ["Potato chips salted", "چیپسی پەتاتە بە خوێ", "شيبس بطاطا مملح"], 2, "6281036111110", 650.0, 1000.0, 57.0, 12.0, "piece"),
    ("SNK-002", ["Roasted pistachios 200g", "پستەی برژاو 200 گرام", "فستق محمص 200 غ"], 2, "6260001234567", 3750.0, 5000.0, 4.0, 8.0, "piece"),
    ("SNK-003", ["Dark chocolate bar", "شوکولاتەی تاریک", "لوح شوكولاتة داكنة"], 2, "7622300489434", 1400.0, 2000.0, 40.0, 10.0, "piece"),
    ("DRY-001", ["Fresh milk 1L", "شیری تازە 1 لیتر", "حليب طازج 1 لتر"], 3, "6281007012345", 1250.0, 1750.0, 26.0, 12.0, "piece"),
    ("DRY-002", ["Yogurt 500g", "ماست 500 گرام", "لبن زبادي 500 غ"], 3, "6281007054321", 1000.0, 1500.0, 0.0, 10.0, "piece"),
    ("DRY-003", ["White cheese 400g", "پەنیری سپی 400 گرام", "جبن أبيض 400 غ"], 3, "6281007099887", 3500.0, 4500.0, 18.0, 6.0, "piece"),
    ("BAK-001", ["Samoon bread (5)", "نانی سەمون (5)", "خبز صمون (5)"], 4, "2000000000011", 600.0, 1000.0, 60.0, 20.0, "piece"),
    ("BAK-002", ["Klicha date cookies", "کلێچەی خورما", "كليجة بالتمر"], 4, "2000000000028", 2500.0, 3500.0, 14.0, 6.0, "piece"),
    ("PRD-001", ["Tomatoes 1kg", "تەماتە 1 کیلۆ", "طماطة 1 كغ"], 5, "2000000000035", 900.0, 1250.0, 35.0, 10.0, "kg"),
    ("PRD-002", ["Cucumbers 1kg", "خەیار 1 کیلۆ", "خيار 1 كغ"], 5, "2000000000042", 700.0, 1000.0, 7.0, 10.0, "kg"),
    ("PRD-003", ["Pomegranates 1kg", "هەنار 1 کیلۆ", "رمان 1 كغ"], 5, "2000000000059", 2200.0, 3000.0, 22.0, 8.0, "kg"),
];

/// The 15-product sample catalog in the shop's language.
pub async fn insert_demo_catalog(pool: &SqlitePool, lang: &str) -> Result<()> {
    for c in &DEMO_CATEGORIES {
        sqlx::query(
            "INSERT INTO categories (id, name, parent_id, color, icon_key, display_order) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(c.id)
        .bind(pick(&c.names, lang))
        .bind(c.parent)
        .bind(c.color)
        .bind(c.icon)
        .bind(c.order)
        .execute(pool)
        .await?;
    }
    for (sku, names, category, barcode, cost, price, stock, min, unit) in DEMO_PRODUCTS {
        sqlx::query(
            "INSERT INTO products (sku, name, category_id, barcode, cost_price, selling_price, stock_quantity, min_stock_level, unit)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(sku)
        .bind(pick(&names, lang))
        .bind(category)
        .bind(barcode)
        .bind(cost)
        .bind(price)
        .bind(stock)
        .bind(min)
        .bind(unit)
        .execute(pool)
        .await?;
    }
    Ok(())
}
