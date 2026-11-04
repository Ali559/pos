-- Phase 1: processed media, shop profile, localized category names,
-- and default settings.

CREATE TABLE media_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT UNIQUE NOT NULL,
    path TEXT NOT NULL, -- relative to <app_data_dir>/media
    mime TEXT NOT NULL DEFAULT 'image/webp',
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    blurhash TEXT NOT NULL,
    sha256 TEXT NOT NULL, -- of the original upload, for dedupe
    original_name TEXT,
    created_by INTEGER,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (created_by) REFERENCES users(id)
);
CREATE INDEX idx_media_files_sha256 ON media_files(sha256);

CREATE TABLE shop_profile (
    id INTEGER PRIMARY KEY CHECK (id = 1), -- single row
    name_en TEXT NOT NULL,
    name_ku TEXT NOT NULL,
    name_ar TEXT,
    logo_media_id INTEGER,
    phone TEXT,
    phone_alt TEXT,
    address_en TEXT,
    address_ku TEXT,
    address_ar TEXT,
    tax_number TEXT,
    receipt_footer_en TEXT,
    receipt_footer_ku TEXT,
    receipt_footer_ar TEXT,
    currency TEXT NOT NULL DEFAULT 'IQD',
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (logo_media_id) REFERENCES media_files(id)
);

-- `name` stays as the English name
ALTER TABLE categories ADD COLUMN name_ku TEXT;
ALTER TABLE categories ADD COLUMN name_ar TEXT;

INSERT INTO settings (key, value, description) VALUES
    ('language', 'ku', 'UI language: ku, ar or en'),
    ('tax_rate', '0', 'Sales tax percentage applied at the register');
