-- Phase 4: sales tied to shifts, stock batches with expiry, notifications,
-- expenses. Times written by Rust are RFC 3339 UTC; dates are YYYY-MM-DD.

ALTER TABLE sales ADD COLUMN shift_id INTEGER REFERENCES shifts(id);
CREATE INDEX idx_sales_shift ON sales(shift_id);
CREATE INDEX idx_payments_sale ON payments(sale_id);

-- Stock arrives in batches; sales consume the soonest-expiring first.
-- Stock that predates batches (or was never batched) is simply untracked
-- for expiry: products.stock_quantity stays the source of truth for counts.
CREATE TABLE product_batches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL,
    quantity_received REAL NOT NULL,
    quantity_remaining REAL NOT NULL,
    expiry_date TEXT, -- YYYY-MM-DD, local
    cost_price REAL,
    note TEXT,
    received_at TEXT NOT NULL,
    received_by INTEGER,
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE,
    FOREIGN KEY (received_by) REFERENCES users(id)
);
CREATE INDEX idx_batches_product_expiry ON product_batches(product_id, expiry_date);

-- One open row per condition (kind + product + batch). `dismissed_at` hides
-- it while the condition lasts; `resolved_at` closes it when it no longer
-- holds, so a recurrence makes a fresh row.
CREATE TABLE notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL CHECK (kind IN ('stock_low', 'stock_out', 'expiring', 'expired', 'expense_due')),
    product_id INTEGER,
    batch_id INTEGER,
    expense_id INTEGER,
    payload TEXT, -- JSON for display
    created_at TEXT NOT NULL,
    read_at TEXT,
    dismissed_at TEXT,
    resolved_at TEXT,
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE,
    FOREIGN KEY (batch_id) REFERENCES product_batches(id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX idx_notifications_open
    ON notifications(kind, COALESCE(product_id, 0), COALESCE(batch_id, 0), COALESCE(expense_id, 0))
    WHERE resolved_at IS NULL;

CREATE TABLE expense_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name_en TEXT NOT NULL,
    name_ku TEXT NOT NULL,
    name_ar TEXT,
    icon_key TEXT NOT NULL DEFAULT 'tabler:receipt',
    color TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    display_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE expenses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    type_id INTEGER NOT NULL,
    amount REAL NOT NULL CHECK (amount >= 0),
    spent_on TEXT NOT NULL, -- YYYY-MM-DD, local
    note TEXT,
    receipt_media_id INTEGER,
    -- A recurring expense is a template: each period it produces a draft
    -- (status 'draft', recurring_parent_id set) for someone to confirm
    recurring TEXT CHECK (recurring IN ('weekly', 'monthly')),
    recurring_parent_id INTEGER,
    status TEXT NOT NULL DEFAULT 'confirmed' CHECK (status IN ('confirmed', 'draft')),
    created_by INTEGER,
    created_at TEXT NOT NULL,
    FOREIGN KEY (type_id) REFERENCES expense_types(id),
    FOREIGN KEY (receipt_media_id) REFERENCES media_files(id),
    FOREIGN KEY (recurring_parent_id) REFERENCES expenses(id) ON DELETE SET NULL,
    FOREIGN KEY (created_by) REFERENCES users(id)
);
CREATE INDEX idx_expenses_spent ON expenses(spent_on);
CREATE UNIQUE INDEX idx_expenses_one_per_period ON expenses(recurring_parent_id, spent_on)
    WHERE recurring_parent_id IS NOT NULL;

INSERT INTO expense_types (name_en, name_ku, name_ar, icon_key, color, display_order) VALUES
    ('Rent', 'کرێ', 'إيجار', 'tabler:building-store', '#5f4996', 1),
    ('Electricity', 'کارەبا', 'كهرباء', 'tabler:bolt', '#94600c', 2),
    ('Generator', 'مۆلیدە', 'مولدة', 'tabler:engine', '#9a3f26', 3),
    ('Salaries', 'مووچە', 'رواتب', 'tabler:users', '#2f6b38', 4),
    ('Supplies', 'پێداویستی', 'لوازم', 'tabler:package', '#2d5a8f', 5),
    ('Other', 'هیتر', 'أخرى', 'tabler:receipt', '#57534e', 6);

INSERT INTO settings (key, value, description) VALUES
    ('expiry_warn_days', '7', 'Warn this many days before a batch expires'),
    ('os_notifications', '1', 'Also show out-of-stock and expired alerts as system notifications');
