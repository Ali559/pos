-- Returns without a receipt and exchanges (a return need not point at a
-- sale, nor a payment), opening cash counts, sign-in throttling.

CREATE TABLE returns_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    original_sale_id INTEGER, -- NULL: returned without a receipt
    return_date TEXT DEFAULT CURRENT_TIMESTAMP,
    user_id INTEGER NOT NULL,
    total_refund REAL NOT NULL,
    reason TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    shift_id INTEGER REFERENCES shifts(id),
    refund_method TEXT NOT NULL DEFAULT 'cash', -- cash | card | exchange
    approved_by INTEGER REFERENCES users(id),
    FOREIGN KEY (original_sale_id) REFERENCES sales(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);
INSERT INTO returns_new (id, original_sale_id, return_date, user_id, total_refund, reason, created_at, shift_id, refund_method, approved_by)
SELECT id, original_sale_id, return_date, user_id, total_refund, reason, created_at, shift_id, refund_method, approved_by FROM returns;

-- Lines name the product directly; sale_item_id only when there was a receipt
CREATE TABLE return_items_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    return_id INTEGER NOT NULL,
    sale_item_id INTEGER,
    product_id INTEGER NOT NULL,
    product_name TEXT NOT NULL,
    quantity REAL NOT NULL,
    cost_price REAL NOT NULL DEFAULT 0,
    subtotal REAL NOT NULL DEFAULT 0, -- before tax
    refund_amount REAL NOT NULL,      -- with tax
    restocked INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (return_id) REFERENCES returns(id) ON DELETE CASCADE,
    FOREIGN KEY (sale_item_id) REFERENCES sale_items(id),
    FOREIGN KEY (product_id) REFERENCES products(id)
);
INSERT INTO return_items_new (id, return_id, sale_item_id, product_id, product_name, quantity, cost_price, subtotal, refund_amount, restocked)
SELECT ri.id, ri.return_id, ri.sale_item_id, si.product_id, si.product_name, ri.quantity, si.cost_price, ri.subtotal, ri.refund_amount, ri.restocked
FROM return_items ri JOIN sale_items si ON si.id = ri.sale_item_id;

CREATE TABLE payments_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_id INTEGER, -- NULL for a refund without a receipt
    payment_method TEXT NOT NULL, -- cash | card | exchange
    amount REAL NOT NULL,
    reference_number TEXT,
    payment_date TEXT DEFAULT CURRENT_TIMESTAMP,
    tendered REAL,
    change_given REAL,
    shift_id INTEGER REFERENCES shifts(id),
    return_id INTEGER REFERENCES returns(id),
    FOREIGN KEY (sale_id) REFERENCES sales(id) ON DELETE CASCADE
);
INSERT INTO payments_new (id, sale_id, payment_method, amount, reference_number, payment_date, tendered, change_given, shift_id, return_id)
SELECT id, sale_id, payment_method, amount, reference_number, payment_date, tendered, change_given, shift_id, return_id FROM payments;

DROP TABLE payments;
DROP TABLE return_items;
DROP TABLE returns;
ALTER TABLE returns_new RENAME TO returns;
ALTER TABLE return_items_new RENAME TO return_items;
ALTER TABLE payments_new RENAME TO payments;

CREATE INDEX idx_returns_sale ON returns(original_sale_id);
CREATE INDEX idx_returns_date ON returns(return_date);
CREATE INDEX idx_return_items_line ON return_items(sale_item_id);
CREATE INDEX idx_return_items_return ON return_items(return_id);
CREATE INDEX idx_payments_sale ON payments(sale_id);
CREATE INDEX idx_payments_shift ON payments(shift_id);
CREATE INDEX idx_payments_date ON payments(payment_date);

-- The cashier counts the drawer when the shift starts
ALTER TABLE cash_drawers ADD COLUMN opening_counted_at TEXT;
UPDATE cash_drawers SET opening_counted_at = opened_at;

-- Failed sign-ins per username (or "pin"), for a short lockout
CREATE TABLE auth_throttle (
    key TEXT PRIMARY KEY,
    failures INTEGER NOT NULL DEFAULT 0,
    locked_until TEXT
);

INSERT INTO settings (key, value, description) VALUES
    ('block_expired', '0', 'Refuse to sell stock whose batch has expired'),
    ('receipt_printer', '', 'Empty: system print dialog; "system:<name>" or "tcp:<host:port>": straight to a receipt printer');
