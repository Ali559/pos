-- Phase 7: backups. Two new alert kinds; SQLite can't alter a CHECK, so the
-- table is rebuilt. Backup status and folder live in `settings`
-- (backup_dir, backup_auto, backup_keep, backup_last_ok_at,
-- backup_last_attempt_at, backup_last_error).

CREATE TABLE notifications_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL CHECK (kind IN ('stock_low', 'stock_out', 'expiring', 'expired', 'expense_due',
                                       'backup_failed', 'backup_overdue')),
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
INSERT INTO notifications_new (id, kind, product_id, batch_id, expense_id, payload, created_at, read_at, dismissed_at, resolved_at)
SELECT id, kind, product_id, batch_id, expense_id, payload, created_at, read_at, dismissed_at, resolved_at FROM notifications;
DROP TABLE notifications;
ALTER TABLE notifications_new RENAME TO notifications;

CREATE UNIQUE INDEX idx_notifications_open
    ON notifications(kind, COALESCE(product_id, 0), COALESCE(batch_id, 0), COALESCE(expense_id, 0))
    WHERE resolved_at IS NULL;
