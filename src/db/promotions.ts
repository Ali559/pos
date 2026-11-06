// Promotions. Changes go through save_promotion / delete_promotion
// (src-tauri/src/pricing.rs).
import { select } from "./client";
import type { Promo, PromoKind } from "@/lib/pricing";

export interface PromotionRow {
  id: number;
  name: string;
  kind: "percent" | "multibuy" | "bundle";
  percent: number | null;
  buy_qty: number | null;
  free_qty: number | null;
  bundle_price: number | null;
  starts_on: string | null;
  ends_on: string | null;
  is_active: number;
  product_ids: number[];
  category_ids: number[];
  /** Sold under it so far */
  uses: number;
}

export const listPromotions = async (): Promise<PromotionRow[]> => {
  const rows = await select<Omit<PromotionRow, "product_ids" | "category_ids"> & { products: string | null; cats: string | null }>(
    `SELECT pr.id, pr.name, pr.kind, pr.percent, pr.buy_qty, pr.free_qty, pr.bundle_price, pr.starts_on, pr.ends_on, pr.is_active,
            (SELECT group_concat(product_id) FROM promotion_targets t WHERE t.promotion_id = pr.id AND product_id IS NOT NULL) AS products,
            (SELECT group_concat(category_id) FROM promotion_targets t WHERE t.promotion_id = pr.id AND category_id IS NOT NULL) AS cats,
            (SELECT COUNT(*) FROM sale_items si WHERE si.promotion_id = pr.id) AS uses
     FROM promotions pr ORDER BY pr.is_active DESC, pr.id DESC`,
  );
  const ids = (s: string | null) => (s ? s.split(",").map(Number) : []);
  return rows.map(({ products, cats, ...r }) => ({ ...r, product_ids: ids(products), category_ids: ids(cats) }));
};

/** Local date, YYYY-MM-DD, as Rust's stock::today() */
export const todayLocal = () => new Date().toLocaleDateString("en-CA");

export const isRunning = (p: PromotionRow, today = todayLocal()) =>
  !!p.is_active && (!p.starts_on || p.starts_on <= today) && (!p.ends_on || p.ends_on >= today);

export const ruleOf = (p: PromotionRow): PromoKind | null =>
  p.kind === "percent" && p.percent != null
    ? { kind: "percent", percent: p.percent }
    : p.kind === "multibuy" && p.buy_qty != null && p.free_qty != null
      ? { kind: "multibuy", buy: p.buy_qty, free: p.free_qty }
      : p.kind === "bundle" && p.buy_qty != null && p.bundle_price != null
        ? { kind: "bundle", qty: p.buy_qty, price: p.bundle_price }
        : null;

/** The promotions running today, for the register's preview */
export const runningPromotions = async (): Promise<Promo[]> =>
  (await listPromotions())
    .filter((p) => isRunning(p))
    .flatMap((p) => {
      const rule = ruleOf(p);
      return rule ? [{ id: p.id, name: p.name, rule, productIds: p.product_ids, categoryIds: p.category_ids }] : [];
    });
