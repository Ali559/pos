// The activity log (audit_log): who did what, when, and on which till.
// Entries store an action code and JSON details; src/lib/activity.ts turns
// them into a sentence in the shop's language.
import { select } from "./client";
import { toUtc, type Range } from "@/lib/dateRange";

export interface AuditEntry {
  id: number;
  action: string;
  created_at: string;
  user_id: number | null;
  user_name: string | null;
  till_name: string | null;
  table_name: string | null;
  record_id: number | null;
  /** The thing it was about, by its current name (product, customer, receipt…) */
  target: string | null;
  details: string | null;
}

/** Filter groups; each is a set of action codes or prefixes */
export const ACTIVITY_GROUPS: Record<string, string[]> = {
  signin: ["login", "logout", "setup", "override"],
  shifts: ["shift.", "drawer.", "schedule."],
  sales: ["sale", "sale.", "refund"],
  stock: ["stock.", "product.", "category.", "import", "prices.", "promotion.", "delivery"],
  customers: ["customer.", "loyalty.", "supplier."],
  staff: ["role.", "user."],
  money: ["expense.", "expense_type."],
  system: ["setting", "shop.", "backup", "backup.", "till.", "export"],
};

const TARGET = `CASE a.table_name
  WHEN 'products' THEN (SELECT name FROM products WHERE id = a.record_id)
  WHEN 'customers' THEN (SELECT name FROM customers WHERE id = a.record_id)
  WHEN 'suppliers' THEN (SELECT name FROM suppliers WHERE id = a.record_id)
  WHEN 'categories' THEN (SELECT name FROM categories WHERE id = a.record_id)
  WHEN 'users' THEN (SELECT full_name FROM users WHERE id = a.record_id)
  WHEN 'roles' THEN (SELECT name FROM roles WHERE id = a.record_id)
  WHEN 'expense_types' THEN (SELECT name FROM expense_types WHERE id = a.record_id)
  WHEN 'expenses' THEN (SELECT et.name FROM expenses e JOIN expense_types et ON et.id = e.type_id WHERE e.id = a.record_id)
  WHEN 'promotions' THEN (SELECT name FROM promotions WHERE id = a.record_id)
  WHEN 'sales' THEN (SELECT receipt_number FROM sales WHERE id = a.record_id)
  WHEN 'returns' THEN (SELECT s.receipt_number FROM returns r JOIN sales s ON s.id = r.original_sale_id WHERE r.id = a.record_id)
  WHEN 'deliveries' THEN (SELECT su.name FROM deliveries d JOIN suppliers su ON su.id = d.supplier_id WHERE d.id = a.record_id)
  WHEN 'stock_counts' THEN (SELECT name FROM stock_counts WHERE id = a.record_id)
  WHEN 'tills' THEN (SELECT name FROM tills WHERE id = a.record_id)
END`;

export const listActivity = (f: { range: Range; userId?: number | null; group?: string | null; search?: string; limit?: number; offset?: number }) => {
  const params: unknown[] = [toUtc(f.range.from), toUtc(f.range.to)];
  const where = ["a.created_at >= $1", "a.created_at < $2"];
  if (f.userId) {
    params.push(f.userId);
    where.push(`a.user_id = $${params.length}`);
  }
  const codes = f.group ? ACTIVITY_GROUPS[f.group] : undefined;
  if (codes) {
    const parts = codes.map((c) => {
      params.push(c.endsWith(".") ? `${c}%` : c);
      return c.endsWith(".") ? `a.action LIKE $${params.length}` : `a.action = $${params.length}`;
    });
    where.push(`(${parts.join(" OR ")})`);
  }
  const q = f.search?.trim();
  if (q) {
    params.push(`%${q}%`);
    const n = params.length;
    where.push(`(${TARGET} LIKE $${n} OR a.new_value LIKE $${n} OR u.full_name LIKE $${n})`);
  }
  return select<AuditEntry>(
    `SELECT a.id, a.action, a.created_at, a.user_id, u.full_name AS user_name, t.name AS till_name,
            a.table_name, a.record_id, ${TARGET} AS target, a.new_value AS details
     FROM audit_log a
     LEFT JOIN users u ON u.id = a.user_id
     LEFT JOIN tills t ON t.id = a.till_id
     WHERE ${where.join(" AND ")}
     ORDER BY a.created_at DESC, a.id DESC
     LIMIT ${Math.trunc(f.limit ?? 100)} OFFSET ${Math.trunc(f.offset ?? 0)}`,
    params,
  );
};
