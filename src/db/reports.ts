// The end-of-day (Z) report: everything the shop took and paid out in a
// range (usually one day), and the same per cashier. Reads only.
import { select, selectOne } from "./client";
import { toUtc, type Range } from "@/lib/dateRange";

export interface DayTotals {
  sales: number;
  /** What the sales came to (after discounts, with tax) */
  total: number;
  /** Given by staff, and taken off by promotions */
  discounts: number;
  promotions: number;
  tax: number;
  returns: number;
  refunded: number;
  /** Customers paying off debt */
  debtCash: number;
  debtCard: number;
  /** Negative: cash taken from the till for suppliers */
  suppliers: number;
}

export interface MethodRow {
  method: string;
  /** Taken for sales */
  taken: number;
  /** Given back for returns (negative) */
  refunded: number;
}

export interface CashierRow {
  user_id: number;
  full_name: string;
  sales: number;
  total: number;
  discounts: number;
  cash: number;
  card: number;
  refunded: number;
  /** Sum of over (+) and short (−) on drawers they closed in the range; null if none */
  drawer: number | null;
  /** The same for the drawer's dollars; null if none were counted */
  drawer_usd: number | null;
}

export interface DayReport {
  totals: DayTotals;
  methods: MethodRow[];
  cashiers: CashierRow[];
}

const SALES = "s.sale_date >= $1 AND s.sale_date < $2 AND s.payment_status <> 'pending'";

export const dayReport = async (range: Range): Promise<DayReport> => {
  const p = [toUtc(range.from), toUtc(range.to)];
  const [head, money, methods, cashiers] = await Promise.all([
    selectOne<Pick<DayTotals, "sales" | "total" | "discounts" | "promotions" | "tax">>(
      `SELECT COUNT(*) AS sales, COALESCE(SUM(s.total_amount), 0) AS total,
              COALESCE(SUM(s.discount_amount), 0) - COALESCE(SUM((SELECT SUM(promo_discount) FROM sale_items WHERE sale_id = s.id)), 0) AS discounts,
              COALESCE(SUM((SELECT SUM(promo_discount) FROM sale_items WHERE sale_id = s.id)), 0) AS promotions,
              COALESCE(SUM(s.tax_amount), 0) AS tax
       FROM sales s WHERE ${SALES}`,
      p,
    ),
    selectOne<Pick<DayTotals, "returns" | "refunded" | "debtCash" | "debtCard" | "suppliers">>(
      `SELECT (SELECT COUNT(*) FROM returns WHERE return_date >= $1 AND return_date < $2) AS returns,
              (SELECT COALESCE(SUM(total_refund), 0) FROM returns WHERE return_date >= $1 AND return_date < $2) AS refunded,
              COALESCE(SUM(CASE WHEN ledger_id IS NOT NULL AND payment_method = 'cash' THEN amount END), 0) AS debtCash,
              COALESCE(SUM(CASE WHEN ledger_id IS NOT NULL AND payment_method = 'card' THEN amount END), 0) AS debtCard,
              COALESCE(SUM(CASE WHEN sale_id IS NULL AND return_id IS NULL AND ledger_id IS NULL AND amount < 0 THEN amount END), 0) AS suppliers
       FROM payments WHERE payment_date >= $1 AND payment_date < $2`,
      p,
    ),
    select<MethodRow>(
      `SELECT payment_method AS method,
              COALESCE(SUM(CASE WHEN amount > 0 THEN amount END), 0) AS taken,
              COALESCE(SUM(CASE WHEN amount < 0 THEN amount END), 0) AS refunded
       FROM payments
       WHERE payment_date >= $1 AND payment_date < $2 AND payment_method <> 'exchange'
         AND ledger_id IS NULL AND (sale_id IS NOT NULL OR return_id IS NOT NULL)
       GROUP BY payment_method ORDER BY taken DESC`,
      p,
    ),
    select<CashierRow>(
      `SELECT u.id AS user_id, u.full_name,
              (SELECT COUNT(*) FROM sales s WHERE s.user_id = u.id AND ${SALES}) AS sales,
              (SELECT COALESCE(SUM(s.total_amount), 0) FROM sales s WHERE s.user_id = u.id AND ${SALES}) AS total,
              (SELECT COALESCE(SUM(s.discount_amount), 0) - COALESCE(SUM((SELECT SUM(promo_discount) FROM sale_items WHERE sale_id = s.id)), 0)
                 FROM sales s WHERE s.user_id = u.id AND ${SALES}) AS discounts,
              (SELECT COALESCE(SUM(pay.amount), 0) FROM payments pay JOIN sales s ON s.id = pay.sale_id
                 WHERE s.user_id = u.id AND pay.payment_method IN ('cash', 'usd') AND pay.amount > 0 AND pay.payment_date >= $1 AND pay.payment_date < $2) AS cash,
              (SELECT COALESCE(SUM(pay.amount), 0) FROM payments pay JOIN sales s ON s.id = pay.sale_id
                 WHERE s.user_id = u.id AND pay.payment_method = 'card' AND pay.amount > 0 AND pay.payment_date >= $1 AND pay.payment_date < $2) AS card,
              (SELECT COALESCE(SUM(r.total_refund), 0) FROM returns r WHERE r.user_id = u.id AND r.return_date >= $1 AND r.return_date < $2) AS refunded,
              (SELECT SUM(cd.difference) FROM shifts sh JOIN cash_drawers cd ON cd.id = sh.cash_drawer_id
                 WHERE sh.user_id = u.id AND cd.closed_at >= $1 AND cd.closed_at < $2 AND cd.difference IS NOT NULL) AS drawer,
              (SELECT SUM(cd.difference_usd) FROM shifts sh JOIN cash_drawers cd ON cd.id = sh.cash_drawer_id
                 WHERE sh.user_id = u.id AND cd.closed_at >= $1 AND cd.closed_at < $2 AND cd.difference_usd IS NOT NULL) AS drawer_usd
       FROM users u
       WHERE EXISTS (SELECT 1 FROM sales s WHERE s.user_id = u.id AND ${SALES})
          OR EXISTS (SELECT 1 FROM returns r WHERE r.user_id = u.id AND r.return_date >= $1 AND r.return_date < $2)
          OR EXISTS (SELECT 1 FROM shifts sh JOIN cash_drawers cd ON cd.id = sh.cash_drawer_id
                     WHERE sh.user_id = u.id AND cd.closed_at >= $1 AND cd.closed_at < $2)
       ORDER BY total DESC`,
      p,
    ),
  ]);
  return {
    totals: {
      sales: 0, total: 0, discounts: 0, promotions: 0, tax: 0, returns: 0, refunded: 0, debtCash: 0, debtCard: 0, suppliers: 0,
      ...head,
      ...money,
    },
    methods,
    cashiers,
  };
};
