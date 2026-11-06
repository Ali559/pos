<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ClipboardCheck, ClipboardList, ScanBarcode, X } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { Category } from "@/db/catalog";
import { getCountLines, listStockCounts, type CountLine, type StockCount } from "@/db/suppliers";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatDateTime } from "@/lib/locale";
import type { ScanResult } from "@/lib/scan";
import { useAuth } from "@/stores/auth";

// A stock count: pick a shelf/category (or everything), scan or type what's
// there, see the differences, apply them in one go. Open counts can be
// continued later.
const props = defineProps<{ categories: Category[] }>();
const emit = defineEmits<{ applied: [] }>();
const { t } = useI18n();
const auth = useAuth();

const counts = ref<StockCount[]>([]);
const current = ref<StockCount | null>(null);
const lines = ref<CountLine[]>([]);
const error = ref("");
const message = ref("");

const loadCounts = async () => {
  counts.value = await listStockCounts();
};
onMounted(loadCounts);

// Starting one
const newName = ref("");
const newCategory = ref("");
const start = async () => {
  error.value = "";
  try {
    const id = await call<number>("start_stock_count", { name: newName.value || null, categoryId: newCategory.value ? Number(newCategory.value) : null });
    newName.value = "";
    await loadCounts();
    const c = counts.value.find((x) => x.id === id);
    if (c) await openCount(c);
  } catch (e) {
    error.value = errorMessage(e);
  }
};

const openCount = async (c: StockCount) => {
  current.value = c;
  message.value = "";
  lines.value = await getCountLines(c.id, c.category_id);
  nextTick(() => scanInput.value?.focus());
};
const reload = async () => {
  if (current.value) lines.value = await getCountLines(current.value.id, current.value.category_id);
};

// Counting: drafts per product until saved
const drafts = ref<Record<number, string>>({});
const record = async (l: CountLine, value?: number) => {
  if (!current.value) return;
  const raw = value ?? Number(drafts.value[l.product_id]);
  if (!Number.isFinite(raw) || (value === undefined && drafts.value[l.product_id] === "")) return;
  error.value = "";
  try {
    const r = await call<{ expected: number; counted: number }>("record_count", { countId: current.value.id, productId: l.product_id, counted: raw });
    l.counted = r.counted;
    l.expected = r.expected;
    delete drafts.value[l.product_id];
  } catch (e) {
    error.value = errorMessage(e);
  }
};
const uncount = async (l: CountLine) => {
  if (!current.value) return;
  await call("remove_count_item", { countId: current.value.id, productId: l.product_id });
  l.counted = null;
  l.expected = null;
};

// Scanning adds one to that product's count
const scan = ref("");
const scanInput = ref<HTMLInputElement | null>(null);
const onScan = async () => {
  const code = scan.value.trim();
  if (!code || !current.value) return;
  scan.value = "";
  try {
    const r = await call<ScanResult | null>("resolve_scan", { code });
    let l = r ? lines.value.find((x) => x.product_id === r.productId) : undefined;
    if (r && !l) {
      // Outside this count's category: count it anyway
      await call("record_count", { countId: current.value.id, productId: r.productId, counted: 0 });
      await reload();
      l = lines.value.find((x) => x.product_id === r.productId);
    }
    if (!r || !l) {
      error.value = t("pos.notFound", { code });
      return;
    }
    const step = r.scaleBarcode ? r.quantity : r.unitId ? (await packFactor(r.unitId)) : 1;
    await record(l, (l.counted ?? 0) + step);
  } catch (e) {
    error.value = errorMessage(e);
  }
};
const packFactor = async (unitId: number) => {
  const { select } = await import("@/db/client");
  return (await select<{ factor: number }>("SELECT factor FROM product_units WHERE id = $1", [unitId]))[0]?.factor ?? 1;
};

const fmt = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(3).replace(/0+$/, ""));
const diff = (l: CountLine) => (l.counted ?? 0) - (l.expected ?? 0);
const counted = computed(() => lines.value.filter((l) => l.counted !== null));
const differences = computed(() => counted.value.filter((l) => Math.abs(diff(l)) > 1e-9));
const filter = ref<"all" | "todo" | "diff">("todo");
const visible = computed(() =>
  filter.value === "todo" ? lines.value.filter((l) => l.counted === null) : filter.value === "diff" ? differences.value : lines.value,
);

const apply = async () => {
  if (!current.value) return;
  error.value = "";
  try {
    const r = await call<{ changed: number; gained: number; lost: number }>("apply_stock_count", { countId: current.value.id });
    message.value = t("stockCount.applied", { n: r.changed, gained: fmt(r.gained), lost: fmt(r.lost) });
    current.value = null;
    await loadCounts();
    emit("applied");
  } catch (e) {
    error.value = errorMessage(e);
  }
};
const cancel = async () => {
  if (!current.value) return;
  await call("cancel_stock_count", { countId: current.value.id });
  current.value = null;
  await loadCounts();
};
</script>

<template>
  <div class="space-y-5">
    <p v-if="message" role="status" class="rounded-lg bg-success/10 p-3 text-sm text-success">{{ message }}</p>
    <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>

    <!-- List of counts, and starting a new one -->
    <template v-if="!current">
      <form class="flex flex-wrap items-end gap-3 rounded-xl border bg-card p-5" @submit.prevent="start">
        <div class="space-y-2">
          <Label for="sc-name">{{ $t("stockCount.name") }}</Label>
          <Input id="sc-name" v-model="newName" dir="auto" :placeholder="$t('stockCount.namePlaceholder')" class="w-64" />
        </div>
        <div class="space-y-2">
          <Label for="sc-cat">{{ $t("inventory.table.category") }}</Label>
          <select id="sc-cat" v-model="newCategory" class="h-10 w-56 rounded-md border border-input bg-card px-2 text-sm">
            <option value="">{{ $t("stockCount.everything") }}</option>
            <option v-for="c in props.categories.filter((c) => c.is_active)" :key="c.id" :value="String(c.id)">{{ c.parent_id ? "— " : "" }}{{ c.name }}</option>
          </select>
        </div>
        <Button type="submit"><ClipboardList />{{ $t("stockCount.start") }}</Button>
      </form>
      <ul v-if="counts.length" class="divide-y rounded-xl border bg-card">
        <li v-for="c in counts" :key="c.id" class="flex items-center gap-3 px-5 py-3">
          <div class="min-w-0 flex-1">
            <p class="font-medium"><bdi>{{ c.name ?? $t("stockCount.untitled", { n: c.id }) }}</bdi><span v-if="c.category" class="ms-2 text-sm text-muted-foreground"><bdi>{{ c.category }}</bdi></span></p>
            <p class="text-xs text-muted-foreground">{{ formatDateTime(new Date(c.started_at)) }} · {{ c.user }} · {{ $t("stockCount.itemsCounted", { n: c.items }) }}</p>
          </div>
          <span class="rounded-md px-2 py-0.5 text-xs font-semibold" :class="c.status === 'open' ? 'bg-warning/15 text-warning' : c.status === 'applied' ? 'bg-success/12 text-success' : 'bg-muted text-muted-foreground'">{{ $t(`stockCount.status.${c.status}`) }}</span>
          <Button v-if="c.status === 'open'" size="sm" variant="outline" @click="openCount(c)">{{ $t("stockCount.continue") }}</Button>
        </li>
      </ul>
    </template>

    <!-- Counting -->
    <template v-else>
      <div class="flex flex-wrap items-center gap-3">
        <div class="min-w-0 flex-1">
          <h2 class="text-lg font-semibold"><bdi>{{ current.name ?? $t("stockCount.untitled", { n: current.id }) }}</bdi></h2>
          <p class="text-sm text-muted-foreground">{{ $t("stockCount.progress", { done: counted.length, total: lines.length, diff: differences.length }) }}</p>
        </div>
        <Button variant="ghost" @click="current = null">{{ $t("stockCount.later") }}</Button>
        <Button variant="ghost" class="text-destructive" @click="cancel">{{ $t("stockCount.cancel") }}</Button>
        <Button v-if="auth.can('inventory.adjust_stock')" :disabled="!counted.length" @click="apply"><ClipboardCheck />{{ $t("stockCount.apply", { n: differences.length }) }}</Button>
      </div>
      <form class="relative" @submit.prevent="onScan">
        <ScanBarcode :size="20" class="pointer-events-none absolute start-4 top-1/2 -translate-y-1/2 text-primary" />
        <input ref="scanInput" v-model="scan" autocomplete="off" inputmode="numeric" :placeholder="$t('stockCount.scanHint')" :aria-label="$t('pos.scanBarcode')"
          class="h-14 w-full rounded-xl border-2 border-input bg-card ps-12 pe-4 text-lg outline-none focus:border-primary" />
      </form>
      <div class="grid grid-cols-3 gap-1 rounded-lg bg-muted p-1 sm:w-96" role="radiogroup">
        <button v-for="f in ['todo', 'diff', 'all'] as const" :key="f" type="button" role="radio" :aria-checked="filter === f"
          class="h-8 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm" @click="filter = f">{{ $t(`stockCount.filter.${f}`) }}</button>
      </div>
      <div class="overflow-hidden rounded-xl border bg-card">
        <table class="w-full text-sm">
          <thead class="border-b text-xs text-muted-foreground">
            <tr>
              <th class="px-5 py-2.5 text-start font-medium">{{ $t("catalog.productName") }}</th>
              <th class="px-3 py-2.5 text-end font-medium">{{ $t("stockCount.system") }}</th>
              <th class="px-3 py-2.5 font-medium">{{ $t("stockCount.counted") }}</th>
              <th class="px-5 py-2.5 text-end font-medium">{{ $t("stockCount.difference") }}</th>
            </tr>
          </thead>
          <tbody class="divide-y">
            <tr v-if="!visible.length"><td colspan="4" class="py-10 text-center text-muted-foreground">{{ $t("stockCount.nothingHere") }}</td></tr>
            <tr v-for="l in visible" :key="l.product_id">
              <td class="px-5 py-2"><bdi class="font-medium">{{ l.name }}</bdi></td>
              <td class="num px-3 py-2 text-end text-muted-foreground">{{ fmt(l.expected ?? l.stock) }}</td>
              <td class="px-3 py-2">
                <form class="flex items-center gap-1" @submit.prevent="record(l)">
                  <Input
                    :model-value="drafts[l.product_id] ?? (l.counted !== null ? fmt(l.counted) : '')"
                    type="number" min="0" step="any" dir="ltr" class="h-9 w-24"
                    :aria-label="$t('stockCount.counted')"
                    @update:model-value="(v) => (drafts[l.product_id] = String(v))"
                    @blur="drafts[l.product_id] !== undefined && record(l)"
                  />
                  <button v-if="l.counted !== null" type="button" class="rounded p-1 text-muted-foreground hover:text-destructive" :aria-label="$t('common.remove')" @click="uncount(l)"><X :size="14" /></button>
                </form>
              </td>
              <td class="num px-5 py-2 text-end font-semibold" :class="l.counted === null ? 'text-muted-foreground' : diff(l) < 0 ? 'text-destructive' : diff(l) > 0 ? 'text-success' : ''">
                {{ l.counted === null ? "—" : diff(l) > 0 ? `+${fmt(diff(l))}` : fmt(diff(l)) }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </template>
  </div>
</template>
