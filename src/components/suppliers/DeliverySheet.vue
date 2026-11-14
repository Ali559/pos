<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { FileText, Loader2, ScanBarcode, X } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import EditSheet from "@/components/EditSheet.vue";
import DateInput from "@/components/pickers/DateInput.vue";
import ImageField from "@/components/media/ImageField.vue";
import { getProductById, listProducts, type Product } from "@/db/catalog";
import { listSuppliers, type Supplier } from "@/db/suppliers";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency, formatIn, formatRate } from "@/lib/format";
import { toIqd, useRate } from "@/stores/rate";
import type { MediaFile } from "@/lib/media";
import type { ScanResult } from "@/lib/scan";
import { useAuth } from "@/stores/auth";

// A delivery: many products received at once, each with its cost and
// expiry, optionally by the carton. What isn't paid now is owed to the
// supplier. A dollar supplier's delivery is invoiced in USD (costs typed in
// dollars); product costs are kept in IQD at the rate in use.
const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ supplierId?: number | null }>();
const emit = defineEmits<{ saved: [] }>();

const { t } = useI18n();
const auth = useAuth();
const canPay = computed(() => auth.can("suppliers.pay"));

interface Line {
  product: Product;
  unitId: string; // "" = single
  quantity: string;
  cost: string;
  expiry: string;
}

const suppliers = ref<Supplier[]>([]);
const supplierId = ref("");
const invoiceNumber = ref("");
const invoice = ref<MediaFile | null>(null);
const note = ref("");
const lines = ref<Line[]>([]);
const paid = ref("");
const method = ref<"till" | "cash" | "bank">("till");
const rate = useRate();
// A delivery with no supplier can be bought in dollars too
const looseCurrency = ref<"IQD" | "USD">("IQD");
const paidIn = ref<"USD" | "IQD">("USD");
const currency = computed<"IQD" | "USD">(() =>
  supplierId.value ? (suppliers.value.find((s) => String(s.id) === supplierId.value)?.currency ?? "IQD") : looseCurrency.value,
);
const usd = computed(() => currency.value === "USD");
const money = (n: number) => formatIn(n, currency.value);
const saving = ref(false);
const error = ref("");

// Finding products: a scan or a few letters of the name
const query = ref("");
const results = ref<Product[]>([]);
const searchInput = ref<HTMLInputElement | null>(null);
const findError = ref("");

watch(open, async (v) => {
  if (!v) return;
  suppliers.value = await listSuppliers();
  supplierId.value = props.supplierId ? String(props.supplierId) : "";
  invoiceNumber.value = "";
  invoice.value = null;
  note.value = "";
  lines.value = [];
  paid.value = "";
  method.value = "till";
  looseCurrency.value = "IQD";
  paidIn.value = "USD";
  rate.reload().catch(() => {});
  error.value = "";
  query.value = "";
  results.value = [];
  nextTick(() => searchInput.value?.focus());
});

let timer: ReturnType<typeof setTimeout>;
watch(query, (q) => {
  clearTimeout(timer);
  findError.value = "";
  if (q.trim().length < 2) {
    results.value = [];
    return;
  }
  timer = setTimeout(async () => {
    results.value = (await listProducts({ search: q, limit: 8 })).filter((p) => p.trackStock);
  }, 150);
});

const add = (p: Product, unitId: number | null = null) => {
  const unit = p.units.find((u) => u.id === unitId);
  const existing = lines.value.find((l) => l.product.id === p.id && l.unitId === (unit ? String(unit.id) : ""));
  if (existing) existing.quantity = String((Number(existing.quantity) || 0) + 1);
  else lines.value.push({ product: p, unitId: unit ? String(unit.id) : "", quantity: "1", cost: costFor(p, unit?.factor ?? 1), expiry: "" });
  query.value = "";
  results.value = [];
  nextTick(() => searchInput.value?.focus());
};

/** The last cost, per pack when receiving packs (in dollars at today's rate
 *  for a dollar delivery) */
const costFor = (p: Product, factor: number) => {
  if (!p.cost) return "";
  if (!usd.value) return String(p.cost * factor);
  const r = rate.per100.value;
  return r ? String(Math.round(((p.cost * factor) / r) * 10000) / 100) : "";
};
// Switching between a dinar and a dollar supplier: prices again in the new currency
watch(currency, () => lines.value.forEach((l) => (l.cost = costFor(l.product, packOf(l)?.factor ?? 1))));

// Enter: a barcode (product or pack), else the first search result
const onEnter = async () => {
  const code = query.value.trim();
  if (!code) return;
  try {
    const scan = await call<ScanResult | null>("resolve_scan", { code });
    if (scan && !scan.scaleBarcode) {
      const p = await getProductById(scan.productId);
      if (p) return add(p, scan.unitId);
    }
  } catch {
    // not a code: fall through to the name search
  }
  if (results.value[0]) add(results.value[0]);
  else findError.value = t("pos.notFound", { code });
};

const lineTotal = (l: Line) => (Number(l.quantity) || 0) * (Number(l.cost) || 0);
const total = computed(() => lines.value.reduce((n, l) => n + lineTotal(l), 0));
const paidNow = computed(() => (canPay.value ? Math.min(total.value, Math.max(0, Number(paid.value) || 0)) : 0));
const owedAfter = computed(() => total.value - paidNow.value);
const packOf = (l: Line) => l.product.units.find((u) => String(u.id) === l.unitId) ?? null;
const unitLabel = (u: string) => t(`catalog.units.${u}`).toLocaleLowerCase();

// Switching single ⇄ carton keeps the cost per single
const onUnitChange = (l: Line, unitId: string) => {
  const before = packOf(l)?.factor ?? 1;
  l.unitId = unitId;
  const after = packOf(l)?.factor ?? 1;
  if (l.cost !== "") l.cost = String(Math.round(((Number(l.cost) || 0) / before) * after * 100) / 100);
};

const save = async () => {
  saving.value = true;
  error.value = "";
  try {
    await call("receive_delivery", {
      supplierId: supplierId.value ? Number(supplierId.value) : null,
      invoiceNumber: invoiceNumber.value || null,
      invoiceMediaId: invoice.value?.id ?? null,
      note: note.value || null,
      lines: lines.value.map((l) => ({
        productId: l.product.id,
        quantity: Number(l.quantity) || 0,
        unitId: l.unitId ? Number(l.unitId) : null,
        cost: Number(l.cost) || 0,
        expiryDate: l.expiry || null,
      })),
      // Without a supplier there's nobody to owe: it's paid in full
      paid: supplierId.value ? paidNow.value : total.value,
      paidMethod: method.value,
      currency: supplierId.value ? null : looseCurrency.value,
      paidIn: usd.value ? paidIn.value : null,
      rate: usd.value ? rate.per100.value : null,
    });
    open.value = false;
    emit("saved");
  } catch (e) {
    error.value = errorMessage(e);
    if ((e as { code?: string })?.code === "rate_changed") rate.reload().catch(() => {});
  } finally {
    saving.value = false;
  }
};
</script>

<template>
  <EditSheet v-model:open="open" :title="$t('deliveries.receive')" @submit="save">
    <div class="grid gap-4 sm:grid-cols-2">
      <div class="space-y-2">
        <Label for="d-supplier">{{ $t("deliveries.supplier") }}</Label>
        <select id="d-supplier" v-model="supplierId" class="h-10 w-full rounded-md border border-input bg-card px-2 text-sm">
          <option value="">{{ $t("deliveries.noSupplier") }}</option>
          <option v-for="s in suppliers" :key="s.id" :value="String(s.id)">{{ s.name }}</option>
        </select>
      </div>
      <div v-if="!supplierId && rate.enabled.value" class="space-y-2">
        <Label for="d-currency">{{ $t("suppliers.fields.currency") }}</Label>
        <select id="d-currency" v-model="looseCurrency" class="h-10 w-full rounded-md border border-input bg-card px-2 text-sm">
          <option value="IQD">{{ $t("suppliers.currency.IQD") }}</option>
          <option value="USD">{{ $t("suppliers.currency.USD") }}</option>
        </select>
      </div>
      <div class="space-y-2">
        <Label for="d-invoice">{{ $t("deliveries.invoiceNumber") }}</Label>
        <Input id="d-invoice" v-model="invoiceNumber" dir="ltr" />
      </div>
    </div>

    <!-- Add products: scan, or type part of the name -->
    <div class="relative space-y-2">
      <Label for="d-find">{{ $t("deliveries.addProducts") }}</Label>
      <div class="relative">
        <ScanBarcode :size="18" class="pointer-events-none absolute start-3 top-1/2 -translate-y-1/2 text-primary" />
        <input
          id="d-find"
          ref="searchInput"
          v-model="query"
          autocomplete="off"
          :placeholder="$t('deliveries.findPlaceholder')"
          class="h-11 w-full rounded-md border-2 border-input bg-card ps-10 pe-3 outline-none focus-visible:border-primary"
          @keydown.enter.prevent="onEnter"
        />
      </div>
      <p v-if="findError" role="alert" class="text-sm text-destructive">{{ findError }}</p>
      <ul v-if="results.length" class="absolute inset-x-0 z-10 max-h-64 overflow-y-auto rounded-lg border bg-card shadow-lg">
        <li v-for="p in results" :key="p.id">
          <button type="button" class="flex w-full items-center justify-between gap-3 px-3 py-2 text-start text-sm hover:bg-muted" @click="add(p)">
            <bdi class="font-medium">{{ p.name }}</bdi>
            <span class="text-xs text-muted-foreground">{{ $t("deliveries.inStock", { n: p.stock }) }}</span>
          </button>
        </li>
      </ul>
    </div>

    <p v-if="!lines.length" class="rounded-lg border border-dashed p-6 text-center text-sm text-muted-foreground">{{ $t("deliveries.empty") }}</p>
    <ol v-else class="space-y-2">
      <li v-for="(l, i) in lines" :key="`${l.product.id}:${l.unitId}`" class="space-y-2 rounded-lg border p-3">
        <div class="flex items-center gap-2">
          <bdi class="min-w-0 flex-1 truncate font-medium">{{ l.product.name }}</bdi>
          <span class="num text-sm font-semibold">{{ money(lineTotal(l)) }}</span>
          <Button type="button" variant="ghost" size="icon-sm" :aria-label="$t('common.remove')" @click="lines.splice(i, 1)"><X /></Button>
        </div>
        <div class="grid grid-cols-[1fr_1fr_1.3fr] gap-2">
          <div class="space-y-1">
            <Label class="text-xs">{{ $t("stock.quantity") }}</Label>
            <div class="flex gap-1">
              <Input v-model="l.quantity" type="number" min="0" step="any" required dir="ltr" class="min-w-0" />
              <select
                v-if="l.product.units.length"
                :value="l.unitId"
                class="h-10 max-w-24 rounded-md border bg-card px-1 text-xs"
                :aria-label="$t('catalog.packs.title')"
                @change="onUnitChange(l, ($event.target as HTMLSelectElement).value)"
              >
                <option value="">{{ $t("catalog.packs.single") }}</option>
                <option v-for="u in l.product.units" :key="u.id" :value="String(u.id)">{{ u.name }} ({{ u.factor }})</option>
              </select>
            </div>
          </div>
          <div class="space-y-1">
            <Label class="text-xs">{{ packOf(l) ? $t("deliveries.costPer", { unit: packOf(l)!.name }) : $t("deliveries.cost") }}<template v-if="usd"> ($)</template></Label>
            <Input v-model="l.cost" type="number" min="0" step="any" required dir="ltr" />
          </div>
          <div class="space-y-1">
            <Label class="text-xs">{{ $t("stock.expiry") }}</Label>
            <DateInput v-model="l.expiry" clearable :placeholder="$t('stock.noExpiry')" />
          </div>
        </div>
        <p v-if="packOf(l) && Number(l.quantity) > 0" class="text-xs text-muted-foreground">
          {{ $t("deliveries.packNote", { n: Number(l.quantity) * packOf(l)!.factor, unit: unitLabel(l.product.unit) }) }}
        </p>
      </li>
    </ol>

    <div class="space-y-2">
      <Label>{{ $t("deliveries.invoicePhoto") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
      <ImageField v-model="invoice" :icon="FileText" :hint="$t('deliveries.invoicePhotoHint')" />
    </div>

    <div class="space-y-3 rounded-xl border p-4">
      <p class="flex justify-between text-lg font-semibold"><span>{{ $t("pos.total") }}</span><span class="num">{{ money(total) }}</span></p>
      <p v-if="usd && rate.per100.value" class="num -mt-2 flex justify-between text-xs text-muted-foreground">
        <span>{{ $t("cash.rate", { rate: formatRate(rate.per100.value) }) }}</span><span>≈ {{ formatCurrency(toIqd(total, rate.per100.value)) }}</span>
      </p>
      <p v-else-if="usd" role="alert" class="-mt-2 text-xs text-destructive">{{ $t("errors.no_rate") }}</p>
      <template v-if="canPay && supplierId">
        <div class="space-y-2">
          <Label for="d-paid">{{ $t("deliveries.paidNow") }}</Label>
          <Input id="d-paid" v-model="paid" type="number" min="0" step="any" dir="ltr" placeholder="0" />
        </div>
        <div v-if="paidNow > 0" class="grid grid-cols-3 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
          <button v-for="m in ['till', 'cash', 'bank'] as const" :key="m" type="button" role="radio" :aria-checked="method === m"
            class="h-9 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm" @click="method = m">
            {{ $t(`suppliers.methods.${m}`) }}
          </button>
        </div>
        <p class="flex justify-between text-sm text-muted-foreground"><span>{{ $t("deliveries.owedAfter") }}</span><span class="num">{{ money(owedAfter) }}</span></p>
      </template>
      <template v-else-if="!supplierId">
        <p class="text-sm text-muted-foreground">{{ $t("deliveries.noSupplierPaid") }}</p>
        <div class="grid grid-cols-3 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
          <button v-for="m in ['till', 'cash', 'bank'] as const" :key="m" type="button" role="radio" :aria-checked="method === m"
            class="h-9 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm" @click="method = m">
            {{ $t(`suppliers.methods.${m}`) }}
          </button>
        </div>
      </template>
      <div v-if="usd && method === 'till' && (supplierId ? paidNow > 0 : true)" class="space-y-1">
        <Label class="text-xs">{{ $t("suppliers.fromDrawer") }}</Label>
        <div class="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
          <button v-for="c in ['USD', 'IQD'] as const" :key="c" type="button" role="radio" :aria-checked="paidIn === c"
            class="h-9 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm" @click="paidIn = c">
            {{ $t(`suppliers.drawerIn.${c}`) }}
          </button>
        </div>
      </div>
    </div>

    <template #footer>
      <Button type="submit" :disabled="saving || !lines.length"><Loader2 v-if="saving" class="animate-spin" />{{ $t("deliveries.save") }}</Button>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
    </template>
  </EditSheet>
</template>
