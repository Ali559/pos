<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { Download, Loader2 } from "lucide-vue-next";
import Chart from "@/components/data/Chart.vue";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import AppIcon from "@/components/icons/AppIcon.vue";
import RangePicker, { resolveRange, type RangeModel } from "@/components/reports/RangePicker.vue";
import {
  bucketFor,
  bucketKeys,
  categorySales,
  employeeStats,
  expensesByType,
  heatmap,
  paymentSplit,
  productSales,
  salesUsers,
  series,
  slowMovers,
  summary,
  type EmployeeRow,
  type ExpenseSum,
  type Filters,
  type ProductRow,
  type Summary,
} from "@/db/analytics";
import { listCategories, type Category } from "@/db/catalog";
import { categoryInk } from "@/lib/categoryColor";
import { previousRange, rangeDays, toLocalDate, type Preset } from "@/lib/dateRange";
import { errorMessage } from "@/lib/errors";
import { exportCsv } from "@/lib/exportCsv";
import { formatCurrency } from "@/lib/format";
import { useAuth } from "@/stores/auth";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const auth = useAuth();
const canExport = computed(() => auth.can("analytics.export"));

// ------------------------------------------------ filters (in the route)

const q = (k: string) => (typeof route.query[k] === "string" ? (route.query[k] as string) : undefined);
const range = ref<RangeModel>({ preset: (q("range") as Preset) ?? "thisWeek", from: q("from"), to: q("to") });
const compare = ref(q("compare") !== "0");
const userId = ref(q("user") ?? "all");
const categoryId = ref(q("category") ?? "all");
const payment = ref(q("payment") ?? "all");
const tab = ref(q("tab") ?? "overview");

watch([range, compare, userId, categoryId, payment, tab], () => {
  router.replace({
    query: {
      range: range.value.preset,
      ...(range.value.preset === "custom" ? { from: range.value.from, to: range.value.to } : {}),
      compare: compare.value ? "1" : "0",
      ...(userId.value !== "all" ? { user: userId.value } : {}),
      ...(categoryId.value !== "all" ? { category: categoryId.value } : {}),
      ...(payment.value !== "all" ? { payment: payment.value } : {}),
      tab: tab.value,
    },
  });
}, { deep: true });

const filters = computed<Filters>(() => ({
  range: resolveRange(range.value),
  userId: userId.value === "all" ? null : Number(userId.value),
  categoryId: categoryId.value === "all" ? null : Number(categoryId.value),
  payment: payment.value === "all" ? null : (payment.value as "cash" | "card"),
}));
const prevFilters = computed<Filters>(() => ({ ...filters.value, range: previousRange(filters.value.range) }));
// Net profit only makes sense for the whole shop
const narrowed = computed(() => !!(filters.value.userId || filters.value.categoryId || filters.value.payment));

// ------------------------------------------------------------- data

const users = ref<{ id: number; full_name: string }[]>([]);
const categories = ref<Category[]>([]);
const loading = ref(false);
const error = ref("");

const cur = ref<Summary | null>(null);
const prev = ref<Summary | null>(null);
const points = ref<{ labels: string[]; now: number[]; before: number[] }>({ labels: [], now: [], before: [] });
const heat = ref<Map<string, number>>(new Map());
const products = ref<ProductRow[]>([]);
const slow = ref<Awaited<ReturnType<typeof slowMovers>>>([]);
const cats = ref<Awaited<ReturnType<typeof categorySales>>>([]);
const pays = ref<Awaited<ReturnType<typeof paymentSplit>>>([]);
const staff = ref<EmployeeRow[]>([]);
const expenses = ref<ExpenseSum[]>([]);

const label = (key: string, bucket: string) => {
  if (bucket === "hour") return `${key.slice(11)}:00`;
  if (bucket === "month") return `${key.slice(5, 7)}/${key.slice(0, 4)}`;
  return `${key.slice(8, 10)}/${key.slice(5, 7)}`;
};

const load = async () => {
  loading.value = true;
  error.value = "";
  try {
    const f = filters.value;
    const p = prevFilters.value;
    const bucket = bucketFor(rangeDays(f.range));
    const [a, b, s, sp, h, pr, sl, c, py, st, ex] = await Promise.all([
      summary(f), summary(p), series(f, bucket), series(p, bucket), heatmap(f),
      productSales(f), slowMovers(f), categorySales(f), paymentSplit(f), employeeStats(f),
      expensesByType(f.range),
    ]);
    cur.value = a;
    prev.value = b;
    const keys = bucketKeys(f.range, bucket);
    const prevKeys = bucketKeys(p.range, bucket);
    const byKey = new Map(s.map((r) => [r.bucket, r.revenue]));
    const byPrev = new Map(sp.map((r) => [r.bucket, r.revenue]));
    points.value = {
      labels: keys.map((k) => label(k, bucket)),
      now: keys.map((k) => byKey.get(k) ?? 0),
      before: keys.map((_, i) => byPrev.get(prevKeys[i]) ?? 0),
    };
    heat.value = new Map(h.map((r) => [`${r.weekday}-${r.hour}`, r.transactions]));
    products.value = pr;
    slow.value = sl;
    cats.value = c;
    pays.value = py;
    staff.value = st;
    expenses.value = ex;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
};

onMounted(async () => {
  [users.value, categories.value] = await Promise.all([salesUsers(), listCategories()]);
  await load();
});
watch(filters, load, { deep: true });

// ------------------------------------------------------- derived

const kpis = computed(() => {
  const a = cur.value;
  const b = prev.value;
  if (!a || !b) return [];
  const avg = (s: Summary) => (s.transactions ? s.revenue / s.transactions : 0);
  const margin = (s: Summary) => s.revenue - s.cost;
  const pct = (x: number, y: number) => (y ? ((x - y) / y) * 100 : null);
  return [
    { key: "revenue", value: formatCurrency(a.revenue), delta: pct(a.revenue, b.revenue) },
    { key: "transactions", value: String(a.transactions), delta: pct(a.transactions, b.transactions) },
    { key: "avgBasket", value: formatCurrency(avg(a)), delta: pct(avg(a), avg(b)) },
    {
      key: "grossMargin",
      value: formatCurrency(margin(a)),
      sub: a.revenue ? t("analytics.ofRevenue", { n: Math.round((margin(a) / a.revenue) * 100) }) : "",
      delta: pct(margin(a), margin(b)),
    },
  ];
});

const css = getComputedStyle(document.documentElement);
const token = (n: string) => css.getPropertyValue(`--${n}`).trim();
const axis = { border: { display: false }, ticks: { color: token("muted-foreground"), font: { family: "inherit" } } };

const chartData = computed(() => ({
  labels: points.value.labels,
  datasets: [
    {
      label: t("analytics.kpi.revenue"),
      data: points.value.now,
      borderColor: token("chart-1"),
      backgroundColor: `${token("chart-1")}1f`,
      borderWidth: 2.5,
      tension: 0.3,
      fill: true,
      pointRadius: 0,
      pointHoverRadius: 5,
    },
    ...(compare.value
      ? [{
          label: t("analytics.previousPeriod"),
          data: points.value.before,
          borderColor: token("muted-foreground"),
          borderDash: [5, 5],
          borderWidth: 1.5,
          tension: 0.3,
          fill: false,
          pointRadius: 0,
        }]
      : []),
  ],
}));
const chartOptions = {
  interaction: { mode: "index" as const, intersect: false },
  plugins: { legend: { display: false } },
  scales: {
    y: { ...axis, beginAtZero: true, grid: { color: token("border") } },
    x: { ...axis, grid: { display: false }, ticks: { ...axis.ticks, maxTicksLimit: 12 } },
  },
};

// Heatmap: Saturday first, only the hours the shop actually trades
const WEEKDAYS = [6, 0, 1, 2, 3, 4, 5];
const dayKeys = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
const hours = computed(() => {
  const used = [...heat.value.keys()].map((k) => Number(k.split("-")[1]));
  if (!used.length) return [];
  const lo = Math.min(...used);
  const hi = Math.max(...used);
  return Array.from({ length: hi - lo + 1 }, (_, i) => lo + i);
});
const heatMax = computed(() => Math.max(1, ...heat.value.values()));
const heatColor = (n: number) =>
  n ? `color-mix(in oklab, var(--primary) ${Math.round(15 + (n / heatMax.value) * 85)}%, var(--card))` : "var(--muted)";

const share = <T extends { revenue?: number; amount?: number }>(rows: T[], key: "revenue" | "amount") => {
  const total = rows.reduce((n, r) => n + (r[key] ?? 0), 0);
  return (r: T) => (total ? ((r[key] ?? 0) / total) * 100 : 0);
};
const catShare = computed(() => share(cats.value, "revenue"));
const payShare = computed(() => share(pays.value, "amount"));

const topProducts = computed(() => products.value.slice(0, 10));
const bottomProducts = computed(() => [...products.value].reverse().slice(0, 10));

const expenseTotal = computed(() => expenses.value.reduce((n, e) => n + e.amount, 0));
const grossMargin = computed(() => (cur.value ? cur.value.revenue - cur.value.cost : 0));
const expShare = computed(() => share(expenses.value, "amount"));

// ---------------------------------------------------------- export

const exporting = ref("");
const fileStem = (name: string) => {
  const r = filters.value.range;
  return `${name}_${toLocalDate(r.from)}_${toLocalDate(new Date(r.to.getTime() - 1))}`;
};
const doExport = async (name: string, rows: (string | number | null)[][]) => {
  exporting.value = name;
  error.value = "";
  try {
    await exportCsv(fileStem(name), rows);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    exporting.value = "";
  }
};

const exportProducts = () =>
  doExport("products", [
    [t("inventory.table.name"), t("analytics.table.quantity"), t("analytics.kpi.revenue"), t("analytics.kpi.grossMargin")],
    ...products.value.map((p) => [p.name, p.quantity, p.revenue, p.margin]),
  ]);
const exportEmployees = () =>
  doExport("employees", [
    [t("analytics.table.employee"), t("analytics.table.transactions"), t("analytics.kpi.revenue"), t("analytics.table.items"), t("analytics.table.hours"), t("analytics.table.perHour")],
    ...staff.value.map((e) => [e.full_name, e.transactions, e.revenue, e.items, e.hours.toFixed(1), e.hours ? Math.round(e.revenue / e.hours) : ""]),
  ]);
const exportSeries = () =>
  doExport("sales", [
    [t("schedule.date"), t("analytics.kpi.revenue"), ...(compare.value ? [t("analytics.previousPeriod")] : [])],
    ...points.value.labels.map((l, i) => [l, points.value.now[i], ...(compare.value ? [points.value.before[i]] : [])]),
  ]);
const exportExpenses = () =>
  doExport("expenses", [
    [t("expenses.type"), t("expenses.amount")],
    ...expenses.value.map((e) => [e.name, e.amount]),
    [t("expenses.total"), expenseTotal.value],
    [t("analytics.kpi.grossMargin"), grossMargin.value],
    [t("analytics.netProfit"), grossMargin.value - expenseTotal.value],
  ]);
</script>

<template>
  <div class="mx-auto max-w-7xl space-y-5 p-6">
    <!-- One filter bar for every tab -->
    <div class="flex flex-wrap items-center gap-2 rounded-xl border bg-card p-3">
      <RangePicker v-model="range" />
      <Select v-model="userId" :key="`u${$i18n.locale}`">
        <SelectTrigger class="w-44"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem value="all">{{ $t("analytics.filters.allStaff") }}</SelectItem>
          <SelectItem v-for="u in users" :key="u.id" :value="String(u.id)">{{ u.full_name }}</SelectItem>
        </SelectContent>
      </Select>
      <Select v-model="categoryId" :key="`c${$i18n.locale}`">
        <SelectTrigger class="w-44"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem value="all">{{ $t("inventory.allCategories") }}</SelectItem>
          <SelectItem v-for="c in categories" :key="c.id" :value="String(c.id)">
            {{ (c.parent_id ? "— " : "") + c.name }}
          </SelectItem>
        </SelectContent>
      </Select>
      <Select v-model="payment" :key="`p${$i18n.locale}`">
        <SelectTrigger class="w-36"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem value="all">{{ $t("analytics.filters.allPayments") }}</SelectItem>
          <SelectItem value="cash">{{ $t("pos.cash") }}</SelectItem>
          <SelectItem value="card">{{ $t("pos.card") }}</SelectItem>
        </SelectContent>
      </Select>
      <label class="ms-auto flex items-center gap-2 text-sm text-muted-foreground">
        <input v-model="compare" type="checkbox" class="size-4 accent-primary" />
        {{ $t("analytics.compare") }}
      </label>
      <Loader2 v-if="loading" class="size-4 animate-spin text-muted-foreground" />
    </div>
    <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>

    <Tabs v-model="tab">
      <TabsList class="mb-5">
        <TabsTrigger value="overview">{{ $t("analytics.tabs.overview") }}</TabsTrigger>
        <TabsTrigger value="products">{{ $t("analytics.tabs.products") }}</TabsTrigger>
        <TabsTrigger value="employees">{{ $t("analytics.tabs.emp") }}</TabsTrigger>
        <TabsTrigger value="expenses">{{ $t("analytics.tabs.expenses") }}</TabsTrigger>
      </TabsList>

      <!-- Overview -->
      <TabsContent value="overview" class="space-y-5">
        <div class="grid overflow-hidden rounded-xl border bg-card sm:grid-cols-2 lg:grid-cols-4">
          <div v-for="(k, i) in kpis" :key="k.key" class="p-5" :class="i > 0 && 'border-t sm:border-t-0 sm:border-s'">
            <p class="text-sm text-muted-foreground">{{ $t(`analytics.kpi.${k.key}`) }}</p>
            <p class="mt-1 text-3xl font-semibold tracking-tight">{{ k.value }}</p>
            <p class="mt-1 text-sm">
              <span v-if="compare && k.delta !== null" class="font-semibold" :class="k.delta >= 0 ? 'text-success' : 'text-destructive'">
                {{ k.delta >= 0 ? "▲" : "▼" }} {{ Math.abs(k.delta).toFixed(1) }}%
              </span>
              <span class="ms-1 text-muted-foreground">{{ compare && k.delta !== null ? $t("analytics.vsPrevious") : k.sub }}</span>
            </p>
          </div>
        </div>

        <section class="min-w-0 rounded-xl border bg-card p-5">
          <div class="mb-4 flex items-center gap-3">
            <h2 class="font-semibold">{{ $t("analytics.charts.salesOverview") }}</h2>
            <span v-if="compare" class="flex items-center gap-1.5 text-xs text-muted-foreground">
              <span class="inline-block w-5 border-t border-dashed border-muted-foreground" /> {{ $t("analytics.previousPeriod") }}
            </span>
            <Button v-if="canExport" variant="ghost" size="sm" class="ms-auto" :disabled="!!exporting" @click="exportSeries">
              <Download /> CSV
            </Button>
          </div>
          <div class="relative h-64 w-full">
            <Chart type="line" :data="chartData" :options="chartOptions" />
          </div>
        </section>

        <div class="grid gap-5 lg:grid-cols-5">
          <section class="min-w-0 rounded-xl border bg-card p-5 lg:col-span-3">
            <h2 class="mb-1 font-semibold">{{ $t("analytics.busiestHours") }}</h2>
            <p class="mb-4 text-xs text-muted-foreground">{{ $t("analytics.busiestHoursHint") }}</p>
            <p v-if="!hours.length" class="py-8 text-center text-sm text-muted-foreground">{{ $t("analytics.noSales") }}</p>
            <div v-else class="overflow-x-auto" dir="ltr">
              <table class="border-separate border-spacing-0.5 text-[11px]">
                <thead>
                  <tr>
                    <th />
                    <th v-for="h in hours" :key="h" class="num px-0.5 font-normal text-muted-foreground">{{ String(h).padStart(2, "0") }}</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="d in WEEKDAYS" :key="d">
                    <th class="pe-2 text-end font-normal whitespace-nowrap text-muted-foreground">{{ $t(`analytics.charts.${dayKeys[d]}`) }}</th>
                    <td
                      v-for="h in hours"
                      :key="h"
                      class="size-6 rounded-sm"
                      :style="{ background: heatColor(heat.get(`${d}-${h}`) ?? 0) }"
                      :title="`${heat.get(`${d}-${h}`) ?? 0}`"
                    />
                  </tr>
                </tbody>
              </table>
            </div>
          </section>

          <div class="space-y-5 lg:col-span-2">
            <section class="rounded-xl border bg-card p-5">
              <h2 class="mb-4 font-semibold">{{ $t("analytics.byCategory") }}</h2>
              <ul class="space-y-3">
                <li v-for="c in cats" :key="c.id ?? 0" class="space-y-1">
                  <div class="flex items-center gap-2 text-sm">
                    <AppIcon v-if="c.icon_key" :icon="c.icon_key" :size="16" :style="{ color: categoryInk(c.color) }" />
                    <span class="flex-1">{{ c.name ? c.name : $t("catalog.noCategory") }}</span>
                    <span class="num font-medium">{{ formatCurrency(c.revenue) }}</span>
                  </div>
                  <div class="h-1.5 overflow-hidden rounded-full bg-muted">
                    <div class="h-full rounded-full" :style="{ width: `${catShare(c)}%`, background: categoryInk(c.color) }" />
                  </div>
                </li>
              </ul>
              <p v-if="!cats.length" class="text-sm text-muted-foreground">{{ $t("analytics.noSales") }}</p>
            </section>
            <section class="rounded-xl border bg-card p-5">
              <h2 class="mb-4 font-semibold">{{ $t("analytics.byPayment") }}</h2>
              <ul class="space-y-3">
                <li v-for="p in pays" :key="p.method" class="space-y-1">
                  <div class="flex items-center justify-between text-sm">
                    <span>{{ $t(`pos.${p.method}`) }} <span class="text-muted-foreground">({{ p.transactions }})</span></span>
                    <span class="num font-medium">{{ formatCurrency(p.amount) }}</span>
                  </div>
                  <div class="h-1.5 overflow-hidden rounded-full bg-muted">
                    <div class="h-full rounded-full bg-primary" :style="{ width: `${payShare(p)}%` }" />
                  </div>
                </li>
              </ul>
              <p v-if="categoryId !== 'all'" class="mt-3 text-xs text-muted-foreground">{{ $t("analytics.paymentsIgnoreCategory") }}</p>
            </section>
          </div>
        </div>
      </TabsContent>

      <!-- Products -->
      <TabsContent value="products" class="space-y-5">
        <section class="overflow-hidden rounded-xl border bg-card">
          <div class="flex items-center border-b px-5 py-4">
            <h2 class="font-semibold">{{ $t("analytics.charts.topProducts") }}</h2>
            <Button v-if="canExport" variant="ghost" size="sm" class="ms-auto" :disabled="!!exporting" @click="exportProducts">
              <Download /> {{ $t("analytics.exportAll") }}
            </Button>
          </div>
          <Table>
            <TableHeader>
              <TableRow class="hover:bg-transparent">
                <TableHead class="ps-5">{{ $t("inventory.table.name") }}</TableHead>
                <TableHead class="text-end">{{ $t("analytics.table.quantity") }}</TableHead>
                <TableHead class="text-end">{{ $t("analytics.kpi.revenue") }}</TableHead>
                <TableHead class="pe-5 text-end">{{ $t("analytics.kpi.grossMargin") }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="!topProducts.length" class="hover:bg-transparent">
                <TableCell colspan="4" class="py-10 text-center text-muted-foreground">{{ $t("analytics.noSales") }}</TableCell>
              </TableRow>
              <TableRow v-for="p in topProducts" :key="p.product_id">
                <TableCell class="ps-5 font-medium"><bdi>{{ p.name }}</bdi></TableCell>
                <TableCell class="num text-end">{{ p.quantity }}</TableCell>
                <TableCell class="num text-end font-semibold">{{ formatCurrency(p.revenue) }}</TableCell>
                <TableCell class="num pe-5 text-end">
                  {{ formatCurrency(p.margin) }}
                  <span class="text-muted-foreground">({{ p.revenue ? Math.round((p.margin / p.revenue) * 100) : 0 }}%)</span>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </section>

        <div class="grid gap-5 lg:grid-cols-2">
          <section class="overflow-hidden rounded-xl border bg-card">
            <h2 class="border-b px-5 py-4 font-semibold">{{ $t("analytics.leastSold") }}</h2>
            <ul class="divide-y text-sm">
              <li v-for="p in bottomProducts" :key="p.product_id" class="flex justify-between px-5 py-2.5">
                <bdi>{{ p.name }}</bdi>
                <span class="num text-muted-foreground">{{ p.quantity }} — {{ formatCurrency(p.revenue) }}</span>
              </li>
            </ul>
          </section>
          <section class="overflow-hidden rounded-xl border bg-card">
            <h2 class="border-b px-5 py-4 font-semibold">{{ $t("analytics.notSold") }}</h2>
            <p v-if="!slow.length" class="px-5 py-6 text-sm text-muted-foreground">{{ $t("analytics.everythingSold") }}</p>
            <ul class="divide-y text-sm">
              <li v-for="p in slow" :key="p.id" class="flex justify-between px-5 py-2.5">
                <bdi>{{ p.name }}</bdi>
                <span class="num text-muted-foreground">{{ $t("pos.left", { n: p.stock }) }}</span>
              </li>
            </ul>
          </section>
        </div>
      </TabsContent>

      <!-- Employees -->
      <TabsContent value="employees">
        <section class="overflow-hidden rounded-xl border bg-card">
          <div class="flex items-center border-b px-5 py-4">
            <h2 class="font-semibold">{{ $t("analytics.table.performance") }}</h2>
            <Button v-if="canExport" variant="ghost" size="sm" class="ms-auto" :disabled="!!exporting" @click="exportEmployees">
              <Download /> CSV
            </Button>
          </div>
          <Table>
            <TableHeader>
              <TableRow class="hover:bg-transparent">
                <TableHead class="ps-5">{{ $t("analytics.table.employee") }}</TableHead>
                <TableHead class="text-end">{{ $t("analytics.table.transactions") }}</TableHead>
                <TableHead class="text-end">{{ $t("analytics.kpi.revenue") }}</TableHead>
                <TableHead class="text-end">{{ $t("analytics.kpi.avgBasket") }}</TableHead>
                <TableHead class="text-end">{{ $t("analytics.table.itemsPerSale") }}</TableHead>
                <TableHead class="text-end">{{ $t("analytics.table.hours") }}</TableHead>
                <TableHead class="pe-5 text-end">{{ $t("analytics.table.perHour") }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="!staff.length" class="hover:bg-transparent">
                <TableCell colspan="7" class="py-10 text-center text-muted-foreground">{{ $t("analytics.noSales") }}</TableCell>
              </TableRow>
              <TableRow v-for="e in staff" :key="e.user_id">
                <TableCell class="ps-5 font-medium">{{ e.full_name }}</TableCell>
                <TableCell class="num text-end">{{ e.transactions }}</TableCell>
                <TableCell class="num text-end font-semibold">{{ formatCurrency(e.revenue) }}</TableCell>
                <TableCell class="num text-end">{{ formatCurrency(e.transactions ? e.revenue / e.transactions : 0) }}</TableCell>
                <TableCell class="num text-end">{{ e.transactions ? (e.items / e.transactions).toFixed(1) : "—" }}</TableCell>
                <TableCell class="num text-end">{{ e.hours.toFixed(1) }}</TableCell>
                <TableCell class="num pe-5 text-end">{{ e.hours ? formatCurrency(e.revenue / e.hours) : "—" }}</TableCell>
              </TableRow>
            </TableBody>
          </Table>
          <p class="border-t px-5 py-3 text-xs text-muted-foreground">{{ $t("analytics.hoursHint") }}</p>
        </section>
      </TabsContent>

      <!-- Expenses and profit -->
      <TabsContent value="expenses" class="space-y-5">
        <div class="grid overflow-hidden rounded-xl border bg-card sm:grid-cols-3">
          <div class="p-5">
            <p class="text-sm text-muted-foreground">{{ $t("analytics.kpi.grossMargin") }}</p>
            <p class="mt-1 text-3xl font-semibold tracking-tight">{{ formatCurrency(grossMargin) }}</p>
          </div>
          <div class="border-t p-5 sm:border-t-0 sm:border-s">
            <p class="text-sm text-muted-foreground">{{ $t("expenses.total") }}</p>
            <p class="mt-1 text-3xl font-semibold tracking-tight">{{ formatCurrency(expenseTotal) }}</p>
          </div>
          <div class="border-t p-5 sm:border-t-0 sm:border-s" :class="!narrowed && (grossMargin - expenseTotal >= 0 ? 'bg-success/8' : 'bg-destructive/8')">
            <p class="text-sm text-muted-foreground">{{ $t("analytics.netProfit") }}</p>
            <p class="mt-1 text-3xl font-semibold tracking-tight">
              {{ narrowed ? "—" : formatCurrency(grossMargin - expenseTotal) }}
            </p>
            <p v-if="narrowed" class="mt-1 text-xs text-muted-foreground">{{ $t("analytics.netProfitNeedsAll") }}</p>
          </div>
        </div>
        <section class="rounded-xl border bg-card p-5">
          <div class="mb-4 flex items-center">
            <h2 class="font-semibold">{{ $t("analytics.expensesByType") }}</h2>
            <Button v-if="canExport" variant="ghost" size="sm" class="ms-auto" :disabled="!!exporting" @click="exportExpenses">
              <Download /> CSV
            </Button>
          </div>
          <ul class="space-y-3">
            <li v-for="e in expenses" :key="e.type_id" class="space-y-1">
              <div class="flex items-center gap-2 text-sm">
                <AppIcon :icon="e.icon_key" :size="16" :style="{ color: categoryInk(e.color) }" />
                <span class="flex-1">{{ e.name }}</span>
                <span class="num font-medium">{{ formatCurrency(e.amount) }}</span>
              </div>
              <div class="h-1.5 overflow-hidden rounded-full bg-muted">
                <div class="h-full rounded-full" :style="{ width: `${expShare(e)}%`, background: categoryInk(e.color) }" />
              </div>
            </li>
          </ul>
          <p v-if="!expenses.length" class="text-sm text-muted-foreground">{{ $t("expenses.none") }}</p>
        </section>
      </TabsContent>
    </Tabs>
  </div>
</template>
