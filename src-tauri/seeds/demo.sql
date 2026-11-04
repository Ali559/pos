-- Sample catalog for development. Loaded by the `seed_demo_data` command
-- (debug builds only) and automatically by the browser dev fallback.
-- Prices are in IQD.

INSERT INTO categories (id, name, name_ku, name_ar, color, display_order) VALUES
    (1, 'Beverages', 'خواردنەوە', 'مشروبات', '#2d5a8f', 1),
    (2, 'Snacks', 'سووکەخۆراک', 'وجبات خفيفة', '#94600c', 2),
    (3, 'Dairy', 'شیرەمەنی', 'ألبان', '#5f4996', 3),
    (4, 'Bakery', 'نانەوایی', 'مخبوزات', '#9a3f26', 4),
    (5, 'Produce', 'میوە و سەوزە', 'خضار وفواكه', '#2f6b38', 5);

INSERT INTO products (sku, name, category_id, barcode, cost_price, selling_price, stock_quantity, min_stock_level, unit) VALUES
    ('BEV-001', 'Coca-Cola 330ml', 1, '5449000000996', 500, 750, 142, 24, 'piece'),
    ('BEV-002', 'Mineral water 1.5L', 1, '6281006432108', 300, 500, 88, 24, 'piece'),
    ('BEV-003', 'Ayran 250ml', 1, '8690526012345', 500, 750, 9, 12, 'piece'),
    ('BEV-004', 'Black tea 500g', 1, '8690000123401', 4500, 6000, 31, 6, 'piece'),
    ('SNK-001', 'Potato chips salted', 2, '6281036111110', 650, 1000, 57, 12, 'piece'),
    ('SNK-002', 'Roasted pistachios 200g', 2, '6260001234567', 3750, 5000, 4, 8, 'piece'),
    ('SNK-003', 'Dark chocolate bar', 2, '7622300489434', 1400, 2000, 40, 10, 'piece'),
    ('DRY-001', 'Fresh milk 1L', 3, '6281007012345', 1250, 1750, 26, 12, 'piece'),
    ('DRY-002', 'Yogurt 500g', 3, '6281007054321', 1000, 1500, 0, 10, 'piece'),
    ('DRY-003', 'White cheese 400g', 3, '6281007099887', 3500, 4500, 18, 6, 'piece'),
    ('BAK-001', 'Samoon bread (5)', 4, '2000000000011', 600, 1000, 60, 20, 'piece'),
    ('BAK-002', 'Klicha date cookies', 4, '2000000000028', 2500, 3500, 14, 6, 'piece'),
    ('PRD-001', 'Tomatoes 1kg', 5, '2000000000035', 900, 1250, 35, 10, 'kg'),
    ('PRD-002', 'Cucumbers 1kg', 5, '2000000000042', 700, 1000, 7, 10, 'kg'),
    ('PRD-003', 'Pomegranates 1kg', 5, '2000000000059', 2200, 3000, 22, 8, 'kg');
