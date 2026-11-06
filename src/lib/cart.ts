// A line on the register's cart. Kept here so held carts (stored as JSON)
// and the register agree on its shape. Rust re-prices everything at
// checkout (src-tauri/src/sales.rs); these numbers are the preview.

export interface CartItem {
  /** Unique per line: a carton and singles of the same product are two lines */
  key: string;
  /** Product id */
  id: number;
  /** Product name, with the note for a typed-price line */
  name: string;
  /** Per selected unit: the pack's price, per kg, or the typed price */
  price: number;
  /** In the selected unit (packs, kg, pieces) */
  quantity: number;
  /** Off this line, as an amount */
  discount: number;
  /** Sold by the pack */
  unitId: number | null;
  unitName: string | null;
  /** How many of the product's own unit one of `quantity` is (1 when no pack) */
  factor: number;
  /** Quantity can be fractional (kg, liter): edited with the weight dialog */
  weighed: boolean;
  /** A scale label; Rust decodes it again */
  scaleBarcode: string | null;
  /** A price label's total: the line costs exactly this */
  amount: number | null;
  /** A typed price (open-price products) */
  openPrice: number | null;
  note: string | null;
}

/** Carts held before pack sizes existed only had the first few fields */
export const normalizeItem = (i: Partial<CartItem> & { id: number; name: string; price: number; quantity: number }): CartItem => ({
  discount: 0,
  unitId: null,
  unitName: null,
  factor: 1,
  weighed: false,
  scaleBarcode: null,
  amount: null,
  openPrice: null,
  note: null,
  ...i,
  key: i.key ?? `${i.id}:`,
});

/** Before discounts. Weighed and typed lines are rounded like Rust does. */
export const lineGross = (i: CartItem, round: (n: number) => number) =>
  i.amount ?? (i.weighed || i.openPrice !== null ? round(i.price * i.quantity) : i.price * i.quantity);

/** In the product's own unit: what stock counts */
export const baseQuantity = (i: CartItem) => i.quantity * i.factor;

/** "Carton × 2", "1.25 kg", "3" for the line's quantity */
export const quantityLabel = (i: CartItem, unitLabel: (unit: string) => string, unit: string) => {
  const n = Number(i.quantity.toFixed(3));
  if (i.unitName) return `${i.unitName} × ${n}`;
  if (i.weighed) return `${n} ${unitLabel(unit)}`;
  return String(n);
};
