-- Phase 2: roles and permissions, employees, shifts.

CREATE TABLE permissions (
    key TEXT PRIMARY KEY,
    group_key TEXT NOT NULL,
    is_dangerous INTEGER NOT NULL DEFAULT 0, -- flagged in the role editor
    sort INTEGER NOT NULL DEFAULT 0
);

INSERT INTO permissions (key, group_key, is_dangerous, sort) VALUES
    ('sales.sell', 'sales', 0, 10),
    ('sales.discount', 'sales', 0, 11),
    ('sales.void_line', 'sales', 0, 12),
    ('sales.refund', 'sales', 1, 13),
    ('sales.reprint', 'sales', 0, 14),
    ('inventory.view', 'inventory', 0, 20),
    ('inventory.edit', 'inventory', 0, 21),
    ('inventory.adjust_stock', 'inventory', 1, 22),
    ('catalog.categories', 'inventory', 0, 23),
    ('staff.view', 'staff', 0, 30),
    ('staff.edit', 'staff', 1, 31),
    ('staff.shifts', 'staff', 0, 32),
    ('roles.manage', 'staff', 1, 33),
    ('expenses.view', 'expenses', 0, 40),
    ('expenses.edit', 'expenses', 0, 41),
    ('analytics.view', 'analytics', 0, 50),
    ('analytics.export', 'analytics', 0, 51),
    ('settings.shop', 'settings', 0, 60),
    ('settings.system', 'settings', 1, 61);

CREATE TABLE roles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name_en TEXT NOT NULL,
    name_ku TEXT NOT NULL,
    name_ar TEXT,
    description TEXT,
    is_system INTEGER NOT NULL DEFAULT 0, -- Owner: every permission, can't be edited or deleted
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE role_permissions (
    role_id INTEGER NOT NULL,
    permission_key TEXT NOT NULL,
    PRIMARY KEY (role_id, permission_key),
    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
    FOREIGN KEY (permission_key) REFERENCES permissions(key) ON DELETE CASCADE
);

INSERT INTO roles (id, name_en, name_ku, name_ar, description, is_system) VALUES
    (1, 'Owner', 'خاوەن', 'المالك', NULL, 1),
    (2, 'Manager', 'بەڕێوەبەر', 'المدير', NULL, 0),
    (3, 'Cashier', 'کاشێر', 'الكاشير', NULL, 0);

-- Owner's permissions are implicit (all of them), so it has no rows here
INSERT INTO role_permissions (role_id, permission_key)
SELECT 2, key FROM permissions
WHERE key NOT IN ('roles.manage', 'settings.system', 'staff.edit');

INSERT INTO role_permissions (role_id, permission_key) VALUES
    (3, 'sales.sell'),
    (3, 'sales.reprint'),
    (3, 'inventory.view');

-- Rebuild users: role text → role_id, add employee fields, hashed PIN.
-- No build before this one could create users, so this is a no-op copy in
-- practice; the mapping is here for safety.
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    pin_hash TEXT,
    full_name TEXT NOT NULL,
    role_id INTEGER NOT NULL,
    phone TEXT,
    photo_media_id INTEGER,
    hire_date TEXT,
    wage_amount REAL,
    wage_period TEXT CHECK (wage_period IN ('hour', 'month')),
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    last_login TEXT,
    FOREIGN KEY (role_id) REFERENCES roles(id),
    FOREIGN KEY (photo_media_id) REFERENCES media_files(id)
);

INSERT INTO users_new (id, username, password_hash, full_name, role_id, is_active, created_at, last_login)
SELECT id, username, password_hash, full_name,
       CASE role WHEN 'admin' THEN 1 WHEN 'manager' THEN 2 ELSE 3 END,
       is_active, created_at, last_login
FROM users;

DROP TABLE user_permissions; -- replaced by roles
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

-- Times in these tables are RFC 3339 UTC strings ("2026-11-04T15:00:00Z"),
-- written by Rust, so they sort and compare as text.
CREATE TABLE shift_schedules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    starts_at TEXT NOT NULL,
    ends_at TEXT NOT NULL,
    note TEXT,
    CHECK (ends_at > starts_at),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX idx_shift_schedules_user_start ON shift_schedules(user_id, starts_at);

CREATE TABLE shifts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    schedule_id INTEGER,
    cash_drawer_id INTEGER,
    clocked_in_at TEXT NOT NULL,
    clocked_out_at TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (schedule_id) REFERENCES shift_schedules(id) ON DELETE SET NULL,
    FOREIGN KEY (cash_drawer_id) REFERENCES cash_drawers(id)
);
CREATE INDEX idx_shifts_user_start ON shifts(user_id, clocked_in_at);
CREATE UNIQUE INDEX idx_shifts_one_open ON shifts(user_id) WHERE clocked_out_at IS NULL;
