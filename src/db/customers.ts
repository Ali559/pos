// Customers and their debt. Reads only; changes go through the Rust
// commands in src-tauri/src/customers.rs.
import { select, selectOne } from "./client";

export interface Customer {
  id: number;
  name: string;
  phone: string | null;
  address: string | null;
  notes: string | null;
  /** Most they may owe; null for no limit */
  credit_limit: number | null;
  is_active: number;
  /** Positive: what they owe the shop */
  balance: number;
  /** Last sale, payment or adjustment */
  last_activity: string | null;
}

export type LedgerKind = "sale" | "payment" | "refund" | "adjustment";

export interface LedgerEntry {
  id: number;
  kind: LedgerKind;
  /** + owes more, − owes less */
  amount: number;
  sale_id: number | null;
  receipt_number: string | null;
  method: string | null;
  note: string | null;
  user: string;
  created_at: string;
  /** What they owed after this entry */
  running: number;
}

const COLUMNS = `
  c.id, c.name, c.phone, c.address, c.notes, c.credit_limit, c.is_active,
  COALESCE((SELECT SUM(amount) FROM customer_ledger l WHERE l.customer_id = c.id), 0) AS balance,
  (SELECT MAX(created_at) FROM customer_ledger l WHERE l.customer_id = c.id) AS last_activity`;

/** Phone digits only, like Rust stores them */
export const normalizePhone = (p: string) => p.replace(/[^\d+]/g, "");

export const listCustomers = (opts: { search?: string; owing?: boolean; includeInactive?: boolean; limit?: number } = {}) => {
  const where: string[] = opts.includeInactive ? [] : ["c.is_active = 1"];
  const params: unknown[] = [];
  const q = opts.search?.trim();
  if (q) {
    params.push(`%${q}%`, `%${normalizePhone(q) || "\u0000"}%`);
    where.push(`(c.name LIKE $1 OR c.phone LIKE $2)`);
  }
  const having = opts.owing ? "WHERE balance > 0.000001" : "";
  return select<Customer>(
    `SELECT * FROM (SELECT ${COLUMNS} FROM customers c ${where.length ? `WHERE ${where.join(" AND ")}` : ""}) ${having}
     ORDER BY ${opts.owing ? "balance DESC" : "name COLLATE NOCASE"} LIMIT ${Math.trunc(opts.limit ?? 500)}`,
    params,
  );
};

export const getCustomer = (id: number) => selectOne<Customer>(`SELECT ${COLUMNS} FROM customers c WHERE c.id = $1`, [id]);

/** Everyone's debt added up, and how many owe something */
export const debtTotals = async () =>
  (await selectOne<{ total: number; people: number }>(
    `SELECT COALESCE(SUM(b), 0) AS total, COUNT(*) AS people FROM
       (SELECT SUM(amount) AS b FROM customer_ledger GROUP BY customer_id HAVING SUM(amount) > 0.000001)`,
  )) ?? { total: 0, people: 0 };

/** Oldest first, with the running balance */
export const getLedger = (customerId: number) =>
  select<LedgerEntry>(
    `SELECT l.id, l.kind, l.amount, l.sale_id, s.receipt_number, l.method, l.note, u.full_name AS user, l.created_at,
            SUM(l.amount) OVER (ORDER BY l.created_at, l.id) AS running
     FROM customer_ledger l
     JOIN users u ON u.id = l.user_id
     LEFT JOIN sales s ON s.id = l.sale_id
     WHERE l.customer_id = $1
     ORDER BY l.created_at, l.id`,
    [customerId],
  );
