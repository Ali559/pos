<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import {
  Banknote,
  Check,
  CreditCard,
  Loader2,
  Minus,
  PauseCircle,
  Plus,
  ScanBarcode,
  Trash2,
  X,
} from "lucide-vue-next";
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
import { useShop } from "@/stores/shop";
import { localized } from "@/utils/i18n";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";

interface CartItem {
  id: number;
  name: string;
  price: number;
  quantity: number;
}

const { t } = useI18n();
const { taxRate, load: loadShop } = useShop();

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
// The last completed sale, shown on the empty receipt until the next scan
const lastSale = ref<{ receiptNumber: string; total: number } | null>(null);
const charging = ref(false);
const chargeError = ref("");
const cartItems = ref<CartItem[]>([]);

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
const subtotal = computed(() =>
  cartItems.value.reduce((sum, i) => sum + i.price * i.quantity, 0),
);
const tax = computed(() => (subtotal.value * taxRate.value) / 100);
const total = computed(() => subtotal.value + tax.value);

// Methods
const focusScan = () => nextTick(() => scanInput.value?.focus());

const addProduct = (p: Product) => {
  if (stockState(p) === "out") return;
  lastSale.value = null;
  chargeError.value = "";
  const existing = cartItems.value.find((i) => i.id === p.id);
  if (existing) {
    if (existing.quantity + 1 > p.stock) return;
    existing.quantity++;
  }
  else
    cartItems.value.push({ id: p.id, name: localized(p, "name"), price: p.price, quantity: 1 });
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

const decrementQuantity = (id: number) => {
  const item = cartItems.value.find((i) => i.id === id);
  if (item && item.quantity > 1) item.quantity--;
  else removeFromCart(id);
};

const removeFromCart = (id: number) => {
  cartItems.value = cartItems.value.filter((i) => i.id !== id);
};

const clearCart = () => {
  cartItems.value = [];
  focusScan();
};

const reloadProducts = async () => {
  products.value = await listProducts();
};

// Rust recomputes prices from the database; only ids and quantities go over
const checkout = async () => {
  if (!cartItems.value.length || charging.value) return;
  charging.value = true;
  chargeError.value = "";
  try {
    const sale = await call<{ receiptNumber: string; total: number }>("checkout", {
      lines: cartItems.value.map((i) => ({ productId: i.id, quantity: i.quantity })),
      paymentMethod: payment.value,
    });
    lastSale.value = sale;
    clearCart();
    await reloadProducts();
  } catch (e) {
    chargeError.value = errorMessage(e);
  } finally {
    charging.value = false;
  }
};

onMounted(async () => {
  focusScan();
  [categories.value, products.value] = await Promise.all([
    listCategories(),
    listProducts(),
    loadShop(),
  ]);
  catalogLoaded.value = true;
});
</script>

<template>
  <div class="flex h-full min-h-0">
    <!-- Catalogue side -->
    <section class="flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto p-6">
      <form @submit.prevent="addItemByBarcode">
        <label for="scan" class="sr-only">{{ $t("pos.scanBarcode") }}</label>
        <div
          class="flex h-16 items-center gap-3 rounded-xl border-2 bg-card px-5 transition-colors"
          :class="
            scanError
              ? 'border-destructive'
              : 'border-input focus-within:border-primary'
          "
        >
          <ScanBarcode
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
            :placeholder="$t('pos.barcodePlaceholder')"
            class="num h-full min-w-0 flex-1 bg-transparent text-xl tracking-wide outline-none placeholder:text-base placeholder:tracking-normal placeholder:text-muted-foreground"
            @input="notFoundCode = ''"
          />
          <kbd
            class="hidden rounded border bg-muted px-2 py-0.5 text-xs text-muted-foreground sm:block"
            >Enter</kbd
          >
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
              class="grid size-7 place-items-center rounded-full"
              :style="{ background: categoryTint(c.color), color: categoryInk(c.color) }"
            >
              <AppIcon :icon="c.icon_key" :size="16" />
            </span>
            {{ localized(c, "name") }}
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
            {{ localized(c, "name") }}
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
            :disabled="stockState(p) === 'out'"
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
              ><bdi>{{ localized(p, "name") }}</bdi></span
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
            <span class="text-sm text-muted-foreground">{{ $t("pos.newSale") }}</span>
          </div>
          <p class="text-sm text-muted-foreground">
            {{ $t("pos.items", itemCount) }}
          </p>
        </header>
        <div class="rule-dashed mx-5" />

        <ol class="min-h-0 flex-1 overflow-y-auto px-2 py-2">
          <li
            v-if="cartItems.length === 0 && lastSale"
            class="flex h-full flex-col items-center justify-center gap-2 px-6 text-center"
            role="status"
          >
            <span class="grid size-12 place-items-center rounded-full bg-success/12 text-success">
              <Check :size="26" />
            </span>
            <span class="font-semibold">{{ $t("pos.saleDone") }}</span>
            <span class="num text-sm text-muted-foreground" dir="ltr">{{ lastSale.receiptNumber }}</span>
            <span class="text-2xl font-bold">{{ formatCurrency(lastSale.total) }}</span>
          </li>
          <li
            v-else-if="cartItems.length === 0"
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
              <span class="num font-semibold">{{
                formatCurrency(item.price * item.quantity)
              }}</span>
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
                @click="removeFromCart(item.id)"
                :aria-label="$t('settings.delete')"
                class="ms-auto grid size-7 place-items-center rounded-md text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:text-destructive focus-visible:opacity-100"
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
          <div v-if="taxRate > 0" class="flex justify-between">
            <dt class="text-muted-foreground">
              {{ $t("pos.tax") }} {{ taxRate }}%
            </dt>
            <dd class="num">{{ formatCurrency(tax) }}</dd>
          </div>
          <div class="flex items-baseline justify-between pt-3">
            <dt class="text-base font-semibold">{{ $t("pos.total") }}</dt>
            <dd class="text-[2.5rem] leading-none font-bold tracking-tight">
              {{ formatCurrency(total) }}
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
        :disabled="cartItems.length === 0 || charging"
        @click="checkout"
      >
        <span class="flex items-center gap-2">
          <Loader2 v-if="charging" class="animate-spin" />
          {{ $t("pos.charge") }}
        </span>
        <span>{{ formatCurrency(total) }}</span>
      </Button>

      <div class="grid grid-cols-2 gap-2">
        <Button
          variant="outline"
          size="lg"
          :disabled="cartItems.length === 0"
        >
          <PauseCircle />
          {{ $t("pos.hold") }}
        </Button>
        <Button
          variant="outline"
          size="lg"
          class="hover:border-destructive/40 hover:bg-destructive/10 hover:text-destructive"
          :disabled="cartItems.length === 0"
          @click="clearCart"
        >
          <Trash2 />
          {{ $t("pos.clearCart") }}
        </Button>
      </div>
    </aside>
  </div>
</template>
