// The register's preview of what checkout charges. Mirrors
// src-tauri/src/sales.rs (checkout) and pricing.rs line for line; Rust is
// what's charged, this is what the cashier and the customer see first.

export type PriceLevel = "retail" | "wholesale";

export type PromoKind =
  | { kind: "percent"; percent: number }
  | { kind: "multibuy"; buy: number; free: number }
  | { kind: "bundle"; qty: number; price: number };

export interface Promo {
  id: number;
  name: string;
  rule: PromoKind;
  productIds: number[];
  categoryIds: number[];
}

/** What pricing needs to know about a product */
export interface PricedProduct {
  id: number;
  price: number;
  wholesalePrice: number | null;
  qtyPrices: { minQty: number; price: number }[];
  taxExempt: number | boolean;
  unit: string;
  categoryId: number | null;
  rootCategoryId: number | null;
}

/** A cart line, as far as pricing goes */
export interface PricedLine {
  id: number;
  quantity: number;
  /** Per selected unit for packs, scale labels and typed prices */
  price: number;
  discount: number;
  unitId: number | null;
  scaleBarcode: string | null;
  amount: number | null;
  openPrice: number | null;
  weighed: boolean;
}

export const roundMoney = (n: number, currency: string) => (currency === "IQD" ? Math.round(n) : Math.round(n * 100) / 100);

const WHOLE = ["piece", "pack", "box"];

/** pricing.rs unit_price */
export const unitPrice = (p: PricedProduct, level: PriceLevel, qty: number) => {
  if (level === "wholesale") return p.wholesalePrice ?? p.price;
  const tier = [...p.qtyPrices].filter((t) => t.minQty <= qty + 1e-9).sort((a, b) => b.minQty - a.minQty)[0];
  return tier ? Math.min(tier.price, p.price) : p.price;
};

/** pricing.rs promo_discount */
export const promoDiscount = (rule: PromoKind, qty: number, price: number, currency: string) => {
  let d = 0;
  if (rule.kind === "percent") d = (price * qty * Math.min(100, Math.max(0, rule.percent))) / 100;
  else if (rule.kind === "multibuy" && rule.buy > 0 && rule.free > 0) d = Math.floor(qty / (rule.buy + rule.free)) * rule.free * price;
  else if (rule.kind === "bundle" && rule.qty > 1) d = Math.max(0, Math.floor(qty / rule.qty) * (rule.qty * price - rule.price));
  return roundMoney(d, currency);
};

export const promosFor = (p: PricedProduct, promos: Promo[]) =>
  promos.filter(
    (pr) =>
      pr.productIds.includes(p.id) ||
      (p.categoryId !== null && pr.categoryIds.includes(p.categoryId)) ||
      (p.rootCategoryId !== null && pr.categoryIds.includes(p.rootCategoryId)),
  );

/** pricing.rs best_promotion */
export const bestPromotion = (promos: Promo[], qty: number, price: number, currency: string) => {
  let best: { promo: Promo; off: number } | null = null;
  for (const promo of promos) {
    const off = promoDiscount(promo.rule, qty, price, currency);
    if (off > 0 && (!best || off > best.off)) best = { promo, off };
  }
  return best;
};

/** sales.rs allocate: split `amount` by weights, last share takes the rest */
export const allocate = (amount: number, weights: number[], currency: string) => {
  const total = weights.reduce((a, b) => a + b, 0);
  if (amount === 0 || total <= 0) return weights.map(() => 0);
  let given = 0;
  return weights.map((w, i) => {
    const share = i === weights.length - 1 ? amount - given : roundMoney((amount * w) / total, currency);
    given += share;
    return share;
  });
};

export interface LinePrice {
  /** Per unit actually charged (after wholesale / quantity price) */
  each: number;
  gross: number;
  promo: number;
  promoName: string | null;
}

export interface CartPrice {
  lines: LinePrice[];
  subtotal: number;
  promotions: number;
  /** Discounts someone gave (lines + whole sale) */
  manual: number;
  discount: number;
  tax: number;
  total: number;
}

/**
 * Prices a cart the way checkout will. Plain lines of the same product and
 * pack are merged first (Rust does the same), so quantity prices and
 * promotions see the whole quantity; each cart line then gets its share.
 */
export const priceCart = (
  items: PricedLine[],
  products: Map<number, PricedProduct>,
  opts: { level: PriceLevel; promos: Promo[]; cartDiscount: number; taxRate: number; currency: string },
): CartPrice => {
  const { level, currency } = opts;
  const r = (n: number) => roundMoney(n, currency);
  const isPlain = (i: PricedLine) => i.scaleBarcode === null && i.openPrice === null;

  // Groups as checkout merges them
  const groups: { lines: number[]; qty: number; discount: number; gross: number; promo: number; promoName: string | null; each: number; exempt: boolean }[] = [];
  items.forEach((i, idx) => {
    const g = isPlain(i) ? groups.find((g) => { const f = items[g.lines[0]!]!; return isPlain(f) && f.id === i.id && f.unitId === i.unitId; }) : undefined;
    if (g) {
      g.lines.push(idx);
      g.qty += i.quantity;
      g.discount += i.discount;
    } else {
      groups.push({ lines: [idx], qty: i.quantity, discount: i.discount, gross: 0, promo: 0, promoName: null, each: i.price, exempt: false });
    }
  });

  for (const g of groups) {
    const first = items[g.lines[0]!]!;
    const p = products.get(first.id);
    g.exempt = !!p?.taxExempt;
    if (first.amount !== null) {
      g.gross = first.amount;
    } else if (first.unitId !== null || first.openPrice !== null || first.scaleBarcode !== null || !p) {
      g.gross = first.openPrice !== null || first.scaleBarcode !== null ? r(first.price * g.qty) : first.price * g.qty;
    } else {
      g.each = unitPrice(p, level, g.qty);
      g.gross = WHOLE.includes(p.unit) ? g.each * g.qty : r(g.each * g.qty);
      if (level === "retail") {
        const best = bestPromotion(promosFor(p, opts.promos), g.qty, g.each, currency);
        if (best) {
          g.promo = Math.min(best.off, g.gross);
          g.promoName = best.promo.name;
        }
      }
    }
    g.discount = r(g.discount);
  }

  // The whole-sale discount spread over what's left of each line
  const cart = r(opts.cartDiscount);
  const shares = allocate(cart, groups.map((g) => g.gross - g.discount - g.promo), currency);
  groups.forEach((g, i) => (g.discount += shares[i]!));

  const subtotal = r(groups.reduce((n, g) => n + g.gross, 0));
  const manual = groups.reduce((n, g) => n + g.discount, 0);
  const promotions = groups.reduce((n, g) => n + g.promo, 0);
  const discount = manual + promotions;
  const taxable = groups.filter((g) => !g.exempt).reduce((n, g) => n + g.gross - g.discount - g.promo, 0);
  const tax = r((taxable * opts.taxRate) / 100);

  // Each cart line's share of its group (by quantity)
  const lines: LinePrice[] = items.map(() => ({ each: 0, gross: 0, promo: 0, promoName: null }));
  for (const g of groups) {
    const promoShares = allocate(g.promo, g.lines.map((i) => items[i]!.quantity), currency);
    g.lines.forEach((li, k) => {
      const item = items[li]!;
      lines[li] = {
        each: g.each,
        gross: g.lines.length === 1 ? g.gross : item.amount ?? (g.each * item.quantity),
        promo: promoShares[k]!,
        promoName: g.promoName,
      };
    });
  }
  return { lines, subtotal, promotions, manual, discount, tax, total: subtotal - discount + tax };
};
