-- Phase 3: category icons and two-level tree, product translations, icons
-- and images.

-- Rebuild categories: names are unique per parent instead of globally
-- ("Other" under two categories is normal), and every category has an icon.
CREATE TABLE categories_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL, -- English
    name_ku TEXT,
    name_ar TEXT,
    description TEXT,
    parent_id INTEGER,
    icon_key TEXT NOT NULL DEFAULT 'tabler:category', -- "<set>:<icon>"
    image_media_id INTEGER,
    display_order INTEGER NOT NULL DEFAULT 0,
    color TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_id) REFERENCES categories(id),
    FOREIGN KEY (image_media_id) REFERENCES media_files(id)
);

INSERT INTO categories_new (id, name, name_ku, name_ar, description, parent_id,
    display_order, color, is_active, created_at)
SELECT id, name, name_ku, name_ar, description, parent_id,
    COALESCE(display_order, 0), color, COALESCE(is_active, 1), created_at
FROM categories;

DROP TABLE categories;
ALTER TABLE categories_new RENAME TO categories;

CREATE UNIQUE INDEX idx_categories_name_per_parent
    ON categories(COALESCE(parent_id, 0), name COLLATE NOCASE);
CREATE INDEX idx_categories_parent ON categories(parent_id, display_order);

-- Products: translated names and an optional icon
ALTER TABLE products ADD COLUMN name_ku TEXT;
ALTER TABLE products ADD COLUMN name_ar TEXT;
ALTER TABLE products ADD COLUMN icon_key TEXT;

CREATE TABLE product_media (
    product_id INTEGER NOT NULL,
    media_id INTEGER NOT NULL,
    position INTEGER NOT NULL DEFAULT 0, -- 0 = cover
    PRIMARY KEY (product_id, media_id),
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE,
    FOREIGN KEY (media_id) REFERENCES media_files(id)
);
CREATE INDEX idx_product_media_order ON product_media(product_id, position);
