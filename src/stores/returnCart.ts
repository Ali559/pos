import { computed, ref } from "vue";

// Items coming back at the register: picked from a past sale (sales
// history → Exchange) or scanned without a receipt. The register nets them
// against whatever is being bought; Rust works out the real refund.
export interface ReturnCartLine {
  /** Line of the original sale, when there's a receipt */
  saleItemId: number | null;
  productId: number;
  name: string;
  quantity: number;
  /** Most that can come back (receipt lines), Infinity without a receipt */
  max: number;
  /** Refund per unit including tax, for the preview */
  unitRefund: number;
  restock: boolean;
}

const saleId = ref<number | null>(null);
const receiptNumber = ref<string | null>(null);
const lines = ref<ReturnCartLine[]>([]);
const reason = ref("");

const round = (n: number, currency: string) => (currency === "IQD" ? Math.round(n) : Math.round(n * 100) / 100);

export const useReturnCart = () => ({
  saleId,
  receiptNumber,
  lines,
  reason,
  active: computed(() => lines.value.length > 0),
  /** Preview of the refund; the same rounding as returns.rs, per line */
  total: (currency: string) =>
    lines.value.reduce((sum, l) => sum + round(l.unitRefund * l.quantity, currency), 0),
  /** Starts a return against a sale (replaces anything pending) */
  fromSale(sale: { id: number; receipt: string }, picked: ReturnCartLine[]) {
    saleId.value = sale.id;
    receiptNumber.value = sale.receipt;
    lines.value = picked;
    reason.value = "";
  },
  /** Adds a scanned item with no receipt */
  addOpen(p: { id: number; name: string; unitRefund: number }) {
    if (saleId.value !== null) return false;
    const existing = lines.value.find((l) => l.productId === p.id);
    if (existing) existing.quantity++;
    else lines.value.push({ saleItemId: null, productId: p.id, name: p.name, quantity: 1, max: Infinity, unitRefund: p.unitRefund, restock: true });
    return true;
  },
  clear() {
    saleId.value = null;
    receiptNumber.value = null;
    lines.value = [];
    reason.value = "";
  },
  /** What create_return and checkout's `exchange` expect */
  payload() {
    return saleId.value !== null
      ? {
          saleId: saleId.value,
          lines: lines.value.map((l) => ({ saleItemId: l.saleItemId, quantity: l.quantity, restock: l.restock })),
          reason: reason.value || null,
        }
      : {
          saleId: null,
          products: lines.value.map((l) => ({ productId: l.productId, quantity: l.quantity, restock: l.restock })),
          reason: reason.value || null,
        };
  },
});
