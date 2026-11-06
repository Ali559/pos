-- Phase 11: pricing and promotions.

-- Wholesale: a second price per product (NULL = same as retail), used when
-- the sale is at the wholesale level; a customer can default to it.
ALTER TABLE products ADD COLUMN wholesale_price REAL;
ALTER TABLE customers ADD COLUMN price_level TEXT NOT NULL DEFAULT 'retail' CHECK (price_level IN ('retail', 'wholesale'));
ALTER TABLE sales ADD COLUMN price_level TEXT NOT NULL DEFAULT 'retail';

-- Cheaper each from a quantity (3+ at 700 instead of 750). Retail only.
CREATE TABLE product_qty_prices (
    product_id INTEGER NOT NULL,
    min_qty REAL NOT NULL CHECK (min_qty > 1),
    price REAL NOT NULL CHECK (price >= 0),  -- each
    PRIMARY KEY (product_id, min_qty),
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE
);

-- No tax on this product (basic food, medicine…)
ALTER TABLE products ADD COLUMN tax_exempt INTEGER NOT NULL DEFAULT 0;

-- Promotions, applied automatically at checkout to retail sales:
--   percent  — `percent` off
--   multibuy — buy `buy_qty`, get `free_qty` free
--   bundle   — `buy_qty` for `bundle_price`
CREATE TABLE promotions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('percent', 'multibuy', 'bundle')),
    percent REAL,
    buy_qty REAL,
    free_qty REAL,
    bundle_price REAL,
    starts_on TEXT,            -- YYYY-MM-DD, local; NULL = already running
    ends_on TEXT,              -- last day, inclusive; NULL = no end
    is_active INTEGER NOT NULL DEFAULT 1,
    created_by INTEGER,
    created_at TEXT NOT NULL,
    FOREIGN KEY (created_by) REFERENCES users(id)
);
-- What a promotion covers: products and/or whole categories
CREATE TABLE promotion_targets (
    promotion_id INTEGER NOT NULL,
    product_id INTEGER,
    category_id INTEGER,
    FOREIGN KEY (promotion_id) REFERENCES promotions(id) ON DELETE CASCADE,
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE,
    FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE CASCADE,
    CHECK ((product_id IS NULL) <> (category_id IS NULL))
);
CREATE INDEX idx_promotion_targets ON promotion_targets(promotion_id);

-- The promotion a sale line got, as a snapshot. Its amount is part of the
-- line's discount_amount (so returns and reports see it) and kept apart in
-- promo_discount.
ALTER TABLE sale_items ADD COLUMN promotion_id INTEGER REFERENCES promotions(id);
ALTER TABLE sale_items ADD COLUMN promotion_name TEXT;
ALTER TABLE sale_items ADD COLUMN promo_discount REAL NOT NULL DEFAULT 0;

-- Every change to a product's prices or cost
CREATE TABLE price_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL,
    field TEXT NOT NULL CHECK (field IN ('price', 'wholesale', 'cost')),
    old_value REAL,
    new_value REAL,
    source TEXT NOT NULL,      -- edit | bulk | delivery
    user_id INTEGER,
    changed_at TEXT NOT NULL,
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE INDEX idx_price_history_product ON price_history(product_id, changed_at);

INSERT INTO permissions (key, group_key, is_dangerous, sort) VALUES
    ('sales.wholesale', 'sales', 0, 16),
    ('catalog.promotions', 'inventory', 0, 25),
    ('catalog.bulk_prices', 'inventory', 1, 26);
INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, p.key FROM roles r, permissions p
WHERE r.id = 2 AND p.key IN ('sales.wholesale', 'catalog.promotions', 'catalog.bulk_prices');
