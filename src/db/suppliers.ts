// Suppliers, deliveries, reorder suggestions and stock counts. Reads only;
// changes go through src-tauri/src/suppliers.rs and stockcount.rs.
import { select, selectOne } from "./client";

export interface Supplier {
  id: number;
  name: string;
  contact_person: string | null;
  phone: string | null;
  address: string | null;
  notes: string | null;
  is_active: number;
  /** What the shop owes them */
  owed: number;
  last_delivery: string | null;
  products: number;
}

const SUPPLIER_COLUMNS = `
  s.id, s.name, s.contact_person, s.phone, s.address, s.notes, s.is_active,
  COALESCE((SELECT SUM(amount) FROM supplier_ledger l WHERE l.supplier_id = s.id), 0) AS owed,
  (SELECT MAX(received_at) FROM deliveries d WHERE d.supplier_id = s.id) AS last_delivery,
  (SELECT COUNT(*) FROM product_suppliers ps WHERE ps.supplier_id = s.id) AS products`;

export const listSuppliers = (opts: { search?: string; includeInactive?: boolean } = {}) => {
  const where: string[] = opts.includeInactive ? [] : ["s.is_active = 1"];
  const params: unknown[] = [];
  if (opts.search?.trim()) {
    params.push(`%${opts.search.trim()}%`);
    where.push("(s.name LIKE $1 OR s.phone LIKE $1 OR s.contact_person LIKE $1)");
  }
  return select<Supplier>(
    `SELECT ${SUPPLIER_COLUMNS} FROM suppliers s ${where.length ? `WHERE ${where.join(" AND ")}` : ""} ORDER BY s.name COLLATE NOCASE`,
    params,
  );
};

export const getSupplier = (id: number) => selectOne<Supplier>(`SELECT ${SUPPLIER_COLUMNS} FROM suppliers s WHERE s.id = $1`, [id]);

export const owedToSuppliers = async () =>
  (await selectOne<{ total: number }>(
    `SELECT COALESCE(SUM(b), 0) AS total FROM (SELECT SUM(amount) AS b FROM supplier_ledger GROUP BY supplier_id HAVING SUM(amount) > 0.000001)`,
  ))?.total ?? 0;

export interface SupplierEntry {
  id: number;
  kind: "delivery" | "payment" | "adjustment";
  amount: number;
  delivery_id: number | null;
  method: string | null;
  note: string | null;
  user: string;
  created_at: string;
  running: number;
}

export const getSupplierLedger = (supplierId: number) =>
  select<SupplierEntry>(
    `SELECT l.id, l.kind, l.amount, l.delivery_id, l.method, l.note, u.full_name AS user, l.created_at,
            SUM(l.amount) OVER (ORDER BY l.created_at, l.id) AS running
     FROM supplier_ledger l JOIN users u ON u.id = l.user_id
     WHERE l.supplier_id = $1 ORDER BY l.created_at, l.id`,
    [supplierId],
  );

export interface DeliveryRow {
  id: number;
  supplier: string | null;
  invoice_number: string | null;
  total: number;
  items: number;
  user: string;
  received_at: string;
}

export const listDeliveries = (opts: { supplierId?: number | null; limit?: number } = {}) =>
  select<DeliveryRow>(
    `SELECT d.id, s.name AS supplier, d.invoice_number, d.total, u.full_name AS user, d.received_at,
            (SELECT COUNT(*) FROM delivery_items i WHERE i.delivery_id = d.id) AS items
     FROM deliveries d LEFT JOIN suppliers s ON s.id = d.supplier_id JOIN users u ON u.id = d.user_id
     ${opts.supplierId ? "WHERE d.supplier_id = $1" : ""}
     ORDER BY d.received_at DESC, d.id DESC LIMIT ${Math.trunc(opts.limit ?? 200)}`,
    opts.supplierId ? [opts.supplierId] : [],
  );

export interface DeliveryItem {
  id: number;
  name: string;
  unit: string;
  quantity: number;
  cost: number;
  unit_name: string | null;
  unit_factor: number | null;
  expiry_date: string | null;
}

export const getDeliveryItems = (deliveryId: number) =>
  select<DeliveryItem>(
    `SELECT i.id, p.name, p.unit, i.quantity, i.cost, i.unit_name, i.unit_factor, i.expiry_date
     FROM delivery_items i JOIN products p ON p.id = i.product_id WHERE i.delivery_id = $1 ORDER BY i.id`,
    [deliveryId],
  );

/** Something to reorder: at or under its reorder level */
export interface ReorderRow {
  id: number;
  name: string;
  unit: string;
  stock: number;
  min_stock: number;
  /** Sold per day over the last 28 days */
  per_day: number;
  cost: number;
  supplier_id: number | null;
  supplier: string | null;
  supplier_phone: string | null;
  /** The largest pack, to order in whole packs */
  pack_name: string | null;
  pack_factor: number | null;
}

export const reorderList = () =>
  select<ReorderRow>(
    `SELECT p.id, p.name, p.unit, p.stock_quantity AS stock, p.min_stock_level AS min_stock, p.cost_price AS cost,
            COALESCE((SELECT SUM(si.quantity) FROM sale_items si JOIN sales s ON s.id = si.sale_id
                      WHERE si.product_id = p.id AND s.sale_date >= datetime('now', '-28 days')), 0) / 28.0 AS per_day,
            ps.supplier_id, su.name AS supplier, su.phone AS supplier_phone,
            (SELECT name FROM product_units u WHERE u.product_id = p.id ORDER BY factor DESC LIMIT 1) AS pack_name,
            (SELECT factor FROM product_units u WHERE u.product_id = p.id ORDER BY factor DESC LIMIT 1) AS pack_factor
     FROM products p
     LEFT JOIN product_suppliers ps ON ps.product_id = p.id AND ps.is_primary = 1
     LEFT JOIN suppliers su ON su.id = ps.supplier_id
     WHERE p.is_active = 1 AND p.track_stock = 1 AND p.stock_quantity <= p.min_stock_level
     ORDER BY su.name IS NULL, su.name COLLATE NOCASE, p.name COLLATE NOCASE`,
  );

/** How many to order: enough for two weeks of sales or twice the reorder
 *  level, whichever is more, rounded up to whole packs when there are packs. */
export const suggestedOrder = (r: ReorderRow) => {
  const target = Math.max(r.min_stock * 2, r.per_day * 14, r.min_stock + 1);
  const need = Math.max(1, Math.ceil(target - Math.max(0, r.stock)));
  if (r.pack_factor && r.pack_factor > 1) {
    const packs = Math.ceil(need / r.pack_factor);
    return { units: packs * r.pack_factor, packs };
  }
  return { units: need, packs: null };
};

export interface StockCount {
  id: number;
  name: string | null;
  category_id: number | null;
  category: string | null;
  status: "open" | "applied" | "cancelled";
  user: string;
  started_at: string;
  finished_at: string | null;
  items: number;
}

export const listStockCounts = () =>
  select<StockCount>(
    `SELECT c.id, c.name, c.category_id, cat.name AS category, c.status, u.full_name AS user, c.started_at, c.finished_at,
            (SELECT COUNT(*) FROM stock_count_items i WHERE i.count_id = c.id) AS items
     FROM stock_counts c JOIN users u ON u.id = c.user_id LEFT JOIN categories cat ON cat.id = c.category_id
     ORDER BY c.status <> 'open', c.started_at DESC LIMIT 50`,
  );

export interface CountLine {
  product_id: number;
  name: string;
  unit: string;
  barcode: string | null;
  /** The system's figure now */
  stock: number;
  /** null until counted */
  counted: number | null;
  expected: number | null;
}

/** Every product in the count's scope, counted or not, plus anything
 *  counted from outside it */
export const getCountLines = (countId: number, categoryId: number | null) =>
  select<CountLine>(
    `SELECT p.id AS product_id, p.name, p.unit, p.barcode, p.stock_quantity AS stock, i.counted, i.expected
     FROM products p
     LEFT JOIN categories c ON c.id = p.category_id
     LEFT JOIN stock_count_items i ON i.count_id = $1 AND i.product_id = p.id
     WHERE p.track_stock = 1 AND (i.product_id IS NOT NULL OR (p.is_active = 1 AND ($2 IS NULL OR p.category_id = $2 OR c.parent_id = $2)))
     ORDER BY i.counted IS NOT NULL, p.name COLLATE NOCASE`,
    [countId, categoryId],
  );
