<script setup lang="ts">
import Chart from "@/components/data/Chart.vue";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
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
import { Search } from "lucide-vue-next";
import { formatCurrency } from "@/lib/format";

const { t } = useI18n();

const activeTab = ref("gen");
const range = ref("today");

// Chart.js draws to canvas, so it needs resolved colors, not Tailwind classes
const css = getComputedStyle(document.documentElement);
const token = (name: string) => css.getPropertyValue(`--${name}`).trim();
const ink = token("muted-foreground");
const grid = token("border");
const pine = token("chart-1");
const sun = token("chart-2");

const stats = computed(() => [
  { label: t("analytics.stats.todaySales"), value: formatCurrency(1234.56), delta: 12.5 },
  { label: t("analytics.stats.transactions"), value: "48", delta: 8.3 },
  { label: t("analytics.stats.avgTransaction"), value: formatCurrency(25.72), delta: -3.2 },
]);

const salesChartData = computed(() => ({
  labels: ["mon", "tue", "wed", "thu", "fri", "sat", "sun"].map((d) =>
    t(`analytics.charts.${d}`),
  ),
  datasets: [
    {
      label: t("analytics.charts.salesOverview"),
      data: [850, 920, 780, 1100, 1050, 1400, 1234],
      borderColor: pine,
      backgroundColor: `${pine}1f`,
      borderWidth: 2.5,
      tension: 0.35,
      fill: true,
      pointRadius: 0,
      pointHoverRadius: 5,
      pointBackgroundColor: sun,
    },
  ],
}));

const axis = {
  border: { display: false },
  ticks: { color: ink, font: { family: "inherit" } },
};

const salesChartOptions = {
  interaction: { mode: "index" as const, intersect: false },
  plugins: { legend: { display: false } },
  scales: {
    y: { ...axis, beginAtZero: true, grid: { color: grid } },
    x: { ...axis, grid: { display: false } },
  },
};

const prodChartData = computed(() => ({
  labels: ["Coca-Cola", "Samoon", "Milk 1L", "Chips"],
  datasets: [
    {
      label: t("analytics.charts.sold"),
      data: [145, 89, 67, 54],
      backgroundColor: [sun, pine, pine, pine],
      borderRadius: 6,
      barThickness: 22,
    },
  ],
}));

const prodChartOptions = {
  indexAxis: "y" as const,
  plugins: { legend: { display: false } },
  scales: {
    x: { ...axis, beginAtZero: true, grid: { color: grid } },
    y: { ...axis, grid: { display: false } },
  },
};

const transactions = [
  { id: "TXN-1041", date: "Nov 4, 2:45 PM", cashier: "Rebin A.", items: 5, total: 45.67, payment: "cash" },
  { id: "TXN-1040", date: "Nov 4, 2:31 PM", cashier: "Rebin A.", items: 2, total: 3.1, payment: "card" },
  { id: "TXN-1039", date: "Nov 4, 1:58 PM", cashier: "Shno K.", items: 11, total: 27.4, payment: "cash" },
  { id: "TXN-1038", date: "Nov 4, 1:12 PM", cashier: "Shno K.", items: 1, total: 0.75, payment: "cash" },
];

const employees = [
  { name: "Rebin A.", transactions: 32, total: 687.5 },
  { name: "Shno K.", transactions: 16, total: 547.06 },
];
</script>

<template>
  <div class="mx-auto max-w-7xl p-6">
    <Tabs v-model="activeTab">
      <TabsList class="mb-6">
        <TabsTrigger value="gen">{{ $t("analytics.tabs.gen") }}</TabsTrigger>
        <TabsTrigger value="sales">{{ $t("analytics.tabs.sales") }}</TabsTrigger>
        <TabsTrigger value="emp">{{ $t("analytics.tabs.emp") }}</TabsTrigger>
      </TabsList>

      <TabsContent value="gen" class="space-y-5">
        <!-- one strip, not four floating cards: these numbers are read together -->
        <div class="grid overflow-hidden rounded-xl border bg-card sm:grid-cols-2 lg:grid-cols-4">
          <div
            v-for="(s, i) in stats"
            :key="s.label"
            class="p-5"
            :class="i > 0 && 'border-t sm:border-t-0 sm:border-s'"
          >
            <p class="text-sm text-muted-foreground">{{ s.label }}</p>
            <p class="mt-1 text-3xl font-semibold tracking-tight">{{ s.value }}</p>
            <p class="mt-1 text-sm">
              <span
                class="font-semibold"
                :class="s.delta >= 0 ? 'text-success' : 'text-destructive'"
                >{{ s.delta >= 0 ? "▲" : "▼" }} {{ Math.abs(s.delta) }}%</span
              >
              <span class="ms-1 text-muted-foreground">{{
                $t("analytics.stats.vsLastWeek")
              }}</span>
            </p>
          </div>
          <div class="border-t bg-sun/15 p-5 sm:border-s lg:border-t-0">
            <p class="text-sm text-muted-foreground">
              {{ $t("analytics.stats.lowStock") }}
            </p>
            <p class="mt-1 text-3xl font-semibold tracking-tight">7</p>
            <p class="mt-1 text-sm text-muted-foreground">
              {{ $t("analytics.stats.restock") }}
            </p>
          </div>
        </div>

        <div class="grid gap-5 lg:grid-cols-5">
          <section class="min-w-0 rounded-xl border bg-card p-5 lg:col-span-3">
            <h2 class="mb-4 font-semibold">{{ $t("analytics.charts.salesOverview") }}</h2>
            <div class="relative h-64 w-full">
              <Chart type="line" :data="salesChartData" :options="salesChartOptions" />
            </div>
          </section>
          <section class="min-w-0 rounded-xl border bg-card p-5 lg:col-span-2">
            <h2 class="mb-4 font-semibold">{{ $t("analytics.charts.topProducts") }}</h2>
            <div class="relative h-64 w-full">
              <Chart type="bar" :data="prodChartData" :options="prodChartOptions" />
            </div>
          </section>
        </div>
      </TabsContent>

      <TabsContent value="sales">
        <div class="overflow-hidden rounded-xl border bg-card">
          <div class="flex flex-wrap gap-3 border-b p-4">
            <Select v-model="range" :key="$i18n.locale">
              <SelectTrigger class="w-40"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="today">{{ $t("analytics.range.today") }}</SelectItem>
                <SelectItem value="week">{{ $t("analytics.range.week") }}</SelectItem>
                <SelectItem value="month">{{ $t("analytics.range.month") }}</SelectItem>
              </SelectContent>
            </Select>
            <div class="relative min-w-56 flex-1">
              <Search
                :size="18"
                class="pointer-events-none absolute start-3 top-1/2 -translate-y-1/2 text-muted-foreground"
              />
              <input
                type="search"
                :placeholder="$t('analytics.searchPlaceholder')"
                :aria-label="$t('analytics.searchPlaceholder')"
                class="h-10 w-full rounded-md border border-input bg-card ps-10 pe-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              />
            </div>
          </div>
          <Table>
            <TableHeader>
              <TableRow class="hover:bg-transparent">
                <TableHead class="ps-5">{{ $t("analytics.table.id") }}</TableHead>
                <TableHead>{{ $t("analytics.table.date") }}</TableHead>
                <TableHead>{{ $t("analytics.table.cashier") }}</TableHead>
                <TableHead class="text-end">{{ $t("analytics.table.items") }}</TableHead>
                <TableHead>{{ $t("analytics.table.payment") }}</TableHead>
                <TableHead class="pe-5 text-end">{{ $t("analytics.table.total") }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="tx in transactions" :key="tx.id">
                <TableCell class="num ps-5 text-muted-foreground">#{{ tx.id }}</TableCell>
                <TableCell class="num">{{ tx.date }}</TableCell>
                <TableCell>{{ tx.cashier }}</TableCell>
                <TableCell class="num text-end">{{ tx.items }}</TableCell>
                <TableCell>{{ $t(`pos.${tx.payment}`) }}</TableCell>
                <TableCell class="num pe-5 text-end font-semibold">{{
                  formatCurrency(tx.total)
                }}</TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </TabsContent>

      <TabsContent value="emp">
        <div class="overflow-hidden rounded-xl border bg-card">
          <h2 class="border-b px-5 py-4 font-semibold">
            {{ $t("analytics.table.performance") }}
          </h2>
          <Table>
            <TableHeader>
              <TableRow class="hover:bg-transparent">
                <TableHead class="ps-5">{{ $t("analytics.table.employee") }}</TableHead>
                <TableHead class="text-end">{{ $t("analytics.table.transactions") }}</TableHead>
                <TableHead class="text-end">{{ $t("analytics.table.totalSales") }}</TableHead>
                <TableHead class="pe-5 text-end">{{ $t("analytics.table.avg") }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="e in employees" :key="e.name">
                <TableCell class="ps-5 font-medium">{{ e.name }}</TableCell>
                <TableCell class="num text-end">{{ e.transactions }}</TableCell>
                <TableCell class="num text-end font-semibold">{{
                  formatCurrency(e.total)
                }}</TableCell>
                <TableCell class="num pe-5 text-end">{{
                  formatCurrency(e.total / e.transactions)
                }}</TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </TabsContent>
    </Tabs>
  </div>
</template>
