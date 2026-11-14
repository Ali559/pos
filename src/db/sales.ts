// Past sales, for the history screen, returns and receipts. Read-only;
// returns go through the `create_return` command.
import { select, selectOne } from "./client";
import { toUtc, type Range } from "@/lib/dateRange";

export type PaymentMethod = "cash" | "card" | "exchange" | "credit" | "points" | "usd";

type Payment = { method: PaymentMethod; amount: number; tendered: number | null; foreign_amount?: number | null; rate?: number | null };

/** How a payment reads on a receipt: dollars as "$100 @ 169,550". */
export const paymentText = (p: Payment, money: (n: number) => string, usd: (n: number) => string) =>
  p.method === "usd" && p.foreign_amount != null
    ? `${usd(p.foreign_amount)} @ ${Math.round(p.rate ?? 0).toLocaleString("en")}`
    : money(p.tendered ?? p.amount);

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
  /** Sold by the pack: its name and size (quantity is still in the product's unit) */
  unit_name: string | null;
  unit_factor: number | null;
  /** The promotion the line got; its amount is part of `discount` */
  promotion_name: string | null;
  promo_discount: number;
}

/** "Carton × 2" for a pack line, or the quantity; and the price of one of those */
export const lineSold = (l: Pick<SaleLine, "quantity" | "unit_price" | "unit_name" | "unit_factor">) => {
  const fmt = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(3).replace(/0+$/, ""));
  return l.unit_name && l.unit_factor
    ? { label: `${fmt(l.quantity / l.unit_factor)} × ${l.unit_name}`, each: l.unit_price * l.unit_factor }
    : { label: fmt(l.quantity), each: l.unit_price };
};

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
  payments: Payment[];
  /** Who bought it, and what they owe now (for the receipt) */
  customer_id: number | null;
  customer_name: string | null;
  customer_balance: number | null;
  /** Loyalty points the sale earned, and the customer's points now */
  points_earned: number;
  points_balance: number | null;
  lines: SaleLine[];
  returns: { id: number; return_date: string; total_refund: number; reason: string | null; user: string; method: string }[];
}

/** Positive payments, the paying method first; exchange credit and what was
 *  left on account after it */
const PAYMENTS_ORDER = "ORDER BY payment_method IN ('exchange', 'credit', 'points'), payment_method = 'card', id";

export const getSale = async (id: number): Promise<SaleDetail | null> => {
  const head = await selectOne<Omit<SaleDetail, "lines" | "returns">>(
    `SELECT s.id, s.receipt_number, s.sale_date, u.full_name AS cashier, a.full_name AS approver,
            s.customer_id, cu.name AS customer_name,
            CASE WHEN s.customer_id IS NULL THEN NULL
                 ELSE COALESCE((SELECT SUM(amount) FROM customer_ledger WHERE customer_id = s.customer_id), 0) END AS customer_balance,
            COALESCE((SELECT SUM(points) FROM loyalty_ledger WHERE sale_id = s.id AND kind = 'earn'), 0) AS points_earned,
            cu.loyalty_points AS points_balance,
            s.subtotal, COALESCE(s.discount_amount, 0) AS discount, COALESCE(s.tax_amount, 0) AS tax,
            s.total_amount AS total, s.payment_status AS status,
            pay.payment_method AS method, pay.tendered,
            (SELECT SUM(change_given) FROM payments WHERE sale_id = s.id AND amount > 0) AS change,
            (SELECT COALESCE(-SUM(amount), 0) FROM payments WHERE sale_id = s.id AND amount < 0 AND payment_method <> 'exchange') AS refunded
     FROM sales s
     JOIN users u ON u.id = s.user_id
     LEFT JOIN users a ON a.id = s.approved_by
     LEFT JOIN customers cu ON cu.id = s.customer_id
     LEFT JOIN payments pay ON pay.id = (SELECT id FROM payments WHERE sale_id = s.id AND amount > 0 ${PAYMENTS_ORDER} LIMIT 1)
     WHERE s.id = $1`,
    [id],
  );
  if (!head) return null;
  const [lines, returns, payments] = await Promise.all([
    select<SaleLine>(
      `SELECT si.id, si.product_id, si.product_name AS name, si.quantity, si.unit_price,
              COALESCE(si.discount_amount, 0) AS discount, si.subtotal, COALESCE(si.tax_rate, 0) AS tax_rate,
              COALESCE(p.unit, 'piece') AS unit, si.unit_name, si.unit_factor, si.promotion_name, COALESCE(si.promo_discount, 0) AS promo_discount,
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
      `SELECT payment_method AS method, amount, tendered, foreign_amount, rate FROM payments
       WHERE sale_id = $1 AND amount > 0 ${PAYMENTS_ORDER}`,
      [id],
    ),
  ]);
  return { ...head, lines, returns, payments };
};
