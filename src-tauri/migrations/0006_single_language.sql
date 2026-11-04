-- One language per shop, chosen at setup and then fixed. Every name has a
-- single column in that language instead of en/ku/ar variants. Existing
-- values keep the one for the current UI language, falling back to English.

CREATE TEMP TABLE lang AS SELECT COALESCE((SELECT value FROM settings WHERE key = 'language'), 'ku') AS code;

-- Shop profile: rebuilt with single-language fields and the shop type
CREATE TABLE shop_profile_new (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    name TEXT NOT NULL,
    shop_type TEXT NOT NULL DEFAULT 'mini_market', -- mini_market | supermarket | other
    logo_media_id INTEGER,
    phone TEXT,
    phone_alt TEXT,
    address TEXT,
    tax_number TEXT,
    receipt_footer TEXT,
    currency TEXT NOT NULL DEFAULT 'IQD',
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (logo_media_id) REFERENCES media_files(id)
);
INSERT INTO shop_profile_new (id, name, logo_media_id, phone, phone_alt, address, tax_number, receipt_footer, currency, updated_at)
SELECT id,
    CASE (SELECT code FROM lang) WHEN 'ku' THEN COALESCE(NULLIF(name_ku, ''), name_en)
        WHEN 'ar' THEN COALESCE(NULLIF(name_ar, ''), name_en) ELSE name_en END,
    logo_media_id, phone, phone_alt,
    CASE (SELECT code FROM lang) WHEN 'ku' THEN COALESCE(NULLIF(address_ku, ''), address_en)
        WHEN 'ar' THEN COALESCE(NULLIF(address_ar, ''), address_en) ELSE address_en END,
    tax_number,
    CASE (SELECT code FROM lang) WHEN 'ku' THEN COALESCE(NULLIF(receipt_footer_ku, ''), receipt_footer_en)
        WHEN 'ar' THEN COALESCE(NULLIF(receipt_footer_ar, ''), receipt_footer_en) ELSE receipt_footer_en END,
    currency, updated_at
FROM shop_profile;
DROP TABLE shop_profile;
ALTER TABLE shop_profile_new RENAME TO shop_profile;

-- Categories and products: `name` was English; take the chosen language's
UPDATE categories SET name = CASE (SELECT code FROM lang)
    WHEN 'ku' THEN COALESCE(NULLIF(name_ku, ''), name)
    WHEN 'ar' THEN COALESCE(NULLIF(name_ar, ''), name) ELSE name END;
ALTER TABLE categories DROP COLUMN name_ku;
ALTER TABLE categories DROP COLUMN name_ar;

UPDATE products SET name = CASE (SELECT code FROM lang)
    WHEN 'ku' THEN COALESCE(NULLIF(name_ku, ''), name)
    WHEN 'ar' THEN COALESCE(NULLIF(name_ar, ''), name) ELSE name END;
ALTER TABLE products DROP COLUMN name_ku;
ALTER TABLE products DROP COLUMN name_ar;

-- Roles and expense types had name_en/name_ku/name_ar
ALTER TABLE roles ADD COLUMN name TEXT NOT NULL DEFAULT '';
UPDATE roles SET name = CASE (SELECT code FROM lang)
    WHEN 'ku' THEN name_ku WHEN 'ar' THEN COALESCE(NULLIF(name_ar, ''), name_en) ELSE name_en END;
ALTER TABLE roles DROP COLUMN name_en;
ALTER TABLE roles DROP COLUMN name_ku;
ALTER TABLE roles DROP COLUMN name_ar;

ALTER TABLE expense_types ADD COLUMN name TEXT NOT NULL DEFAULT '';
UPDATE expense_types SET name = CASE (SELECT code FROM lang)
    WHEN 'ku' THEN name_ku WHEN 'ar' THEN COALESCE(NULLIF(name_ar, ''), name_en) ELSE name_en END;
ALTER TABLE expense_types DROP COLUMN name_en;
ALTER TABLE expense_types DROP COLUMN name_ku;
ALTER TABLE expense_types DROP COLUMN name_ar;

DROP TABLE lang;

-- Settings for this phase
INSERT INTO settings (key, value, description) VALUES
    ('barcode_lookup', '1', 'Look up unknown barcodes online (Open Food Facts) to fill the product form'),
    ('receipt_width', '80', 'Receipt paper width in mm: 58 or 80'),
    ('auto_print', '0', 'Print a receipt after every sale'),
    ('opening_float', '0', 'Default opening cash in the drawer');
