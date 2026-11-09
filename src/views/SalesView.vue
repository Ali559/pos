<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask } from "@tauri-apps/plugin-dialog";
import { closeDisplay, displayOpen, onDisplayReady, openDisplay, publish, type DisplayState } from "@/lib/customerDisplay";
import { inDesktopApp } from "@/lib/ipc";
import { mediaUrl } from "@/lib/media";
import { i18n } from "@/utils/i18n";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  Banknote,
  Check,
  Keyboard,
  MonitorSmartphone,
  ChevronLeft,
  ChevronRight,
  Star,
  CreditCard,
  Gift,
  History,
  Loader2,
  Minus,
  NotebookPen,
  Package,
  Percent,
  Plus,
  Printer,
  ScanBarcode,
  ScanSearch,
  Trash2,
  Undo2,
  UserRound,
  X,
} from "lucide-vue-next";
import CustomerPicker from "@/components/customers/CustomerPicker.vue";
import OnAccountDialog from "@/components/customers/OnAccountDialog.vue";
import { getCustomer, type Customer } from "@/db/customers";
import HeldSales, { type HeldCart } from "@/components/sales/HeldSales.vue";
import OpeningCashDialog from "@/components/sales/OpeningCashDialog.vue";
import { useReturnCart } from "@/stores/returnCart";
import { RouterLink } from "vue-router";
import CashDialog from "@/components/sales/CashDialog.vue";
import DiscountDialog from "@/components/sales/DiscountDialog.vue";
import ManagerPinDialog from "@/components/sales/ManagerPinDialog.vue";
import WeightDialog from "@/components/sales/WeightDialog.vue";
import OpenPriceDialog from "@/components/sales/OpenPriceDialog.vue";
import PriceCheckDialog from "@/components/sales/PriceCheckDialog.vue";
import { baseQuantity, lineGross, normalizeItem, quantityLabel, type CartItem } from "@/lib/cart";
import { priceCart, type PriceLevel, type Promo } from "@/lib/pricing";
import { runningPromotions } from "@/db/promotions";
import type { ScanResult } from "@/lib/scan";
import { getSetting } from "@/db/settings";
import { printReceipt } from "@/lib/print";
import { useAuth } from "@/stores/auth";
import { usePayments } from "@/stores/payments";
import {
  getProductById,
  isWeighed,
  listCategories,
  listProducts,
  stockState,
  type Category,
  type Product,
  type ProductUnit,
} from "@/db/catalog";
import { formatCurrency } from "@/lib/format";
import { categoryInk, categoryTint } from "@/lib/categoryColor";
import AppIcon from "@/components/icons/AppIcon.vue";
import ProductThumb from "@/components/catalog/ProductThumb.vue";
import MediaImage from "@/components/media/MediaImage.vue";
import { useShop } from "@/stores/shop";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";

interface SaleResult {
  id: number;
  receiptNumber: string;
  total: number;
  change: number | null;
  /** Exchange: value of what came back, and what was paid back on top */
  returned: number;
  refunded: number;
  /** Left on the customer's account */
  onAccount: number;
  tendered: number | null;
  /** Split payment: the card part */
  cardAmount: number;
  /** Loyalty points spent (and their value), earned, and the balance after */
  pointsUsed: number;
  pointsValue: number;
  pointsEarned: number;
  pointsBalance: number | null;
}

/** What the done panel shows: a sale (maybe an exchange) or a plain refund */
type Done = { kind: "sale"; sale: SaleResult } | { kind: "refund"; amount: number; pointsBack: number };

const { t } = useI18n();
const { taxRate, currency, load: loadShop, profile } = useShop();
const auth = useAuth();
// Card payments only when the shop takes cards; loyalty points unless off
const { cards: cardsOn, loyalty: loyaltyOn, pointValue, load: loadPayments } = usePayments();
const pinDialog = ref<InstanceType<typeof ManagerPinDialog> | null>(null);
const discountDialog = ref<InstanceType<typeof DiscountDialog> | null>(null);
const cashDialog = ref<InstanceType<typeof CashDialog> | null>(null);
const weightDialog = ref<InstanceType<typeof WeightDialog> | null>(null);
const openPriceDialog = ref<InstanceType<typeof OpenPriceDialog> | null>(null);
const priceCheckOpen = ref(false);
const heldSales = ref<InstanceType<typeof HeldSales> | null>(null);
const shortcutsOpen = ref(false);
// Esc twice clears the cart; the first press only warns
const clearArmed = ref(false);
const onAccountDialog = ref<InstanceType<typeof OnAccountDialog> | null>(null);
// Who's buying: needed to sell on account (قەرز)
const customer = ref<Customer | null>(null);
const pickerOpen = ref(false);
const canAccount = computed(() => auth.can("customers.credit"));
const pickCustomer = (c: Customer) => {
  customer.value = c;
  // A wholesale customer's sale starts at wholesale prices
  priceLevel.value = c.price_level;
  focusScan();
};
// Retail or wholesale prices for this sale; promotions (running today)
const priceLevel = ref<PriceLevel>("retail");
const promos = ref<Promo[]>([]);
const autoPrint = ref(false);
const returns = useReturnCart();
// Scans go to the items coming back instead of the cart
const returnMode = ref(false);
const openingOpen = ref(false);
const openingExpected = ref(0);

// State
const barcode = ref("");
const scanInput = ref<HTMLInputElement | null>(null);
// The code that failed; the message is derived so it follows the language
const notFoundCode = ref("");
const scanError = computed(() =>
  notFoundCode.value ? t("pos.notFound", { code: notFoundCode.value }) : "",
);
const categories = ref<Category[]>([]);
const products = ref<Product[]>([]);
const catalogLoaded = ref(false);
const activeCategory = ref<number | null>(null);
const activeSub = ref<number | null>(null);
const payment = ref<"cash" | "card" | "account">("cash");
const lastAdded = ref<string | null>(null);
// The last completed sale or refund, shown on the empty receipt until the next scan
const lastSale = ref<Done | null>(null);
const charging = ref(false);
const chargeError = ref("");
const cartItems = ref<CartItem[]>([]);
const cartDiscount = ref(0);

// Computed
const roots = computed(() => categories.value.filter((c) => c.parent_id === null));
const subcategories = computed(() =>
  activeCategory.value === null || activeCategory.value === QUICK
    ? []
    : categories.value.filter((c) => c.parent_id === activeCategory.value),
);
const pickCategory = (id: number | null) => {
  activeCategory.value = id;
  activeSub.value = null;
};

// Quick keys: products pinned to the front, in their own order
const QUICK = -1;
const quickProducts = computed(() =>
  products.value.filter((p) => p.quickKey !== null).sort((a, b) => a.quickKey! - b.quickKey!),
);
const moveQuick = async (p: Product, by: number) => {
  try {
    await call("move_quick_key", { id: p.id, by });
    await reloadProducts();
  } catch (e) {
    chargeError.value = errorMessage(e);
  }
};

const visibleProducts = computed(() =>
  activeCategory.value === QUICK
    ? quickProducts.value
    : activeSub.value !== null
    ? products.value.filter((p) => p.categoryId === activeSub.value)
    : activeCategory.value !== null
      ? products.value.filter((p) => p.rootCategoryId === activeCategory.value)
      : products.value,
);

/** In the product's own unit, over every line (singles + cartons) */
const qtyInCart = (id: number) =>
  Number(cartItems.value.filter((i) => i.id === id).reduce((n, i) => n + baseQuantity(i), 0).toFixed(3));

// Weighed lines count as one item each
const itemCount = computed(() =>
  cartItems.value.reduce((n, i) => n + (i.weighed || i.amount !== null ? 1 : i.quantity), 0),
);
// Mirrors the Rust totals (sales::checkout), which are what gets charged
const roundMoney = (n: number) => (currency.value === "IQD" ? Math.round(n) : Math.round(n * 100) / 100);
// Priced the way checkout prices it (src/lib/pricing.ts mirrors sales.rs)
const productMap = computed(() => new Map(products.value.map((p) => [p.id, p])));
const priced = computed(() =>
  priceCart(cartItems.value, productMap.value, {
    level: priceLevel.value,
    promos: promos.value,
    cartDiscount: cartDiscount.value,
    taxRate: taxRate.value,
    currency: currency.value,
  }),
);
const lineOf = (i: CartItem) => priced.value.lines[cartItems.value.indexOf(i)];
const gross = (i: CartItem) => lineOf(i)?.gross ?? lineGross(i, roundMoney);
const promoOf = (i: CartItem) => lineOf(i)?.promo ?? 0;
const eachOf = (i: CartItem) => lineOf(i)?.each || i.price;
const subtotal = computed(() => priced.value.subtotal);
const lineDiscounts = computed(() => cartItems.value.reduce((sum, i) => sum + i.discount, 0));
const discount = computed(() => priced.value.discount);
const tax = computed(() => priced.value.tax);
const total = computed(() => priced.value.total);
const returnValue = computed(() => returns.total(currency.value));
// What the customer pays (positive) or gets back (negative)
const net = computed(() => (cartItems.value.length ? total.value : 0) - returnValue.value);
const nothingToCharge = computed(() => !cartItems.value.length && !returns.active.value);

// Loyalty: the customer's points can pay for part or all of the sale
const usePoints = ref(false);
const pointsUsable = computed(() => {
  const c = customer.value;
  if (!loyaltyOn.value || !c || !cartItems.value.length || net.value <= 0 || pointValue.value <= 0) return 0;
  return Math.max(0, Math.min(c.loyalty_points, Math.floor((net.value + 1e-6) / pointValue.value)));
});
const pointsToUse = computed(() => (usePoints.value ? pointsUsable.value : 0));
const pointsValue = computed(() => roundMoney(pointsToUse.value * pointValue.value));
// What's left to pay with money
const dueNow = computed(() => net.value - pointsValue.value);

// Methods
const focusScan = () => nextTick(() => scanInput.value?.focus());

// No receipt: refunded at today's price with tax (returns.rs does the same)
const addToReturn = (p: Product) => {
  lastSale.value = null;
  chargeError.value = "";
  const unit = roundMoney(p.price) + roundMoney((p.price * taxRate.value) / 100);
  if (!returns.addOpen({ id: p.id, name: p.name, unitRefund: unit })) {
    chargeError.value = t("returnCart.receiptPending");
  }
};

const productOf = (id: number) => products.value.find((p) => p.id === id);
const unitLabel = (u: string) => t(`catalog.units.${u}`).toLocaleLowerCase();

/** Room left for `p`, in its own unit (Infinity when stock isn't counted) */
const roomFor = (p: Product, exceptKey?: string) =>
  p.trackStock ? p.stock - cartItems.value.filter((i) => i.id === p.id && i.key !== exceptKey).reduce((n, i) => n + baseQuantity(i), 0) : Infinity;

const flash = (key: string) => {
  lastAdded.value = key;
  setTimeout(() => {
    if (lastAdded.value === key) lastAdded.value = null;
  }, 600);
};

type NewLine = Omit<CartItem, "key"> & { key?: string };
const plainLine = (p: Product): NewLine => ({
  id: p.id, name: p.name, price: p.price, quantity: 1, discount: 0,
  unitId: null, unitName: null, factor: 1, weighed: false,
  scaleBarcode: null, amount: null, openPrice: null, note: null,
});

let lineSeq = 0;
const push = (item: NewLine) => {
  const line = normalizeItem({ ...item, key: item.key ?? `${item.id}:x${++lineSeq}` });
  cartItems.value.push(line);
  flash(line.key);
};

/** One more of a product (or one of its packs); a new line if needed */
const addOne = (p: Product, unit: ProductUnit | null = null) => {
  const factor = unit?.factor ?? 1;
  if (roomFor(p) < factor - 1e-9) {
    chargeError.value = t("pos.notEnough", { name: p.name });
    return;
  }
  const key = `${p.id}:${unit?.id ?? ""}`;
  const existing = cartItems.value.find((i) => i.key === key);
  if (existing) {
    existing.quantity++;
    flash(key);
  } else {
    push({ ...plainLine(p), key, price: unit?.price ?? p.price, unitId: unit?.id ?? null, unitName: unit?.name ?? null, factor });
  }
};

const addProduct = async (p: Product) => {
  if (returnMode.value) return addToReturn(p);
  if (stockState(p) === "out") return;
  lastSale.value = null;
  chargeError.value = "";
  // Something typed at the till: ask its price (and what it was)
  if (p.openPrice) {
    const r = await openPriceDialog.value?.ask({ name: p.name });
    if (r) push({ ...plainLine(p), name: r.note ? `${p.name}: ${r.note}` : p.name, price: r.price, openPrice: r.price, note: r.note || null });
    return focusScan();
  }
  // Sold by weight without a scale label: ask how much
  if (isWeighed(p.unit)) {
    const qty = await weightDialog.value?.ask({ name: p.name, unit: unitLabel(p.unit), price: p.price, max: roomFor(p) });
    if (qty) push({ ...plainLine(p), quantity: qty, weighed: true });
    return focusScan();
  }
  addOne(p);
};

/** A scanned code: a product, one of its packs, or a scale label */
const addScanned = async (scan: ScanResult) => {
  const p = productOf(scan.productId) ?? (await getProductById(scan.productId));
  if (!p) return false;
  if (returnMode.value) {
    addToReturn(p);
    return true;
  }
  lastSale.value = null;
  chargeError.value = "";
  if (scan.unitId !== null) {
    const unit = p.units.find((u) => u.id === scan.unitId);
    if (unit) addOne(p, unit);
    return true;
  }
  if (scan.scaleBarcode) {
    if (scan.quantity > roomFor(p) + 1e-9) {
      chargeError.value = t("pos.notEnough", { name: p.name });
      return true;
    }
    push({ ...plainLine(p), quantity: scan.quantity, weighed: true, scaleBarcode: scan.scaleBarcode, amount: scan.amount });
    return true;
  }
  await addProduct(p);
  return true;
};

const addItemByBarcode = async () => {
  const code = barcode.value.trim();
  if (!code) return;
  try {
    const scan = await call<ScanResult | null>("resolve_scan", { code });
    if (!scan || !(await addScanned(scan))) {
      notFoundCode.value = code;
      return;
    }
  } catch (e) {
    chargeError.value = errorMessage(e);
    return;
  }
  notFoundCode.value = "";
  barcode.value = "";
};

const incrementQuantity = (key: string) => {
  const item = cartItems.value.find((i) => i.key === key);
  const p = item && productOf(item.id);
  if (!item || !p) return;
  if (item.weighed) return editWeight(item);
  if (roomFor(p) >= item.factor - 1e-9) item.quantity++;
  else chargeError.value = t("pos.notEnough", { name: p.name });
};

const editWeight = async (item: CartItem) => {
  const p = productOf(item.id);
  if (!p || item.scaleBarcode) return;
  const qty = await weightDialog.value?.ask({ name: p.name, unit: unitLabel(p.unit), price: item.price, current: item.quantity, max: roomFor(p, item.key) });
  if (!qty) return focusScan();
  // Less than before takes something off the cart: that's a void
  if (qty < item.quantity) {
    return runVoid([{ id: item.id, quantity: item.quantity - qty }], () => {
      item.quantity = qty;
      item.discount = Math.min(item.discount, gross(item));
    });
  }
  item.quantity = qty;
  focusScan();
};

/** Switches a line between singles and a pack size */
const switchUnit = (item: CartItem, unitId: string) => {
  const p = productOf(item.id);
  if (!p) return;
  const unit = p.units.find((u) => String(u.id) === unitId) ?? null;
  const factor = unit?.factor ?? 1;
  if (roomFor(p, item.key) < item.quantity * factor - 1e-9) {
    chargeError.value = t("pos.notEnough", { name: p.name });
    return;
  }
  const key = `${p.id}:${unit?.id ?? ""}`;
  const same = cartItems.value.find((i) => i.key === key && i !== item);
  if (same) {
    same.quantity += item.quantity;
    cartItems.value = cartItems.value.filter((i) => i !== item);
    return;
  }
  Object.assign(item, { key, unitId: unit?.id ?? null, unitName: unit?.name ?? null, factor, price: unit?.price ?? p.price, discount: 0 });
};

// Taking something back off the cart is a void: it needs sales.void_line
// or a manager's PIN, and Rust logs it
const approveVoid = async (lines: { id: number; quantity: number }[]) => {
  let overrideToken: string | null = null;
  if (!auth.can("sales.void_line")) {
    const grant = await pinDialog.value?.ask("sales.void_line");
    if (!grant) return false;
    overrideToken = grant.token;
  }
  await call("void_line", { lines: lines.map((l) => ({ productId: l.id, quantity: l.quantity })), overrideToken });
  return true;
};

const runVoid = async (items: { id: number; quantity: number }[], apply: () => void) => {
  chargeError.value = "";
  try {
    if (await approveVoid(items)) apply();
  } catch (e) {
    chargeError.value = errorMessage(e);
  }
  focusScan();
};

const decrementQuantity = (key: string) => {
  const item = cartItems.value.find((i) => i.key === key);
  if (!item) return;
  if (item.weighed) return editWeight(item);
  if (item.quantity <= 1) return removeFromCart(key);
  runVoid([{ id: item.id, quantity: item.factor }], () => {
    item.quantity--;
    item.discount = Math.min(item.discount, gross(item));
  });
};

const removeFromCart = (key: string) => {
  const item = cartItems.value.find((i) => i.key === key);
  if (!item) return;
  runVoid([{ id: item.id, quantity: baseQuantity(item) }], () => {
    cartItems.value = cartItems.value.filter((i) => i.key !== key);
    if (!cartItems.value.length) cartDiscount.value = 0;
  });
};

const resetCart = () => {
  cartItems.value = [];
  cartDiscount.value = 0;
  customer.value = null;
  usePoints.value = false;
  priceLevel.value = "retail";
  if (payment.value === "account") payment.value = "cash";
  focusScan();
};

const clearCart = () => runVoid(cartItems.value.map((i) => ({ id: i.id, quantity: baseQuantity(i) })), resetCart);

const stepReturn = (i: number, by: number) => {
  const l = returns.lines.value[i]!;
  l.quantity = Math.min(l.max, l.quantity + by);
  if (l.quantity <= 0) returns.lines.value.splice(i, 1);
  if (!returns.active.value) returnMode.value = false;
};

const cancelReturn = () => {
  returns.clear();
  returnMode.value = false;
  focusScan();
};

// Hold: the cart goes on a shelf; the register is free for the next customer
const heldCart = computed<HeldCart>(() => ({ items: cartItems.value, cartDiscount: cartDiscount.value, customerId: customer.value?.id ?? null }));
const resumeHeld = async (cart: HeldCart) => {
  lastSale.value = null;
  cartItems.value = cart.items.map((i) => normalizeItem({ ...i }));
  cartDiscount.value = cart.cartDiscount;
  // Looked up again: the balance may have changed while it was held
  customer.value = cart.customerId ? await getCustomer(cart.customerId) : null;
  focusScan();
};

const editLineDiscount = async (item: CartItem) => {
  const n = await discountDialog.value?.ask({ title: item.name, base: gross(item), current: item.discount });
  if (n != null) item.discount = roundMoney(n);
  focusScan();
};

const editCartDiscount = async () => {
  const n = await discountDialog.value?.ask({
    title: t("discount.wholeSale"),
    base: subtotal.value - lineDiscounts.value - priced.value.promotions,
    current: cartDiscount.value,
  });
  if (n != null) cartDiscount.value = roundMoney(n);
  focusScan();
};

const reloadProducts = async () => {
  products.value = await listProducts();
};

// Rust recomputes prices from the database; only ids, quantities and
// discount amounts go over
const approve = async (permission: string) => {
  if (auth.can(permission)) return null;
  const grant = await pinDialog.value?.ask(permission);
  return grant ? grant.token : undefined;
};

const checkout = async () => {
  if (nothingToCharge.value || charging.value) return;
  chargeError.value = "";
  // Items back and nothing bought: a plain refund
  if (!cartItems.value.length) return refund();

  let tendered: number | null = null;
  let cardAmount = 0;
  let onAccount = 0;
  let overLimitOverrideToken: string | null | undefined = null;
  const points = pointsToUse.value;
  if (payment.value === "card" && !cardsOn.value) payment.value = "cash";
  if (payment.value === "account") {
    // Part paid in cash now (maybe nothing), the rest on the customer's account
    if (!customer.value || dueNow.value <= 0) return;
    const paidNow = await onAccountDialog.value?.ask({ due: dueNow.value, customer: customer.value });
    if (paidNow == null) return focusScan();
    onAccount = dueNow.value - paidNow;
    const limit = customer.value.credit_limit;
    if (limit != null && customer.value.balance + onAccount > limit + 1e-6) {
      overLimitOverrideToken = await approve("customers.over_limit");
      if (overLimitOverrideToken === undefined) return focusScan();
    }
  } else if (payment.value === "cash" && dueNow.value > 0) {
    const paid = (await cashDialog.value?.ask(dueNow.value, { split: cardsOn.value })) ?? null;
    if (paid === null) return focusScan();
    tendered = paid.tendered;
    cardAmount = paid.card;
  }
  // Promotions need no approval; discounts someone gives do
  const overrideToken = priced.value.manual > 0 ? await approve("sales.discount") : null;
  if (overrideToken === undefined) return focusScan();
  const wholesaleOverrideToken = priceLevel.value === "wholesale" ? await approve("sales.wholesale") : null;
  if (wholesaleOverrideToken === undefined) return focusScan();
  const refundOverrideToken = returns.active.value ? await approve("sales.refund") : null;
  if (refundOverrideToken === undefined) return focusScan();
  const openPriceOverrideToken = cartItems.value.some((i) => i.openPrice !== null) ? await approve("sales.open_price") : null;
  if (openPriceOverrideToken === undefined) return focusScan();
  charging.value = true;
  try {
    const sale = await call<SaleResult>("checkout", {
      lines: cartItems.value.map((i) => ({
        productId: i.id,
        quantity: i.quantity,
        discount: i.discount,
        unitId: i.unitId,
        scaleBarcode: i.scaleBarcode,
        openPrice: i.openPrice,
        note: i.note,
      })),
      cartDiscount: cartDiscount.value,
      paymentMethod: payment.value === "account" ? "cash" : payment.value,
      tendered,
      customerId: customer.value?.id ?? null,
      onAccount,
      overLimitOverrideToken,
      overrideToken,
      exchange: returns.active.value ? returns.payload() : null,
      refundOverrideToken,
      openPriceOverrideToken,
      priceLevel: priceLevel.value,
      wholesaleOverrideToken,
      points,
      cardAmount,
    });
    lastSale.value = { kind: "sale", sale };
    usePoints.value = false;
    resetCart();
    returns.clear();
    returnMode.value = false;
    const tookCash = (payment.value === "cash" && sale.total - sale.returned - sale.pointsValue > sale.cardAmount) || (payment.value === "account" && sale.total - sale.returned - sale.pointsValue > sale.onAccount);
    if (autoPrint.value) print(sale.id, tookCash);
    await reloadProducts();
  } catch (e) {
    chargeError.value = errorMessage(e);
  } finally {
    charging.value = false;
  }
};

const refund = async () => {
  const overrideToken = await approve("sales.refund");
  if (overrideToken === undefined) return focusScan();
  charging.value = true;
  try {
    const r = await call<{ id: number; refund: number; pointsValue: number; pointsBack: number }>("create_return", {
      ...returns.payload(),
      refundMethod: payment.value === "account" || (payment.value === "card" && !cardsOn.value) ? "cash" : payment.value,
      overrideToken,
    });
    lastSale.value = { kind: "refund", amount: r.refund - r.pointsValue, pointsBack: r.pointsBack };
    returns.clear();
    returnMode.value = false;
    await reloadProducts();
  } catch (e) {
    chargeError.value = errorMessage(e);
  } finally {
    charging.value = false;
    focusScan();
  }
};

const print = (id: number, openDrawer = false) =>
  printReceipt(id, { openDrawer }).catch((e) => (chargeError.value = errorMessage(e)));

// ------------------------------------------------------------ customer display
// The second screen shows what's on the receipt: sent on every change
const displayOn = ref(false);
const logoUrl = ref<string | null>(null);
const displayState = computed<DisplayState>(() => {
  const done = lastSale.value?.kind === "sale" ? lastSale.value.sale : null;
  return {
    locale: i18n.global.locale.value as string,
    dir: (document.documentElement.dir as "ltr" | "rtl") || "ltr",
    shopName: profile.value?.name ?? "",
    logo: logoUrl.value,
    currency: currency.value,
    mode: cartItems.value.length ? "cart" : done ? "done" : "idle",
    lines: cartItems.value.map((i) => ({
      name: i.unitName ? `${i.name} (${i.unitName})` : i.name,
      qty: quantityLabel(i, unitLabel, productOf(i.id)?.unit ?? "piece"),
      total: gross(i) - i.discount - promoOf(i),
      saved: i.discount + promoOf(i),
    })),
    subtotal: subtotal.value,
    discount: discount.value,
    tax: tax.value,
    total: cartItems.value.length ? total.value : done?.total ?? 0,
    paid: done?.tendered ?? null,
    change: done?.change ?? null,
    onAccount: done?.onAccount || null,
  };
});
// Always sent (nothing happens without a display), so a display that
// opens or reloads at any moment catches up on the next change or on ready
watch(displayState, (s) => publish(s), { deep: true });
const toggleDisplay = async () => {
  try {
    if (displayOn.value) {
      await closeDisplay();
      displayOn.value = false;
    } else {
      await openDisplay();
      displayOn.value = true;
    }
  } catch (e) {
    chargeError.value = errorMessage(e);
  }
};

// ------------------------------------------------------------ keyboard
// F-keys for the common actions, Esc twice to clear, and typing anywhere
// goes to the scan field (a scanner that fires while focus is elsewhere
// still lands in the right place).
const dialogOpen = () => !!document.querySelector('[role="dialog"][data-state="open"], [role="alertdialog"][data-state="open"]');
const typingInField = (el: Element | null) =>
  !!el && (el.tagName === "TEXTAREA" || el.tagName === "SELECT" || (el.tagName === "INPUT" && (el as HTMLInputElement).type !== "checkbox" && (el as HTMLInputElement).type !== "radio") || (el as HTMLElement).isContentEditable);

const pay = (method: "cash" | "card" | "account") => {
  if (method === "account" && !(customer.value && canAccount.value)) return;
  if (method === "card" && !cardsOn.value) return;
  payment.value = method;
  checkout();
};

const onKey = (e: KeyboardEvent) => {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  if (dialogOpen()) return;
  const actions: Record<string, () => void> = {
    F1: () => (shortcutsOpen.value = !shortcutsOpen.value),
    F2: () => focusScan(),
    F3: () => (priceCheckOpen.value = true),
    F4: () => heldSales.value?.startHold(),
    F5: () => heldSales.value?.showHeld(),
    F6: () => auth.can("customers.view") && (pickerOpen.value = true),
    F7: () => cartItems.value.length && editCartDiscount(),
    F8: () => pay("cash"),
    F9: () => pay("card"),
    F10: () => pay("account"),
    F12: () => checkout(),
  };
  const action = actions[e.key];
  if (action) {
    // F5 would reload the page and F12 open the developer tools
    e.preventDefault();
    action();
    return;
  }
  if (e.key === "Escape") {
    if (shortcutsOpen.value) return void (shortcutsOpen.value = false);
    if (!cartItems.value.length) return;
    if (clearArmed.value) {
      clearArmed.value = false;
      clearCart();
    } else {
      clearArmed.value = true;
      setTimeout(() => (clearArmed.value = false), 2500);
    }
    return;
  }
  // A printable key with nothing focused: start typing in the scan field
  if (e.key.length === 1 && !typingInField(document.activeElement)) {
    scanInput.value?.focus();
  }
};

let unlisten: (() => void)[] = [];
onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
  unlisten.forEach((u) => u());
});

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  displayOn.value = await displayOpen();
  unlisten.push(await onDisplayReady(() => {
    displayOn.value = true;
    publish(displayState.value);
  }));
  if (inDesktopApp) {
    // Closing the app with a sale on the till loses it: ask first
    unlisten.push(await getCurrentWindow().onCloseRequested(async (e) => {
      if (!cartItems.value.length) return void (await closeDisplay());
      e.preventDefault();
      if (await ask(t("pos.closeWithCart"), { kind: "warning" })) {
        await closeDisplay();
        await getCurrentWindow().destroy();
      }
    }));
  }
  focusScan();
  // A new shift starts with counting the drawer
  call<{ openingCounted: boolean; openingFloat: number } | null>("shift_summary")
    .then((sum) => {
      if (sum && !sum.openingCounted) {
        openingExpected.value = sum.openingFloat;
        openingOpen.value = true;
      }
    })
    .catch(() => {});
  if (returns.active.value) returnMode.value = returns.saleId.value === null;
  const [cats, prods, , auto, running] = await Promise.all([
    listCategories(),
    listProducts(),
    loadShop(),
    getSetting("auto_print"),
    runningPromotions().catch(() => []),
    loadPayments().catch(() => {}),
  ]);
  promos.value = running;
  const logo = profile.value?.logo;
  logoUrl.value = logo ? await mediaUrl(logo).catch(() => null) : null;
  publish(displayState.value);
  categories.value = cats;
  products.value = prods;
  // Start on the quick keys when the shop has pinned some
  if (prods.some((p) => p.quickKey !== null)) pickCategory(QUICK);
  autoPrint.value = auto === "1";
  catalogLoaded.value = true;
});
</script>

<template>
  <div class="flex h-full min-h-0">
    <!-- Catalogue side -->
    <section class="flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto p-6">
      <form @submit.prevent="addItemByBarcode">
        <label for="scan" class="sr-only">{{ $t("pos.scanBarcode") }}</label>
        <div class="flex gap-2">
        <div
          class="flex h-16 min-w-0 flex-1 items-center gap-3 rounded-xl border-2 bg-card px-5 transition-colors"
          :class="
            scanError
              ? 'border-destructive'
              : returnMode
                ? 'border-amber-500 bg-amber-50 dark:bg-amber-950/30'
                : 'border-input focus-within:border-primary'
          "
        >
          <Undo2 v-if="returnMode" :size="26" :stroke-width="1.75" class="shrink-0 text-amber-600" />
          <ScanBarcode
            v-else
            :size="26"
            :stroke-width="1.75"
            class="shrink-0 text-primary"
          />
          <input
            id="scan"
            ref="scanInput"
            v-model="barcode"
            type="text"
            inputmode="numeric"
            autocomplete="off"
            :placeholder="returnMode ? $t('returnCart.scanBack') : $t('pos.barcodePlaceholder')"
            class="num h-full min-w-0 flex-1 bg-transparent text-xl tracking-wide outline-none placeholder:text-base placeholder:tracking-normal placeholder:text-muted-foreground"
            @input="notFoundCode = ''"
          />
          <kbd
            class="hidden rounded border bg-muted px-2 py-0.5 text-xs text-muted-foreground sm:block"
            >Enter</kbd
          >
        </div>
        <button
          type="button"
          class="flex h-16 shrink-0 items-center gap-2 rounded-xl border-2 px-4 text-sm font-medium transition-colors disabled:opacity-40"
          :class="returnMode ? 'border-amber-500 bg-amber-500 text-white' : 'border-input bg-card hover:border-foreground/30'"
          :aria-pressed="returnMode"
          :disabled="returns.saleId.value !== null"
          :title="$t('returnCart.toggleHint')"
          @click="returnMode = !returnMode; focusScan()"
        >
          <Undo2 :size="18" />
          {{ $t("returnCart.toggle") }}
        </button>
        <button
          type="button"
          class="flex h-16 shrink-0 items-center gap-2 rounded-xl border-2 border-input bg-card px-4 text-sm font-medium transition-colors hover:border-foreground/30"
          @click="priceCheckOpen = true"
        >
          <ScanSearch :size="18" />
          {{ $t("priceCheck.button") }}
          <kbd class="kbd">F3</kbd>
        </button>
        </div>
        <p
          v-if="scanError"
          role="alert"
          class="mt-2 text-sm font-medium text-destructive"
        >
          {{ scanError }}
        </p>
      </form>

      <div class="space-y-2">
        <div class="flex flex-wrap gap-2" role="tablist" :aria-label="$t('inventory.table.category')">
          <button
            v-if="quickProducts.length"
            type="button"
            role="tab"
            :aria-selected="activeCategory === QUICK"
            @click="pickCategory(QUICK)"
            class="flex h-10 items-center gap-1.5 rounded-full border px-4 text-sm font-medium transition-colors hover:border-foreground/30 aria-selected:border-foreground aria-selected:bg-foreground aria-selected:text-background"
          >
            <Star :size="15" />
            {{ $t("quickKeys.chip") }}
          </button>
          <button
            type="button"
            role="tab"
            :aria-selected="activeCategory === null"
            @click="pickCategory(null)"
            class="h-10 rounded-full border px-4 text-sm font-medium transition-colors hover:border-foreground/30 aria-selected:border-foreground aria-selected:bg-foreground aria-selected:text-background"
          >
            {{ $t("pos.allCategories") }}
          </button>
          <button
            v-for="c in roots"
            :key="c.id"
            type="button"
            role="tab"
            :aria-selected="activeCategory === c.id"
            @click="pickCategory(c.id)"
            class="flex h-10 items-center gap-2 rounded-full border ps-2 pe-4 text-sm font-medium transition-colors hover:border-foreground/30 aria-selected:border-foreground aria-selected:bg-foreground aria-selected:text-background"
          >
            <span
              class="grid size-7 place-items-center overflow-hidden rounded-full"
              :style="{ background: categoryTint(c.color), color: categoryInk(c.color) }"
            >
              <MediaImage v-if="c.image" :media="c.image" class="size-full" />
              <AppIcon v-else :icon="c.icon_key" :size="16" />
            </span>
            {{ c.name }}
          </button>
        </div>
        <!-- Second row: the chosen category's subcategories -->
        <div
          v-if="subcategories.length"
          class="flex flex-wrap gap-2 border-s-2 ps-3"
          role="tablist"
          :style="{ borderColor: categoryInk(roots.find((r) => r.id === activeCategory)?.color ?? null) }"
        >
          <button
            v-for="c in subcategories"
            :key="c.id"
            type="button"
            role="tab"
            :aria-selected="activeSub === c.id"
            @click="activeSub = activeSub === c.id ? null : c.id"
            class="flex h-8 items-center gap-1.5 rounded-full border px-3 text-[13px] font-medium transition-colors hover:border-foreground/30 aria-selected:border-foreground aria-selected:bg-foreground aria-selected:text-background"
          >
            <AppIcon :icon="c.icon_key" :size="14" />
            {{ c.name }}
          </button>
        </div>
      </div>

      <p
        v-if="catalogLoaded && products.length === 0"
        class="rounded-xl border border-dashed p-10 text-center text-muted-foreground"
      >
        {{ $t("pos.emptyCatalog") }}
      </p>
      <ul
        class="grid grid-cols-[repeat(auto-fill,minmax(168px,1fr))] gap-3"
      >
        <li v-for="(p, i) in visibleProducts" :key="p.id" class="group/tile relative">
          <!-- Reorder quick keys (people who edit the catalog) -->
          <div v-if="activeCategory === QUICK && auth.can('inventory.edit')" class="absolute end-2 bottom-12 z-10 hidden gap-1 group-hover/tile:flex">
            <button type="button" class="grid size-6 place-items-center rounded bg-card/90 shadow disabled:opacity-30" :disabled="i === 0" :aria-label="$t('quickKeys.earlier')" @click="moveQuick(p, -1)"><ChevronLeft :size="14" class="rtl:rotate-180" /></button>
            <button type="button" class="grid size-6 place-items-center rounded bg-card/90 shadow disabled:opacity-30" :disabled="i === visibleProducts.length - 1" :aria-label="$t('quickKeys.later')" @click="moveQuick(p, 1)"><ChevronRight :size="14" class="rtl:rotate-180" /></button>
          </div>
          <button
            type="button"
            @click="addProduct(p)"
            :disabled="stockState(p) === 'out' && !returnMode"
            class="relative flex h-40 w-full flex-col gap-2 rounded-xl p-3 text-start transition-[transform,box-shadow] hover:ring-2 hover:ring-foreground/15 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-45"
            :style="{
              background: categoryTint(p.categoryColor),
              color: categoryInk(p.categoryColor),
            }"
          >
            <span
              v-if="qtyInCart(p.id)"
              class="num absolute end-2.5 top-2.5 grid h-6 min-w-6 place-items-center rounded-full bg-foreground px-1.5 text-xs font-semibold text-background"
              >{{ qtyInCart(p.id) }}</span
            >
            <ProductThumb :product="p" :size="44" class="bg-card/60!" />
            <span
              class="line-clamp-2 text-[15px] leading-snug font-semibold text-foreground"
              ><bdi>{{ p.name }}</bdi></span
            >
            <span class="mt-auto flex items-end justify-between gap-2">
              <span v-if="p.openPrice" class="text-sm font-semibold text-foreground">{{ $t("pos.priceAtTill") }}</span>
              <span v-else class="text-lg font-semibold text-foreground">{{ formatCurrency(p.price) }}<span v-if="isWeighed(p.unit)" class="text-xs font-medium"> / {{ unitLabel(p.unit) }}</span></span>
              <Package v-if="p.units.length" :size="14" class="mb-1 shrink-0" :aria-label="$t('catalog.packs.title')" />
              <span
                class="text-xs font-medium"
                :class="{
                  'text-destructive': stockState(p) !== 'ok',
                }"
              >
                <template v-if="!p.trackStock"></template>
                <template v-else-if="stockState(p) === 'out'">{{
                  $t("pos.outOfStock")
                }}</template>
                <template v-else>{{
                  $t("pos.left", { n: p.stock })
                }}</template>
              </span>
            </span>
          </button>
        </li>
      </ul>
    </section>

    <!-- Receipt side -->
    <aside
      class="flex w-[380px] shrink-0 flex-col gap-4 border-s bg-secondary p-5"
    >
      <div
        class="receipt-edge flex min-h-0 flex-1 flex-col bg-card pb-6 shadow-[0_1px_2px_rgb(0_0_0/0.06)]"
      >
        <header class="px-5 pt-5 pb-3">
          <div class="flex items-baseline justify-between">
            <h2 class="text-base font-semibold">{{ $t("pos.receipt") }}</h2>
            <button
              type="button"
              class="ms-auto me-3 flex items-center gap-1 text-sm hover:text-foreground"
              :class="displayOn ? 'text-primary' : 'text-muted-foreground'"
              :aria-pressed="displayOn"
              :title="$t('display.toggle')"
              @click="toggleDisplay"
            >
              <MonitorSmartphone :size="15" />
            </button>
            <button
              type="button"
              class="me-3 hidden items-center gap-1 text-sm text-muted-foreground hover:text-foreground lg:flex"
              :aria-pressed="shortcutsOpen"
              @click="shortcutsOpen = !shortcutsOpen"
            >
              <Keyboard :size="15" /> F1
            </button>
            <RouterLink
              to="/main/point-of-sale/history"
              class="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
            >
              <History :size="15" /> {{ $t("history.title") }}
            </RouterLink>
          </div>
          <div class="flex items-center justify-between gap-2">
            <p class="text-sm text-muted-foreground">
              {{ $t("pos.items", itemCount) }}
            </p>
            <!-- Retail or wholesale prices for this sale -->
            <div class="grid grid-cols-2 gap-0.5 rounded-md bg-muted p-0.5 text-xs" role="radiogroup" :aria-label="$t('pricing.level')">
              <button v-for="l in ['retail', 'wholesale'] as const" :key="l" type="button" role="radio" :aria-checked="priceLevel === l"
                class="rounded px-2 py-0.5 font-medium text-muted-foreground aria-checked:bg-card aria-checked:text-foreground aria-checked:shadow-sm"
                @click="priceLevel = l">{{ $t(`pricing.levels.${l}`) }}</button>
            </div>
          </div>
          <!-- Who's buying: optional, needed for on account -->
          <div v-if="customer" class="mt-2 flex items-center gap-2 rounded-lg bg-muted px-2.5 py-1.5 text-sm">
            <UserRound :size="15" class="shrink-0 text-muted-foreground" />
            <bdi class="min-w-0 flex-1 truncate font-medium">{{ customer.name }}</bdi>
            <span v-if="customer.balance > 0" class="num text-xs font-semibold text-destructive">{{ $t("customers.owes", { amount: formatCurrency(customer.balance) }) }}</span>
            <span v-if="loyaltyOn && customer.loyalty_points > 0" class="num flex items-center gap-1 text-xs font-medium text-primary">
              <Gift :size="13" />{{ $t("loyalty.pointsShort", { n: customer.loyalty_points }) }}
            </span>
            <button type="button" class="rounded p-0.5 text-muted-foreground hover:bg-card hover:text-foreground" :aria-label="$t('customers.remove')" @click="customer = null; usePoints = false; payment === 'account' && (payment = 'cash')">
              <X :size="14" />
            </button>
          </div>
          <button
            v-else-if="auth.can('customers.view')"
            type="button"
            class="mt-2 flex items-center gap-1.5 text-sm text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
            @click="pickerOpen = true"
          >
            <UserRound :size="15" />{{ $t("customers.addToSale") }}
          </button>
        </header>
        <div class="rule-dashed mx-5" />

        <ol class="min-h-0 flex-1 overflow-y-auto px-2 py-2">
          <!-- Items coming back (a return, or the old half of an exchange) -->
          <li v-if="returns.active.value" class="mb-2 rounded-lg border border-amber-500/40 bg-amber-50 px-3 py-2.5 dark:bg-amber-950/30">
            <div class="mb-1.5 flex items-center justify-between gap-2 text-sm font-semibold text-amber-700 dark:text-amber-400">
              <span class="flex items-center gap-1.5">
                <Undo2 :size="15" />
                {{ returns.receiptNumber.value ? $t("returnCart.fromReceipt", { receipt: returns.receiptNumber.value }) : $t("returnCart.noReceipt") }}
              </span>
              <button type="button" class="text-xs font-medium underline-offset-2 hover:underline" @click="cancelReturn">{{ $t("common.cancel") }}</button>
            </div>
            <ul class="space-y-1.5">
              <li v-for="(l, i) in returns.lines.value" :key="l.saleItemId ?? `p${l.productId}`" class="text-sm">
                <div class="flex items-start gap-2">
                  <bdi class="min-w-0 flex-1 font-medium">{{ l.name }}</bdi>
                  <span class="num font-semibold">−{{ formatCurrency(roundMoney(l.unitRefund * l.quantity)) }}</span>
                </div>
                <div class="mt-1 flex items-center gap-1">
                  <button type="button" class="grid size-6 place-items-center rounded border bg-card" :aria-label="`−1 ${l.name}`" @click="stepReturn(i, -1)"><Minus :size="12" /></button>
                  <span class="num w-7 text-center text-xs font-semibold">{{ l.quantity }}</span>
                  <button type="button" class="grid size-6 place-items-center rounded border bg-card disabled:opacity-40" :disabled="l.quantity >= l.max" :aria-label="`+1 ${l.name}`" @click="stepReturn(i, 1)"><Plus :size="12" /></button>
                  <label class="ms-auto flex items-center gap-1.5 text-xs text-muted-foreground">
                    <input v-model="l.restock" type="checkbox" class="size-3.5 accent-primary" />
                    {{ $t("returns.restock") }}
                  </label>
                </div>
              </li>
            </ul>
          </li>
          <li
            v-if="cartItems.length === 0 && !returns.active.value && lastSale"
            class="flex h-full flex-col items-center justify-center gap-2 px-6 text-center"
            role="status"
          >
            <span class="grid size-12 place-items-center rounded-full bg-success/12 text-success">
              <Check :size="26" />
            </span>
            <template v-if="lastSale.kind === 'sale'">
              <span class="font-semibold">{{ $t("pos.saleDone") }}</span>
              <span class="num text-sm text-muted-foreground" dir="ltr">{{ lastSale.sale.receiptNumber }}</span>
              <span class="text-2xl font-bold">{{ formatCurrency(lastSale.sale.total) }}</span>
              <span v-if="lastSale.sale.returned" class="text-sm text-muted-foreground">
                {{ $t("returnCart.exchanged", { amount: formatCurrency(lastSale.sale.returned) }) }}
              </span>
              <span v-if="lastSale.sale.change" class="mt-2 rounded-lg bg-success/10 px-4 py-2 text-success">
                {{ $t("cash.change") }}
                <span class="num block text-3xl font-bold">{{ formatCurrency(lastSale.sale.change) }}</span>
              </span>
              <span v-if="lastSale.sale.onAccount" class="mt-2 rounded-lg bg-primary/10 px-4 py-2 text-primary">
                {{ $t("onAccount.onAccount") }}
                <span class="num block text-3xl font-bold">{{ formatCurrency(lastSale.sale.onAccount) }}</span>
              </span>
              <span v-if="lastSale.sale.refunded" class="mt-2 rounded-lg bg-amber-500/10 px-4 py-2 text-amber-700 dark:text-amber-400">
                {{ $t("returnCart.payBack") }}
                <span class="num block text-3xl font-bold">{{ formatCurrency(lastSale.sale.refunded) }}</span>
              </span>
              <span v-if="lastSale.sale.pointsUsed" class="text-sm text-muted-foreground">
                {{ $t("loyalty.spent", { n: lastSale.sale.pointsUsed, amount: formatCurrency(lastSale.sale.pointsValue) }) }}
              </span>
              <span v-if="lastSale.sale.pointsEarned" class="flex items-center gap-1 text-sm font-medium text-primary">
                <Gift :size="14" />{{ $t("loyalty.earned", { n: lastSale.sale.pointsEarned, balance: lastSale.sale.pointsBalance ?? 0 }) }}
              </span>
              <Button variant="outline" size="sm" class="mt-2" @click="print(lastSale.sale.id)">
                <Printer /> {{ $t("receipt.print") }}
              </Button>
            </template>
            <template v-else>
              <span class="font-semibold">{{ $t("returnCart.refundDone") }}</span>
              <span class="mt-2 rounded-lg bg-amber-500/10 px-4 py-2 text-amber-700 dark:text-amber-400">
                {{ $t("returnCart.payBack") }}
                <span class="num block text-3xl font-bold">{{ formatCurrency(lastSale.amount) }}</span>
              </span>
              <span v-if="lastSale.pointsBack" class="text-sm text-primary">{{ $t("loyalty.pointsBack", { n: lastSale.pointsBack }) }}</span>
            </template>
          </li>
          <li
            v-else-if="cartItems.length === 0 && !returns.active.value"
            class="grid h-full place-items-center px-6 text-center text-sm text-muted-foreground"
          >
            {{ $t("pos.emptyCart") }}
          </li>
          <li
            v-for="item in cartItems"
            :key="item.key"
            class="group rounded-lg px-3 py-2.5 transition-colors"
            :class="lastAdded === item.key ? 'bg-sun/25' : 'hover:bg-muted'"
          >
            <div class="flex items-start gap-3">
              <span class="min-w-0 flex-1 text-[15px] leading-snug font-medium">
                <bdi>{{ item.name }}</bdi>
              </span>
              <span class="text-end">
                <span class="num block font-semibold">{{ formatCurrency(gross(item) - item.discount - promoOf(item)) }}</span>
                <span v-if="item.discount || promoOf(item)" class="num block text-xs text-muted-foreground line-through">{{
                  formatCurrency(gross(item))
                }}</span>
              </span>
            </div>
            <p v-if="promoOf(item)" class="mt-0.5 text-xs font-medium text-success">
              <bdi>{{ lineOf(item)?.promoName }}</bdi> −{{ formatCurrency(promoOf(item)) }}
            </p>
            <div class="mt-1.5 flex items-center gap-1">
              <!-- A scale label's weight is fixed; other lines step or re-weigh -->
              <template v-if="!item.scaleBarcode">
                <button
                  type="button"
                  @click="decrementQuantity(item.key)"
                  :aria-label="`−1 ${item.name}`"
                  class="grid size-7 place-items-center rounded-md border bg-card hover:bg-accent"
                >
                  <Minus :size="14" />
                </button>
                <button
                  v-if="item.weighed"
                  type="button"
                  class="num min-w-8 rounded-md px-1.5 text-center text-sm font-semibold underline-offset-2 hover:underline"
                  :aria-label="$t('weigh.change')"
                  @click="editWeight(item)"
                >
                  {{ quantityLabel(item, unitLabel, productOf(item.id)?.unit ?? "kg") }}
                </button>
                <span v-else class="num w-8 text-center text-sm font-semibold">{{ item.quantity }}</span>
                <button
                  type="button"
                  @click="incrementQuantity(item.key)"
                  :aria-label="`+1 ${item.name}`"
                  class="grid size-7 place-items-center rounded-md border bg-card hover:bg-accent"
                >
                  <Plus :size="14" />
                </button>
              </template>
              <span v-else class="num text-sm font-semibold">{{ quantityLabel(item, unitLabel, productOf(item.id)?.unit ?? "kg") }}</span>
              <!-- Singles or a pack size -->
              <select
                v-if="!item.weighed && item.openPrice === null && productOf(item.id)?.units.length"
                :value="item.unitId ?? ''"
                class="ms-1 h-7 max-w-28 rounded-md border bg-card px-1 text-xs"
                :aria-label="$t('catalog.packs.title')"
                @change="switchUnit(item, ($event.target as HTMLSelectElement).value)"
              >
                <option value="">{{ $t("catalog.packs.single") }}</option>
                <option v-for="u in productOf(item.id)!.units" :key="u.id" :value="u.id">{{ u.name }} ({{ u.factor }})</option>
              </select>
              <span v-if="item.amount === null" class="num ms-2 text-xs text-muted-foreground"
                >× {{ formatCurrency(eachOf(item)) }}</span
              >
              <button
                type="button"
                @click="editLineDiscount(item)"
                :aria-label="$t('discount.title')"
                class="ms-auto grid size-7 place-items-center rounded-md transition-opacity hover:bg-accent focus-visible:opacity-100"
                :class="item.discount ? 'text-primary' : 'text-muted-foreground opacity-0 group-hover:opacity-100'"
              >
                <Percent :size="15" />
              </button>
              <button
                type="button"
                @click="removeFromCart(item.key)"
                :aria-label="$t('pos.voidLine')"
                class="grid size-7 place-items-center rounded-md text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:text-destructive focus-visible:opacity-100"
              >
                <X :size="16" />
              </button>
            </div>
          </li>
        </ol>

        <div class="rule-dashed mx-5" />
        <dl class="space-y-1.5 px-5 pt-4 text-sm">
          <div class="flex justify-between">
            <dt class="text-muted-foreground">{{ $t("pos.subtotal") }}</dt>
            <dd class="num">{{ formatCurrency(subtotal) }}</dd>
          </div>
          <div class="flex justify-between">
            <dt>
              <button
                type="button"
                class="text-muted-foreground underline-offset-2 hover:text-foreground hover:underline disabled:no-underline disabled:opacity-50"
                :disabled="cartItems.length === 0"
                @click="editCartDiscount"
              >
                {{ discount ? $t("receipt.discount") : $t("discount.add") }}
              </button>
            </dt>
            <dd v-if="discount" class="num text-primary">−{{ formatCurrency(discount) }}</dd>
          </div>
          <div v-if="taxRate > 0" class="flex justify-between">
            <dt class="text-muted-foreground">
              {{ $t("pos.tax") }} {{ taxRate }}%
            </dt>
            <dd class="num">{{ formatCurrency(tax) }}</dd>
          </div>
          <div v-if="returns.active.value" class="flex justify-between text-amber-700 dark:text-amber-400">
            <dt>{{ $t("returnCart.comingBack") }}</dt>
            <dd class="num">−{{ formatCurrency(returnValue) }}</dd>
          </div>
          <div class="flex items-baseline justify-between pt-3">
            <dt class="text-base font-semibold">{{ net < 0 ? $t("returnCart.toRefund") : $t("pos.total") }}</dt>
            <dd class="text-[2.5rem] leading-none font-bold tracking-tight">
              {{ formatCurrency(Math.abs(net)) }}
            </dd>
          </div>
        </dl>
      </div>

      <label
        v-if="pointsUsable > 0"
        class="flex cursor-pointer items-center gap-2 rounded-lg border border-dashed border-primary/40 bg-primary/5 px-3 py-2 text-sm"
      >
        <input v-model="usePoints" type="checkbox" class="size-4 accent-primary" />
        <Gift :size="16" class="text-primary" />
        <span class="flex-1">{{ $t("loyalty.usePoints", { n: pointsUsable }) }}</span>
        <span class="num font-semibold text-primary">−{{ formatCurrency(roundMoney(pointsUsable * pointValue)) }}</span>
      </label>
      <p v-if="usePoints && pointsValue > 0" class="-mt-1 flex justify-between text-sm">
        <span class="text-muted-foreground">{{ $t("loyalty.leftToPay") }}</span>
        <span class="num font-semibold">{{ formatCurrency(dueNow) }}</span>
      </p>
      <fieldset class="grid gap-2" :style="{ gridTemplateColumns: `repeat(${1 + (cardsOn ? 1 : 0) + (customer && canAccount ? 1 : 0)}, minmax(0, 1fr))` }">
        <legend class="sr-only">{{ $t("pos.payWith") }}</legend>
        <label
          v-for="m in [
            { id: 'cash', icon: Banknote },
            ...(cardsOn ? [{ id: 'card', icon: CreditCard }] : []),
            ...(customer && canAccount ? [{ id: 'account', icon: NotebookPen }] : []),
          ] as const"
          :key="m.id"
          class="flex h-12 cursor-pointer items-center justify-center gap-2 rounded-lg border bg-card text-sm font-medium transition-colors has-checked:border-foreground has-checked:bg-foreground has-checked:text-background has-focus-visible:ring-2 has-focus-visible:ring-ring"
        >
          <input
            v-model="payment"
            type="radio"
            name="payment"
            :value="m.id"
            class="sr-only"
          />
          <component :is="m.icon" :size="18" />
          {{ $t(`pos.${m.id}`) }}
          <kbd class="kbd">{{ { cash: "F8", card: "F9", account: "F10" }[m.id] }}</kbd>
        </label>
      </fieldset>

      <p v-if="chargeError" role="alert" class="text-sm font-medium text-destructive">
        {{ chargeError }}
      </p>
      <Button
        variant="sun"
        size="xl"
        class="w-full justify-between"
        :disabled="nothingToCharge || charging"
        @click="checkout"
      >
        <span class="flex items-center gap-2">
          <Loader2 v-if="charging" class="animate-spin" />
          {{ net < 0 ? $t("returnCart.refund") : $t("pos.charge") }}
          <kbd class="kbd">F12</kbd>
        </span>
        <span>{{ formatCurrency(Math.abs(dueNow)) }}</span>
      </Button>

      <div class="grid grid-cols-[1fr_auto_1fr] gap-2">
        <HeldSales
          ref="heldSales"
          :cart="heldCart"
          :can-hold="cartItems.length > 0 && !returns.active.value"
          :can-resume="cartItems.length === 0"
          @held="resetCart(); lastSale = null"
          @resume="resumeHeld"
        />
        <Button
          variant="outline"
          size="lg"
          class="min-w-0 px-3 hover:border-destructive/40 hover:bg-destructive/10 hover:text-destructive"
          :disabled="cartItems.length === 0"
          @click="clearCart"
        >
          <Trash2 />
          {{ clearArmed ? $t("shortcuts.escAgain") : $t("pos.clearCart") }}
          <kbd class="kbd">Esc</kbd>
        </Button>
      </div>
    </aside>
    <ManagerPinDialog ref="pinDialog" />
    <DiscountDialog ref="discountDialog" />
    <CashDialog ref="cashDialog" />
    <WeightDialog ref="weightDialog" />
    <OpenPriceDialog ref="openPriceDialog" />
    <PriceCheckDialog v-model:open="priceCheckOpen" />
    <CustomerPicker v-model:open="pickerOpen" @pick="pickCustomer" />
    <!-- F1: the keys, in a corner, until F1/Esc again -->
    <div
      v-if="shortcutsOpen"
      role="status"
      class="fixed bottom-6 start-24 z-40 w-72 rounded-xl border bg-card p-4 text-sm shadow-xl"
    >
      <p class="mb-2 flex items-center gap-2 font-semibold"><Keyboard :size="16" />{{ $t("shortcuts.title") }}</p>
      <dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
        <template v-for="k in ['F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', ...(cardsOn ? ['F9'] : []), 'F10', 'F12', 'Esc']" :key="k">
          <dt><kbd class="rounded border px-1.5 font-mono text-xs">{{ k }}</kbd></dt>
          <dd>{{ $t(`shortcuts.keys.${k}`) }}</dd>
        </template>
      </dl>
      <p class="mt-2 text-xs text-muted-foreground">{{ $t("shortcuts.typing") }}</p>
    </div>
    <OnAccountDialog ref="onAccountDialog" />
    <OpeningCashDialog v-model:open="openingOpen" :expected="openingExpected" @counted="focusScan()" />
  </div>
</template>
