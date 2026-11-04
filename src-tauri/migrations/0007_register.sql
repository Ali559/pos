-- Register: discounts, cash tendered, drawer counts per shift, returns.

-- What the customer handed over and got back (cash only). Refunds are
-- negative payment rows on the original sale, tied to their return.
ALTER TABLE payments ADD COLUMN tendered REAL;
ALTER TABLE payments ADD COLUMN change_given REAL;
ALTER TABLE payments ADD COLUMN shift_id INTEGER REFERENCES shifts(id);
ALTER TABLE payments ADD COLUMN return_id INTEGER REFERENCES returns(id);
CREATE INDEX idx_payments_shift ON payments(shift_id);
CREATE INDEX idx_payments_date ON payments(payment_date);
UPDATE payments SET shift_id = (SELECT shift_id FROM sales WHERE sales.id = payments.sale_id);

-- Who approved a discount the cashier couldn't give themselves
ALTER TABLE sales ADD COLUMN approved_by INTEGER REFERENCES users(id);

ALTER TABLE returns ADD COLUMN shift_id INTEGER REFERENCES shifts(id);
ALTER TABLE returns ADD COLUMN refund_method TEXT NOT NULL DEFAULT 'cash';
ALTER TABLE returns ADD COLUMN approved_by INTEGER REFERENCES users(id);
CREATE INDEX idx_returns_sale ON returns(original_sale_id);
CREATE INDEX idx_returns_date ON returns(return_date);
-- Before tax, for reports; refund_amount includes tax
ALTER TABLE return_items ADD COLUMN subtotal REAL NOT NULL DEFAULT 0;
ALTER TABLE return_items ADD COLUMN restocked INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_return_items_line ON return_items(sale_item_id);

ALTER TABLE cash_drawers ADD COLUMN shift_id INTEGER REFERENCES shifts(id);

CREATE INDEX IF NOT EXISTS idx_sales_receipt ON sales(receipt_number);
