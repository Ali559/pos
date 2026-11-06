-- Phase 10: suppliers, deliveries, what the shop owes suppliers, stock counts.

ALTER TABLE suppliers ADD COLUMN notes TEXT;
CREATE UNIQUE INDEX idx_product_suppliers_pair ON product_suppliers(product_id, supplier_id);

-- One delivery from a supplier (or none: stock bought at the market).
-- Every line becomes a stock batch.
CREATE TABLE deliveries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    supplier_id INTEGER,
    invoice_number TEXT,
    invoice_media_id INTEGER,
    total REAL NOT NULL,          -- what the goods cost
    note TEXT,
    user_id INTEGER NOT NULL,
    shift_id INTEGER,
    received_at TEXT NOT NULL,
    FOREIGN KEY (supplier_id) REFERENCES suppliers(id),
    FOREIGN KEY (invoice_media_id) REFERENCES media_files(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE INDEX idx_deliveries_supplier ON deliveries(supplier_id, received_at);

CREATE TABLE delivery_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    delivery_id INTEGER NOT NULL,
    product_id INTEGER NOT NULL,
    batch_id INTEGER,
    quantity REAL NOT NULL,       -- in the product's own unit
    cost REAL NOT NULL,           -- per product unit
    unit_name TEXT,               -- received by the pack: its name and size
    unit_factor REAL,
    expiry_date TEXT,
    FOREIGN KEY (delivery_id) REFERENCES deliveries(id) ON DELETE CASCADE,
    FOREIGN KEY (product_id) REFERENCES products(id),
    FOREIGN KEY (batch_id) REFERENCES product_batches(id)
);
CREATE INDEX idx_delivery_items_product ON delivery_items(product_id);

-- What the shop owes each supplier: + owes more (a delivery not paid in
-- full), − owes less (a payment).
CREATE TABLE supplier_ledger (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    supplier_id INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('delivery', 'payment', 'adjustment')),
    amount REAL NOT NULL,
    delivery_id INTEGER,
    method TEXT,                  -- payments: till | cash | bank
    note TEXT,
    user_id INTEGER NOT NULL,
    shift_id INTEGER,
    created_at TEXT NOT NULL,
    FOREIGN KEY (supplier_id) REFERENCES suppliers(id),
    FOREIGN KEY (delivery_id) REFERENCES deliveries(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE INDEX idx_supplier_ledger_supplier ON supplier_ledger(supplier_id, created_at);

-- Cash paid to a supplier out of the till: a negative cash payment, so the
-- drawer's expected cash goes down
ALTER TABLE payments ADD COLUMN supplier_ledger_id INTEGER REFERENCES supplier_ledger(id);

-- A stock count: what's on the shelf vs the system. `expected` is the
-- system's count when the item was counted, so sales during the count
-- don't distort the difference.
CREATE TABLE stock_counts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT,
    category_id INTEGER,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'applied', 'cancelled')),
    user_id INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    finished_by INTEGER,
    FOREIGN KEY (category_id) REFERENCES categories(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE TABLE stock_count_items (
    count_id INTEGER NOT NULL,
    product_id INTEGER NOT NULL,
    expected REAL NOT NULL,
    counted REAL NOT NULL,
    counted_at TEXT NOT NULL,
    PRIMARY KEY (count_id, product_id),
    FOREIGN KEY (count_id) REFERENCES stock_counts(id) ON DELETE CASCADE,
    FOREIGN KEY (product_id) REFERENCES products(id)
);

INSERT INTO permissions (key, group_key, is_dangerous, sort) VALUES
    ('suppliers.view', 'suppliers', 0, 80),
    ('suppliers.edit', 'suppliers', 0, 81),
    ('suppliers.pay', 'suppliers', 1, 82),
    ('inventory.count', 'inventory', 0, 24);
-- Manager: all of it; Cashier: may count shelves
INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, p.key FROM roles r, permissions p
WHERE (r.id = 2 AND (p.group_key = 'suppliers' OR p.key = 'inventory.count'))
   OR (r.id = 3 AND p.key = 'inventory.count');
