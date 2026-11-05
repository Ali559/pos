// Past sales, for the history screen, returns and receipts. Read-only;
// returns go through the `create_return` command.
import { select, selectOne } from "./client";
import { toUtc, type Range } from "@/lib/dateRange";

export type PaymentMethod = "cash" | "card" | "exchange";

export type SaleStatus = "completed" | "partially_refunded" | "refunded" | "pending";

export interface SaleRow {
  id: number;
  receipt_number: string;
  sale_date: string;
  cashier: string;
  items: number;
  total: number;
  refunded: number;
  method: PaymentMethod;
  status: SaleStatus;
}

/** Newest first. A search by receipt number ignores the date range. */
export const listSales = (opts: { range: Range; q?: string; limit?: number }) => {
  const q = opts.q?.trim().toUpperCase().replace(/^R-?/, "");
  const params: unknown[] = q ? [`%${q}`] : [toUtc(opts.range.from), toUtc(opts.range.to)];
  const where = q ? "s.receipt_number LIKE $1" : "s.sale_date >= $1 AND s.sale_date < $2";
  return select<SaleRow>(
    `SELECT s.id, s.receipt_number, s.sale_date, u.full_name AS cashier, s.total_amount AS total,
            s.payment_status AS status,
            (SELECT COALESCE(SUM(quantity), 0) FROM sale_items WHERE sale_id = s.id) AS items,
            (SELECT COALESCE(-SUM(amount), 0) FROM payments WHERE sale_id = s.id AND amount < 0 AND payment_method <> 'exchange') AS refunded,
            (SELECT payment_method FROM payments WHERE sale_id = s.id AND amount > 0 ${PAYMENTS_ORDER} LIMIT 1) AS method
     FROM sales s JOIN users u ON u.id = s.user_id
     WHERE ${where}
     ORDER BY s.sale_date DESC, s.id DESC LIMIT ${opts.limit ?? 200}`,
    params,
  );
};

export interface SaleLine {
  id: number;
  product_id: number;
  name: string;
  quantity: number;
  unit_price: number;
  discount: number;
  /** After discounts, before tax */
  subtotal: number;
  tax_rate: number;
  returned: number;
  unit: string;
}

export interface SaleDetail {
  id: number;
  receipt_number: string;
  sale_date: string;
  cashier: string;
  approver: string | null;
  subtotal: number;
  discount: number;
  tax: number;
  total: number;
  status: SaleStatus;
  /** The main payment (cash or card); an exchange adds an "exchange" one */
  method: PaymentMethod;
  tendered: number | null;
  change: number | null;
  refunded: number;
  payments: { method: PaymentMethod; amount: number; tendered: number | null }[];
  lines: SaleLine[];
  returns: { id: number; return_date: string; total_refund: number; reason: string | null; user: string; method: string }[];
}

/** Positive payments, the paying method first and exchange credit last */
const PAYMENTS_ORDER = "ORDER BY payment_method = 'exchange', id";

export const getSale = async (id: number): Promise<SaleDetail | null> => {
  const head = await selectOne<Omit<SaleDetail, "lines" | "returns">>(
    `SELECT s.id, s.receipt_number, s.sale_date, u.full_name AS cashier, a.full_name AS approver,
            s.subtotal, COALESCE(s.discount_amount, 0) AS discount, COALESCE(s.tax_amount, 0) AS tax,
            s.total_amount AS total, s.payment_status AS status,
            pay.payment_method AS method, pay.tendered, pay.change_given AS change,
            (SELECT COALESCE(-SUM(amount), 0) FROM payments WHERE sale_id = s.id AND amount < 0 AND payment_method <> 'exchange') AS refunded
     FROM sales s
     JOIN users u ON u.id = s.user_id
     LEFT JOIN users a ON a.id = s.approved_by
     LEFT JOIN payments pay ON pay.id = (SELECT id FROM payments WHERE sale_id = s.id AND amount > 0 ${PAYMENTS_ORDER} LIMIT 1)
     WHERE s.id = $1`,
    [id],
  );
  if (!head) return null;
  const [lines, returns, payments] = await Promise.all([
    select<SaleLine>(
      `SELECT si.id, si.product_id, si.product_name AS name, si.quantity, si.unit_price,
              COALESCE(si.discount_amount, 0) AS discount, si.subtotal, COALESCE(si.tax_rate, 0) AS tax_rate,
              COALESCE(p.unit, 'piece') AS unit,
              (SELECT COALESCE(SUM(quantity), 0) FROM return_items WHERE sale_item_id = si.id) AS returned
       FROM sale_items si LEFT JOIN products p ON p.id = si.product_id
       WHERE si.sale_id = $1 ORDER BY si.id`,
      [id],
    ),
    select<SaleDetail["returns"][number]>(
      `SELECT r.id, r.return_date, r.total_refund, r.reason, u.full_name AS user, r.refund_method AS method
       FROM returns r JOIN users u ON u.id = r.user_id
       WHERE r.original_sale_id = $1 ORDER BY r.id`,
      [id],
    ),
    select<SaleDetail["payments"][number]>(
      `SELECT payment_method AS method, amount, tendered FROM payments
       WHERE sale_id = $1 AND amount > 0 ${PAYMENTS_ORDER}`,
      [id],
    ),
  ]);
  return { ...head, lines, returns, payments };
};
