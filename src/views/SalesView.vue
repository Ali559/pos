<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  Banknote,
  Check,
  CreditCard,
  History,
  Loader2,
  Minus,
  Percent,
  Plus,
  Printer,
  ScanBarcode,
  Trash2,
  Undo2,
  X,
} from "lucide-vue-next";
import HeldSales, { type HeldCart } from "@/components/sales/HeldSales.vue";
import OpeningCashDialog from "@/components/sales/OpeningCashDialog.vue";
import { useReturnCart } from "@/stores/returnCart";
import { RouterLink } from "vue-router";
import CashDialog from "@/components/sales/CashDialog.vue";
import DiscountDialog from "@/components/sales/DiscountDialog.vue";
import ManagerPinDialog from "@/components/sales/ManagerPinDialog.vue";
import { getSetting } from "@/db/settings";
import { printReceipt } from "@/lib/print";
import { useAuth } from "@/stores/auth";
import {
  findProductByCode,
  listCategories,
  listProducts,
  stockState,
  type Category,
  type Product,
} from "@/db/catalog";
import { formatCurrency } from "@/lib/format";
import { categoryInk, categoryTint } from "@/lib/categoryColor";
import AppIcon from "@/components/icons/AppIcon.vue";
import ProductThumb from "@/components/catalog/ProductThumb.vue";
import MediaImage from "@/components/media/MediaImage.vue";
import { useShop } from "@/stores/shop";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";

interface CartItem {
  id: number;
  name: string;
  price: number;
  quantity: number;
  /** Off this line, as an amount */
  discount: number;
}

interface SaleResult {
  id: number;
  receiptNumber: string;
  total: number;
  change: number | null;
  /** Exchange: value of what came back, and what was paid back on top */
  returned: number;
  refunded: number;
}

/** What the done panel shows: a sale (maybe an exchange) or a plain refund */
type Done = { kind: "sale"; sale: SaleResult } | { kind: "refund"; amount: number };

const { t } = useI18n();
const { taxRate, currency, load: loadShop } = useShop();
const auth = useAuth();
const pinDialog = ref<InstanceType<typeof ManagerPinDialog> | null>(null);
const discountDialog = ref<InstanceType<typeof DiscountDialog> | null>(null);
const cashDialog = ref<InstanceType<typeof CashDialog> | null>(null);
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
const payment = ref<"cash" | "card">("cash");
const lastAdded = ref<number | null>(null);
// The last completed sale or refund, shown on the empty receipt until the next scan
const lastSale = ref<Done | null>(null);
const charging = ref(false);
const chargeError = ref("");
const cartItems = ref<CartItem[]>([]);
const cartDiscount = ref(0);

// Computed
const roots = computed(() => categories.value.filter((c) => c.parent_id === null));
const subcategories = computed(() =>
  activeCategory.value === null
    ? []
    : categories.value.filter((c) => c.parent_id === activeCategory.value),
);
const pickCategory = (id: number | null) => {
  activeCategory.value = id;
  activeSub.value = null;
};

const visibleProducts = computed(() =>
  activeSub.value !== null
    ? products.value.filter((p) => p.categoryId === activeSub.value)
    : activeCategory.value !== null
      ? products.value.filter((p) => p.rootCategoryId === activeCategory.value)
      : products.value,
);

const qtyInCart = (id: number) =>
  cartItems.value.find((i) => i.id === id)?.quantity ?? 0;

const itemCount = computed(() =>
  cartItems.value.reduce((n, i) => n + i.quantity, 0),
);
// Mirrors the Rust totals (sales::checkout), which are what gets charged
const roundMoney = (n: number) => (currency.value === "IQD" ? Math.round(n) : Math.round(n * 100) / 100);
const subtotal = computed(() =>
  cartItems.value.reduce((sum, i) => sum + i.price * i.quantity, 0),
);
const lineDiscounts = computed(() => cartItems.value.reduce((sum, i) => sum + i.discount, 0));
const discount = computed(() => lineDiscounts.value + cartDiscount.value);
const tax = computed(() => roundMoney(((subtotal.value - discount.value) * taxRate.value) / 100));
const total = computed(() => roundMoney(subtotal.value - discount.value) + tax.value);
const returnValue = computed(() => returns.total(currency.value));
// What the customer pays (positive) or gets back (negative)
const net = computed(() => (cartItems.value.length ? total.value : 0) - returnValue.value);
const nothingToCharge = computed(() => !cartItems.value.length && !returns.active.value);

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

const addProduct = (p: Product) => {
  if (returnMode.value) return addToReturn(p);
  if (stockState(p) === "out") return;
  lastSale.value = null;
  chargeError.value = "";
  const existing = cartItems.value.find((i) => i.id === p.id);
  if (existing) {
    if (existing.quantity + 1 > p.stock) return;
    existing.quantity++;
  }
  else
    cartItems.value.push({ id: p.id, name: p.name, price: p.price, quantity: 1, discount: 0 });
  lastAdded.value = p.id;
  setTimeout(() => {
    if (lastAdded.value === p.id) lastAdded.value = null;
  }, 600);
};

const addItemByBarcode = async () => {
  const code = barcode.value.trim();
  if (!code) return;
  const product = await findProductByCode(code);
  if (!product) {
    notFoundCode.value = code;
    return;
  }
  notFoundCode.value = "";
  addProduct(product);
  barcode.value = "";
};

const stockOf = (id: number) => products.value.find((p) => p.id === id)?.stock ?? Infinity;

const incrementQuantity = (id: number) => {
  const item = cartItems.value.find((i) => i.id === id);
  if (item && item.quantity + 1 <= stockOf(id)) item.quantity++;
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

const decrementQuantity = (id: number) => {
  const item = cartItems.value.find((i) => i.id === id);
  if (!item) return;
  if (item.quantity <= 1) return removeFromCart(id);
  runVoid([{ id, quantity: 1 }], () => {
    item.quantity--;
    item.discount = Math.min(item.discount, item.price * item.quantity);
  });
};

const removeFromCart = (id: number) => {
  const item = cartItems.value.find((i) => i.id === id);
  if (!item) return;
  runVoid([{ id, quantity: item.quantity }], () => {
    cartItems.value = cartItems.value.filter((i) => i.id !== id);
    if (!cartItems.value.length) cartDiscount.value = 0;
  });
};

const resetCart = () => {
  cartItems.value = [];
  cartDiscount.value = 0;
  focusScan();
};

const clearCart = () => runVoid(cartItems.value.map((i) => ({ id: i.id, quantity: i.quantity })), resetCart);

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
const heldCart = computed<HeldCart>(() => ({ items: cartItems.value, cartDiscount: cartDiscount.value }));
const resumeHeld = (cart: HeldCart) => {
  lastSale.value = null;
  cartItems.value = cart.items.map((i) => ({ ...i }));
  cartDiscount.value = cart.cartDiscount;
  focusScan();
};

const editLineDiscount = async (item: CartItem) => {
  const n = await discountDialog.value?.ask({ title: item.name, base: item.price * item.quantity, current: item.discount });
  if (n != null) item.discount = roundMoney(n);
  focusScan();
};

const editCartDiscount = async () => {
  const n = await discountDialog.value?.ask({
    title: t("discount.wholeSale"),
    base: subtotal.value - lineDiscounts.value,
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
  if (payment.value === "cash" && net.value > 0) {
    tendered = (await cashDialog.value?.ask(net.value)) ?? null;
    if (tendered === null) return focusScan();
  }
  const overrideToken = discount.value > 0 ? await approve("sales.discount") : null;
  if (overrideToken === undefined) return focusScan();
  const refundOverrideToken = returns.active.value ? await approve("sales.refund") : null;
  if (refundOverrideToken === undefined) return focusScan();
  charging.value = true;
  try {
    const sale = await call<SaleResult>("checkout", {
      lines: cartItems.value.map((i) => ({ productId: i.id, quantity: i.quantity, discount: i.discount })),
      cartDiscount: cartDiscount.value,
      paymentMethod: payment.value,
      tendered,
      overrideToken,
      exchange: returns.active.value ? returns.payload() : null,
      refundOverrideToken,
    });
    lastSale.value = { kind: "sale", sale };
    resetCart();
    returns.clear();
    returnMode.value = false;
    if (autoPrint.value) print(sale.id, payment.value === "cash");
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
    const r = await call<{ id: number; refund: number }>("create_return", {
      ...returns.payload(),
      refundMethod: payment.value,
      overrideToken,
    });
    lastSale.value = { kind: "refund", amount: r.refund };
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

onMounted(async () => {
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
  const [cats, prods, , auto] = await Promise.all([
    listCategories(),
    listProducts(),
    loadShop(),
    getSetting("auto_print"),
  ]);
  categories.value = cats;
  products.value = prods;
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
        <li v-for="p in visibleProducts" :key="p.id">
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
              <span class="text-lg font-semibold text-foreground">{{
                formatCurrency(p.price)
              }}</span>
              <span
                class="text-xs font-medium"
                :class="{
                  'text-destructive': stockState(p) !== 'ok',
                }"
              >
                <template v-if="stockState(p) === 'out'">{{
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
            <RouterLink
              to="/main/point-of-sale/history"
              class="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
            >
              <History :size="15" /> {{ $t("history.title") }}
            </RouterLink>
          </div>
          <p class="text-sm text-muted-foreground">
            {{ $t("pos.items", itemCount) }}
          </p>
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
              <span v-if="lastSale.sale.refunded" class="mt-2 rounded-lg bg-amber-500/10 px-4 py-2 text-amber-700 dark:text-amber-400">
                {{ $t("returnCart.payBack") }}
                <span class="num block text-3xl font-bold">{{ formatCurrency(lastSale.sale.refunded) }}</span>
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
            :key="item.id"
            class="group rounded-lg px-3 py-2.5 transition-colors"
            :class="lastAdded === item.id ? 'bg-sun/25' : 'hover:bg-muted'"
          >
            <div class="flex items-start gap-3">
              <span class="min-w-0 flex-1 text-[15px] leading-snug font-medium">
                <bdi>{{ item.name }}</bdi>
              </span>
              <span class="text-end">
                <span class="num block font-semibold">{{ formatCurrency(item.price * item.quantity - item.discount) }}</span>
                <span v-if="item.discount" class="num block text-xs text-muted-foreground line-through">{{
                  formatCurrency(item.price * item.quantity)
                }}</span>
              </span>
            </div>
            <div class="mt-1.5 flex items-center gap-1">
              <button
                type="button"
                @click="decrementQuantity(item.id)"
                :aria-label="`−1 ${item.name}`"
                class="grid size-7 place-items-center rounded-md border bg-card hover:bg-accent"
              >
                <Minus :size="14" />
              </button>
              <span class="num w-8 text-center text-sm font-semibold">{{
                item.quantity
              }}</span>
              <button
                type="button"
                @click="incrementQuantity(item.id)"
                :aria-label="`+1 ${item.name}`"
                class="grid size-7 place-items-center rounded-md border bg-card hover:bg-accent"
              >
                <Plus :size="14" />
              </button>
              <span class="num ms-2 text-xs text-muted-foreground"
                >× {{ formatCurrency(item.price) }}</span
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
                @click="removeFromCart(item.id)"
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

      <fieldset class="grid grid-cols-2 gap-2">
        <legend class="sr-only">{{ $t("pos.payWith") }}</legend>
        <label
          v-for="m in [
            { id: 'cash', icon: Banknote },
            { id: 'card', icon: CreditCard },
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
        </span>
        <span>{{ formatCurrency(Math.abs(net)) }}</span>
      </Button>

      <div class="grid grid-cols-[1fr_auto_1fr] gap-2">
        <HeldSales
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
          {{ $t("pos.clearCart") }}
        </Button>
      </div>
    </aside>
    <ManagerPinDialog ref="pinDialog" />
    <DiscountDialog ref="discountDialog" />
    <CashDialog ref="cashDialog" />
    <OpeningCashDialog v-model:open="openingOpen" :expected="openingExpected" @counted="focusScan()" />
  </div>
</template>
