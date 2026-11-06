-- Phase 8: selling units.
--  * Pack sizes: one product sold as a single, a 6-pack or a carton, each
--    with its own barcode and price. Stock stays in the product's own unit;
--    a carton sale takes `factor` of them.
--  * Scale barcodes: weighed products get a PLU, the item code a scale
--    prints inside its EAN-13 labels (settings scale_*).
--  * Open-price products (the misc item): the price is typed at the till.
--  * Products that don't track stock (the misc item, services).

CREATE TABLE product_units (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL,
    name TEXT NOT NULL,                      -- "Carton", "6-pack"
    factor REAL NOT NULL CHECK (factor > 1), -- how many of the product's own unit
    barcode TEXT UNIQUE,
    price REAL NOT NULL CHECK (price >= 0),  -- for the whole pack
    position INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE
);
CREATE INDEX idx_product_units_product ON product_units(product_id);

-- What was sold, as a snapshot (the pack can be renamed or deleted later).
-- `quantity` stays in the product's own unit so stock, batches, returns and
-- reports keep working; `unit_price` is per that unit.
ALTER TABLE sale_items ADD COLUMN unit_name TEXT;   -- NULL: sold in the product's own unit
ALTER TABLE sale_items ADD COLUMN unit_factor REAL;

ALTER TABLE products ADD COLUMN plu TEXT;           -- item code inside scale barcodes
CREATE UNIQUE INDEX idx_products_plu ON products(plu) WHERE plu IS NOT NULL;
ALTER TABLE products ADD COLUMN open_price INTEGER NOT NULL DEFAULT 0;
ALTER TABLE products ADD COLUMN track_stock INTEGER NOT NULL DEFAULT 1;

-- Stock only moves for products that track it
DROP TRIGGER after_sale_item_insert;
CREATE TRIGGER after_sale_item_insert
AFTER INSERT ON sale_items
WHEN (SELECT track_stock FROM products WHERE id = NEW.product_id) = 1
BEGIN
    UPDATE products
    SET stock_quantity = stock_quantity - NEW.quantity
    WHERE id = NEW.product_id;

    INSERT INTO stock_adjustments (product_id, adjustment_type, quantity, user_id, reference_id)
    VALUES (NEW.product_id, 'sale', -NEW.quantity,
            (SELECT user_id FROM sales WHERE id = NEW.sale_id),
            NEW.sale_id);
END;

INSERT INTO permissions (key, group_key, is_dangerous, sort) VALUES ('sales.open_price', 'sales', 0, 15);
-- Manager and Cashier, if the shop still has them
INSERT INTO role_permissions (role_id, permission_key)
SELECT id, 'sales.open_price' FROM roles WHERE id IN (2, 3);

-- Common EAN-13 scale label: 20 + 5-digit PLU + 5-digit weight in grams + check
INSERT OR IGNORE INTO settings (key, value, description) VALUES
    ('scale_prefix', '20', 'Scale barcodes start with this (blank: off)'),
    ('scale_code_length', '5', 'Digits of the item code (PLU) after the prefix'),
    ('scale_mode', 'weight', 'weight | price: what the rest of the barcode holds'),
    ('scale_decimals', '3', 'Decimal places of the weight (kg) or price');
