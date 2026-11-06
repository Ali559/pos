-- Phase 9: customers and debt (قەرز).
-- A customer's balance is the sum of their ledger: positive = they owe the
-- shop. A sale on account writes a 'credit' payment (so the sale's payments
-- still add up to its total) and a ledger row; a repayment writes a ledger
-- row and a payment with no sale (so cash repayments land in the drawer).

ALTER TABLE customers ADD COLUMN credit_limit REAL; -- NULL: no limit

CREATE TABLE customer_ledger (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('sale', 'payment', 'refund', 'adjustment')),
    amount REAL NOT NULL,      -- + owes more, − owes less
    sale_id INTEGER,
    return_id INTEGER,
    method TEXT,               -- repayments: cash | card
    note TEXT,
    user_id INTEGER NOT NULL,
    shift_id INTEGER,
    created_at TEXT NOT NULL,
    FOREIGN KEY (customer_id) REFERENCES customers(id),
    FOREIGN KEY (sale_id) REFERENCES sales(id),
    FOREIGN KEY (return_id) REFERENCES returns(id),
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (shift_id) REFERENCES shifts(id)
);
CREATE INDEX idx_customer_ledger_customer ON customer_ledger(customer_id, created_at);

-- A repayment's money: no sale, but it's cash in the drawer
ALTER TABLE payments ADD COLUMN ledger_id INTEGER REFERENCES customer_ledger(id);

INSERT INTO permissions (key, group_key, is_dangerous, sort) VALUES
    ('customers.view', 'customers', 0, 70),
    ('customers.edit', 'customers', 0, 71),
    ('customers.credit', 'customers', 0, 72),
    ('customers.over_limit', 'customers', 1, 73);
-- Manager: everything; Cashier: look up customers and sell on account
INSERT INTO role_permissions (role_id, permission_key)
SELECT r.id, p.key FROM roles r, permissions p
WHERE (r.id = 2 AND p.group_key = 'customers')
   OR (r.id = 3 AND p.key IN ('customers.view', 'customers.credit'));
