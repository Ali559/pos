-- Phase 13: till comfort.
-- Quick keys: products pinned to the front of the register, in this order
-- (NULL = not pinned).
ALTER TABLE products ADD COLUMN quick_key INTEGER;
CREATE INDEX idx_products_quick_key ON products(quick_key) WHERE quick_key IS NOT NULL;
