// Report queries. Everything is computed from sale_items (prices and costs
// as they were at the time of sale), so editing a product never rewrites
// history. Amounts are before tax unless named otherwise.
import { select, selectOne } from "./client";
import { toLocalDate, toUtc, type Range } from "@/lib/dateRange";

export interface Filters {
  range: Range;
  userId?: number | null;
  /** A top-level category also covers its subcategories */
  categoryId?: number | null;
  payment?: "cash" | "card" | null;
}

/** Shared WITH clause: the sale lines that match the filters. */
const linesCte = (f: Filters) => {
  const params: unknown[] = [toUtc(f.range.from), toUtc(f.range.to)];
  // Filters shared by sales and returns; the date applies to each one's own day
  const where: string[] = [];
  if (f.userId) {
    params.push(f.userId);
    where.push(`s.user_id = $${params.length}`);
  }
  if (f.payment) {
    params.push(f.payment);
    where.push(`EXISTS (SELECT 1 FROM payments pay WHERE pay.sale_id = s.id AND pay.amount > 0 AND pay.payment_method = $${params.length})`);
  }
  if (f.categoryId) {
    params.push(f.categoryId);
    where.push(`(p.category_id = $${params.length} OR c.parent_id = $${params.length})`);
  }
  const and = where.map((w) => ` AND ${w}`).join("");
  // Returns count as negative lines on the day they happened, with no
  // sale_id so they don't add transactions
  const sql = `WITH lines AS (
    SELECT si.sale_id, si.product_id, si.product_name, si.quantity, si.subtotal,
           si.cost_price * si.quantity AS cost, s.sale_date, s.user_id,
           COALESCE(c.parent_id, c.id) AS root_category_id
    FROM sale_items si
    JOIN sales s ON s.id = si.sale_id
    LEFT JOIN products p ON p.id = si.product_id
    LEFT JOIN categories c ON c.id = p.category_id
    WHERE s.sale_date >= $1 AND s.sale_date < $2 AND s.payment_status <> 'pending'${and}
    UNION ALL
    SELECT NULL, si.product_id, si.product_name, -ri.quantity, -ri.subtotal,
           -si.cost_price * ri.quantity, r.return_date, s.user_id,
           COALESCE(c.parent_id, c.id)
    FROM return_items ri
    JOIN returns r ON r.id = ri.return_id
    JOIN sale_items si ON si.id = ri.sale_item_id
    JOIN sales s ON s.id = si.sale_id
    LEFT JOIN products p ON p.id = si.product_id
    LEFT JOIN categories c ON c.id = p.category_id
    WHERE r.return_date >= $1 AND r.return_date < $2${and}
  )`;
  return { sql, params };
};

export interface Summary {
  revenue: number;
  transactions: number;
  items: number;
  cost: number;
}

export const summary = async (f: Filters): Promise<Summary> => {
  const { sql, params } = linesCte(f);
  const row = await selectOne<Summary>(
    `${sql} SELECT COALESCE(SUM(subtotal), 0) AS revenue, COUNT(DISTINCT sale_id) AS transactions,
            COALESCE(SUM(quantity), 0) AS items, COALESCE(SUM(cost), 0) AS cost FROM lines`,
    params,
  );
  return row ?? { revenue: 0, transactions: 0, items: 0, cost: 0 };
};

export type Bucket = "hour" | "day" | "week" | "month";

/** Bucket size that keeps a chart readable for the range length. */
export const bucketFor = (days: number): Bucket =>
  days <= 2 ? "hour" : days <= 62 ? "day" : days <= 190 ? "week" : "month";

const bucketExpr: Record<Bucket, string> = {
  hour: "strftime('%Y-%m-%d %H', sale_date, 'localtime')",
  day: "date(sale_date, 'localtime')",
  // The Saturday on or before the day (weeks start Saturday)
  week: "date(sale_date, 'localtime', '-6 days', 'weekday 6')",
  month: "strftime('%Y-%m', sale_date, 'localtime')",
};

export const series = (f: Filters, bucket: Bucket) => {
  const { sql, params } = linesCte(f);
  return select<{ bucket: string; revenue: number; transactions: number }>(
    `${sql} SELECT ${bucketExpr[bucket]} AS bucket, SUM(subtotal) AS revenue,
            COUNT(DISTINCT sale_id) AS transactions
     FROM lines GROUP BY bucket ORDER BY bucket`,
    params,
  );
};

/** Every bucket key in the range, so quiet periods show as zero. */
export const bucketKeys = (range: Range, bucket: Bucket): string[] => {
  const keys: string[] = [];
  const d = new Date(range.from);
  const pad = (n: number) => String(n).padStart(2, "0");
  if (bucket === "week") d.setDate(d.getDate() - ((d.getDay() - 6 + 7) % 7));
  if (bucket === "month") d.setDate(1);
  while (d < range.to && keys.length < 400) {
    if (bucket === "hour") {
      keys.push(`${toLocalDate(d)} ${pad(d.getHours())}`);
      d.setHours(d.getHours() + 1);
    } else if (bucket === "day") {
      keys.push(toLocalDate(d));
      d.setDate(d.getDate() + 1);
    } else if (bucket === "week") {
      keys.push(toLocalDate(d));
      d.setDate(d.getDate() + 7);
    } else {
      keys.push(`${d.getFullYear()}-${pad(d.getMonth() + 1)}`);
      d.setMonth(d.getMonth() + 1);
    }
  }
  return keys;
};

/** Weekday (0 = Sunday) × hour, local time. */
export const heatmap = (f: Filters) => {
  const { sql, params } = linesCte(f);
  return select<{ weekday: number; hour: number; transactions: number; revenue: number }>(
    `${sql} SELECT CAST(strftime('%w', sale_date, 'localtime') AS INTEGER) AS weekday,
            CAST(strftime('%H', sale_date, 'localtime') AS INTEGER) AS hour,
            COUNT(DISTINCT sale_id) AS transactions, SUM(subtotal) AS revenue
     FROM lines GROUP BY weekday, hour`,
    params,
  );
};

export interface ProductRow {
  product_id: number;
  name: string;
  quantity: number;
  revenue: number;
  margin: number;
}

export const productSales = (f: Filters) => {
  const { sql, params } = linesCte(f);
  return select<ProductRow>(
    `${sql} SELECT l.product_id, COALESCE(p.name, MAX(l.product_name)) AS name,
            SUM(l.quantity) AS quantity, SUM(l.subtotal) AS revenue, SUM(l.subtotal - l.cost) AS margin
     FROM lines l LEFT JOIN products p ON p.id = l.product_id
     GROUP BY l.product_id ORDER BY revenue DESC`,
    params,
  );
};

/** Products for sale that sold nothing in the range. */
export const slowMovers = (f: Filters) => {
  const { sql, params } = linesCte(f);
  const cat = f.categoryId ? `AND (p.category_id = $${params.length + 1} OR c.parent_id = $${params.length + 1})` : "";
  return select<{ id: number; name: string; stock: number }>(
    `${sql} SELECT p.id, p.name, p.stock_quantity AS stock
     FROM products p LEFT JOIN categories c ON c.id = p.category_id
     WHERE p.is_active = 1 AND p.id NOT IN (SELECT product_id FROM lines) ${cat}
     ORDER BY p.stock_quantity DESC LIMIT 20`,
    f.categoryId ? [...params, f.categoryId] : params,
  );
};

export const categorySales = (f: Filters) => {
  const { sql, params } = linesCte(f);
  return select<{ id: number | null; name: string | null; color: string | null; icon_key: string | null; revenue: number; margin: number }>(
    `${sql} SELECT l.root_category_id AS id, c.name, c.color, c.icon_key,
            SUM(l.subtotal) AS revenue, SUM(l.subtotal - l.cost) AS margin
     FROM lines l LEFT JOIN categories c ON c.id = l.root_category_id
     GROUP BY l.root_category_id ORDER BY revenue DESC`,
    params,
  );
};

/** Payments are per sale, so a category filter doesn't apply here. Refunds
 *  are negative payments, counted on the day they were paid out. */
export const paymentSplit = (f: Filters) => {
  const params: unknown[] = [toUtc(f.range.from), toUtc(f.range.to)];
  let user = "";
  if (f.userId) {
    params.push(f.userId);
    user = ` AND s.user_id = $3`;
  }
  return select<{ method: string; amount: number; transactions: number }>(
    `SELECT pay.payment_method AS method, SUM(pay.amount) AS amount,
            COUNT(DISTINCT CASE WHEN pay.amount > 0 THEN s.id END) AS transactions
     FROM payments pay JOIN sales s ON s.id = pay.sale_id
     WHERE pay.payment_date >= $1 AND pay.payment_date < $2 AND s.payment_status <> 'pending'${user}
     GROUP BY pay.payment_method ORDER BY amount DESC`,
    params,
  );
};

export interface EmployeeRow {
  user_id: number;
  full_name: string;
  transactions: number;
  revenue: number;
  items: number;
  /** Shift time overlapping the range, in hours */
  hours: number;
}

/** Sales per person plus hours worked from their shifts, clipped to the range. */
export const employeeStats = (f: Filters) => {
  const { sql, params } = linesCte(f);
  return select<EmployeeRow>(
    `${sql},
     per AS (
       SELECT user_id, COUNT(DISTINCT sale_id) AS transactions, SUM(subtotal) AS revenue, SUM(quantity) AS items
       FROM lines GROUP BY user_id
     ),
     worked AS (
       SELECT user_id, SUM(MAX(0,
           julianday(MIN(COALESCE(clocked_out_at, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')), $2))
         - julianday(MAX(clocked_in_at, $1))) * 24) AS hours
       FROM shifts WHERE clocked_in_at < $2 AND COALESCE(clocked_out_at, '9999') > $1
       GROUP BY user_id
     )
     SELECT u.id AS user_id, u.full_name,
            COALESCE(per.transactions, 0) AS transactions, COALESCE(per.revenue, 0) AS revenue,
            COALESCE(per.items, 0) AS items, COALESCE(worked.hours, 0) AS hours
     FROM users u
     LEFT JOIN per ON per.user_id = u.id
     LEFT JOIN worked ON worked.user_id = u.id
     WHERE per.user_id IS NOT NULL OR worked.user_id IS NOT NULL
     ORDER BY revenue DESC`,
    params,
  );
};

export interface ExpenseSum {
  type_id: number;
  name: string;
  icon_key: string;
  color: string | null;
  amount: number;
}

/** Confirmed expenses whose date falls in the range (local dates). */
export const expensesByType = (range: Range) =>
  select<ExpenseSum>(
    `SELECT t.id AS type_id, t.name, t.icon_key, t.color, SUM(e.amount) AS amount
     FROM expenses e JOIN expense_types t ON t.id = e.type_id
     WHERE e.status = 'confirmed' AND e.spent_on >= $1 AND e.spent_on < $2
     GROUP BY t.id ORDER BY amount DESC`,
    [toLocalDate(range.from), toLocalDate(range.to)],
  );

export const salesUsers = () =>
  select<{ id: number; full_name: string }>(
    "SELECT id, full_name FROM users ORDER BY is_active DESC, full_name",
  );
