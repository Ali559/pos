-- US dollars next to dinars: the market rate, dollars taken at the till,
-- dollars in the drawer, and suppliers who sell in dollars. Prices, sales
-- and customer debt stay in the shop's currency (IQD).

-- Every rate the shop has used, newest last. Rates are IQD per 100 USD,
-- the way the market quotes them.
CREATE TABLE exchange_rates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    per_100 REAL NOT NULL CHECK (per_100 > 0),
    source TEXT NOT NULL,          -- manual | telegram:<channel>
    city TEXT,                     -- the market a fetched rate is from
    posted_at TEXT,                -- when the source posted it
    user_id INTEGER NOT NULL,      -- who set or accepted it
    created_at TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- A dollar payment (payment_method 'usd'): `amount` is the part of the sale
-- it paid, in IQD; `foreign_amount` the dollars handed over; `rate` the IQD
-- per 100 USD used; `change_given` the dinars handed back. Paying a supplier
-- dollars from the till is a negative 'usd' row.
ALTER TABLE payments ADD COLUMN foreign_amount REAL;
ALTER TABLE payments ADD COLUMN rate REAL;

-- The drawer's dollars, counted like its dinars
ALTER TABLE cash_drawers ADD COLUMN opening_usd REAL;
ALTER TABLE cash_drawers ADD COLUMN closing_usd REAL;
ALTER TABLE cash_drawers ADD COLUMN expected_usd REAL;
ALTER TABLE cash_drawers ADD COLUMN difference_usd REAL;

-- Suppliers who sell in dollars: their deliveries and account are in USD.
-- Each ledger row says its currency; a supplier's balance sums the rows in
-- the supplier's currency (which can only change while nothing is owed).
ALTER TABLE suppliers ADD COLUMN currency TEXT NOT NULL DEFAULT 'IQD';
ALTER TABLE supplier_ledger ADD COLUMN currency TEXT NOT NULL DEFAULT 'IQD';
ALTER TABLE supplier_ledger ADD COLUMN rate REAL;
-- Dollar deliveries: total in USD, the rate used; each line's cost is kept
-- in IQD (what margins are worked out with) and in USD as invoiced
ALTER TABLE deliveries ADD COLUMN currency TEXT NOT NULL DEFAULT 'IQD';
ALTER TABLE deliveries ADD COLUMN rate REAL;
ALTER TABLE delivery_items ADD COLUMN cost_foreign REAL;

-- Setting and accepting the rate
INSERT INTO permissions (key, group_key, is_dangerous, sort) VALUES
    ('settings.rates', 'settings', 0, 63);
INSERT INTO role_permissions (role_id, permission_key) VALUES (2, 'settings.rates');
