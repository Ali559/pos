-- Phase 14: activity log, loyalty points, card payments, extra tills.

-- Activity log (Settings → Activity): who may read it, and which till an
-- entry came from when several tills share this database.
INSERT INTO permissions (key, group_key, is_dangerous, sort) VALUES
    ('settings.audit', 'settings', 0, 62);
INSERT INTO role_permissions (role_id, permission_key) VALUES (2, 'settings.audit');
ALTER TABLE audit_log ADD COLUMN till_id INTEGER;
CREATE INDEX IF NOT EXISTS idx_audit_log_created ON audit_log(created_at);

-- Card payments are off for new shops (most tills here take cash). A shop
-- that has already taken cards keeps them.
INSERT OR IGNORE INTO settings (key, value)
SELECT 'card_payments', '1' WHERE EXISTS (SELECT 1 FROM payments WHERE payment_method = 'card');

-- Loyalty points. customers.loyalty_points is the balance; this ledger is
-- every change to it, only ever added to.
CREATE TABLE loyalty_ledger (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('earn', 'redeem', 'refund', 'adjustment')),
    points INTEGER NOT NULL,   -- + gained, − spent or taken back
    sale_id INTEGER,
    return_id INTEGER,
    note TEXT,
    user_id INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (customer_id) REFERENCES customers(id),
    FOREIGN KEY (sale_id) REFERENCES sales(id),
    FOREIGN KEY (return_id) REFERENCES returns(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE INDEX idx_loyalty_ledger_customer ON loyalty_ledger(customer_id, created_at);
UPDATE customers SET loyalty_points = 0 WHERE loyalty_points IS NULL;

-- Extra tills on the shop's network. Only the main computer has rows here:
-- each paired till and a hash of its secret.
CREATE TABLE tills (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    paired_at TEXT NOT NULL,
    last_seen_at TEXT,
    app_version TEXT,
    revoked_at TEXT
);
ALTER TABLE sales ADD COLUMN till_id INTEGER;
