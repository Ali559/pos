// bun test tests/
// The same cases as the Rust checkout tests, so the till's preview and what's
// charged can't drift apart.
import { describe, expect, test } from "bun:test";
import { bestPromotion, priceCart, promoDiscount, unitPrice, type PricedLine, type PricedProduct, type Promo } from "../src/lib/pricing";

const product = (id: number, over: Partial<PricedProduct> = {}): PricedProduct => ({
  id, price: 750, wholesalePrice: null, qtyPrices: [], taxExempt: 0, unit: "piece", categoryId: 10, rootCategoryId: 1, ...over,
});
const line = (id: number, quantity: number, over: Partial<PricedLine> = {}): PricedLine => ({
  id, quantity, price: 750, discount: 0, unitId: null, scaleBarcode: null, amount: null, openPrice: null, weighed: false, ...over,
});
const base = { level: "retail" as const, promos: [] as Promo[], cartDiscount: 0, taxRate: 10, currency: "IQD" };

describe("promotions", () => {
  test("percent, buy-2-get-1 and bundles", () => {
    expect(promoDiscount({ kind: "percent", percent: 10 }, 3, 750, "IQD")).toBe(225);
    const b2g1 = { kind: "multibuy", buy: 2, free: 1 } as const;
    expect([2, 3, 5, 6].map((q) => promoDiscount(b2g1, q, 750, "IQD"))).toEqual([0, 750, 750, 1500]);
    expect(promoDiscount({ kind: "bundle", qty: 3, price: 2000 }, 7, 750, "IQD")).toBe(500);
    expect(promoDiscount({ kind: "bundle", qty: 3, price: 3000 }, 3, 750, "IQD")).toBe(0);
  });
  test("the best one wins", () => {
    const promos: Promo[] = [
      { id: 1, name: "10%", rule: { kind: "percent", percent: 10 }, productIds: [1], categoryIds: [] },
      { id: 2, name: "2+1", rule: { kind: "multibuy", buy: 2, free: 1 }, productIds: [1], categoryIds: [] },
    ];
    expect(bestPromotion(promos, 2, 750, "IQD")?.promo.id).toBe(1);
    expect(bestPromotion(promos, 3, 750, "IQD")?.promo.id).toBe(2);
  });
});

describe("priceCart (mirrors sales.rs)", () => {
  const cola = product(1, { wholesalePrice: 600, qtyPrices: [{ minQty: 6, price: 700 }] });
  const bread = product(2, { price: 1000, taxExempt: 1 });
  const products = new Map([[1, cola], [2, bread]]);

  test("quantity price and tax-free lines", () => {
    expect(unitPrice(cola, "retail", 5)).toBe(750);
    expect(unitPrice(cola, "retail", 6)).toBe(700);
    const c = priceCart([line(1, 6), line(2, 1, { price: 1000 })], products, base);
    expect([c.subtotal, c.tax, c.total]).toEqual([5200, 420, 5620]);
  });

  test("wholesale ignores quantity prices and promotions", () => {
    const promos: Promo[] = [{ id: 1, name: "2+1", rule: { kind: "multibuy", buy: 2, free: 1 }, productIds: [1], categoryIds: [] }];
    const c = priceCart([line(1, 6)], products, { ...base, level: "wholesale", promos });
    expect([c.subtotal, c.promotions]).toEqual([3600, 0]);
  });

  test("a promotion by category, with tax on what's left", () => {
    const promos: Promo[] = [{ id: 1, name: "2+1", rule: { kind: "multibuy", buy: 2, free: 1 }, productIds: [], categoryIds: [1] }];
    const c = priceCart([line(1, 3)], products, { ...base, promos });
    expect([c.subtotal, c.promotions, c.discount, c.tax, c.total]).toEqual([2250, 750, 750, 150, 1650]);
    expect(c.lines[0]!.promoName).toBe("2+1");
  });

  test("repeated lines merge before quantity prices, like checkout", () => {
    const c = priceCart([line(1, 3), line(1, 3)], products, base);
    expect(c.subtotal).toBe(4200);
  });

  test("the whole-sale discount is spread like Rust's allocate", () => {
    const c = priceCart([line(1, 1), line(2, 1, { price: 1000 })], products, { ...base, cartDiscount: 175 });
    // 750 and 1000 share 175: 75 and 100; tax only on 750 − 75
    expect([c.manual, c.tax, c.total]).toEqual([175, 68, 1643]);
  });
});
