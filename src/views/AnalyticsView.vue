<script setup lang="ts">
import Chart from "@/components/data/Chart.vue";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

// Tab management
type TabType = "gen" | "sales" | "emp";
const { t } = useI18n(); // Use the composable

const activeTab = ref<TabType>("gen");

const tabs = computed<
  {
    id: TabType;
    label: string;
  }[]
>(() => [
  { id: "gen", label: t("analytics.tabs.gen") },
  { id: "sales", label: t("analytics.tabs.sales") },
  { id: "emp", label: t("analytics.tabs.emp") },
]);

const showAnalTab = (tab: TabType) => {
  activeTab.value = tab;
};

// Sales chart data
const salesChartData = computed(() => ({
  labels: [
    t("analytics.charts.mon"),
    t("analytics.charts.tue"),
    t("analytics.charts.wed"),
    t("analytics.charts.thu"),
    t("analytics.charts.fri"),
    t("analytics.charts.sat"),
    t("analytics.charts.sun"),
  ],
  datasets: [
    {
      label: "Sales",
      data: [850, 920, 780, 1100, 1050, 1400, 1234],
      borderColor: "oklch(0.6723 0.1606 244.9955)",
      backgroundColor: "oklch(0.6723 0.1606 244.9955)/0.1",
      borderWidth: 2,
      tension: 0.4,
      fill: true,
    },
  ],
}));

const salesChartOptions = {
  plugins: {
    legend: { display: false },
    tooltip: {
      mode: "index" as const,
      intersect: false,
    },
  },
  scales: {
    y: {
      beginAtZero: true,
      grid: {
        color: "oklch(0.9317 0.0118 231.6594)",
      },
      ticks: {
        color: "oklch(0.1884 0.0128 248.5103)",
      },
    },
    x: {
      grid: {
        color: "oklch(0.9317 0.0118 231.6594)",
      },
      ticks: {
        color: "oklch(0.1884 0.0128 248.5103)",
      },
    },
  },
};

// Products chart data
const prodChartData = computed(() => ({
  labels: ["Coca Cola", "Milk", "Bread", "Chips"],
  datasets: [
    {
      label: t("analytics.charts.sold"),
      data: [145, 89, 67, 54],
      backgroundColor: "oklch(0.6723 0.1606 244.9955)",
      borderColor: "oklch(0.6723 0.1606 244.9955)",
      borderWidth: 1,
      borderRadius: 4,
    },
  ],
}));

const prodChartOptions = {
  plugins: {
    legend: { display: false },
    tooltip: {
      callbacks: {
        label: (context: any) => `${context.dataset.label}: ${context.raw}`,
      },
    },
  },
  scales: {
    y: {
      beginAtZero: true,
      grid: {
        color: "oklch(0.9317 0.0118 231.6594)",
      },
      ticks: {
        color: "oklch(0.1884 0.0128 248.5103)",
        stepSize: 20,
      },
    },
    x: {
      grid: {
        display: false,
      },
      ticks: {
        color: "oklch(0.1884 0.0128 248.5103)",
      },
    },
  },
};
</script>

<template>
  <div class="p-6">
    <div class="max-w-7xl mx-auto">
      <div class="mb-6">
        <h2 class="text-2xl font-bold text-foreground mb-1">
          {{ $t("analytics.title") }}
        </h2>
        <p class="text-muted-foreground">{{ $t("analytics.subtitle") }}</p>
      </div>

      <!-- Vue 3 Tabs -->
      <div class="mb-6 border-b border-border">
        <div class="flex gap-6">
          <button
            v-for="tab in tabs"
            :key="tab.id"
            @click="showAnalTab(tab.id)"
            :class="[
              'px-4 py-3 font-medium transition-colors',
              activeTab === tab.id
                ? 'text-primary border-b-2 border-primary'
                : 'text-muted-foreground hover:text-foreground',
            ]"
          >
            {{ tab.label }}
          </button>
        </div>
      </div>

      <!-- General Tab -->
      <div v-show="activeTab === 'gen'">
        <div class="grid md:grid-cols-4 gap-6 mb-6">
          <div class="bg-card rounded-xl border border-border p-6 shadow-sm">
            <div class="text-sm text-muted-foreground mb-1">
              {{ $t("analytics.stats.todaySales") }}
            </div>
            <div class="text-3xl font-bold text-foreground">$1,234.56</div>
            <div class="text-sm text-green-600 mt-2">+12.5%</div>
          </div>
          <div class="bg-card rounded-xl border border-border p-6 shadow-sm">
            <div class="text-sm text-muted-foreground mb-1">
              {{ $t("analytics.stats.transactions") }}
            </div>
            <div class="text-3xl font-bold text-foreground">48</div>
            <div class="text-sm text-green-600 mt-2">+8.3%</div>
          </div>
          <div class="bg-card rounded-xl border border-border p-6 shadow-sm">
            <div class="text-sm text-muted-foreground mb-1">
              {{ $t("analytics.stats.avgTransaction") }}
            </div>
            <div class="text-3xl font-bold text-foreground">$25.72</div>
            <div class="text-sm text-destructive mt-2">-3.2%</div>
          </div>
          <div class="bg-card rounded-xl border border-border p-6 shadow-sm">
            <div class="text-sm text-muted-foreground mb-1">
              {{ $t("analytics.stats.lowStock") }}
            </div>
            <div class="text-3xl font-bold text-amber-600">7</div>
            <div class="text-sm text-muted-foreground mt-2">
              {{ $t("analytics.stats.restock") }}
            </div>
          </div>
        </div>

        <div class="grid lg:grid-cols-2 gap-6">
          <div class="bg-card rounded-xl border border-border p-6 shadow-sm">
            <h3 class="font-semibold text-foreground mb-4">
              {{ $t("analytics.charts.salesOverview") }}
            </h3>
            <div class="h-64 mb-4">
              <Chart
                type="line"
                :data="salesChartData"
                :options="salesChartOptions"
              />
            </div>
          </div>
          <div class="bg-card rounded-xl border border-border p-6 shadow-sm">
            <h3 class="font-semibold text-foreground mb-4">
              {{ $t("analytics.charts.topProducts") }}
            </h3>
            <div class="h-64 mb-4">
              <Chart
                type="bar"
                :data="prodChartData"
                :options="prodChartOptions"
              />
            </div>
          </div>
        </div>
      </div>

      <!-- Sales Tab -->
      <div v-show="activeTab === 'sales'" class="space-y-6">
        <div
          class="bg-card rounded-xl border border-border overflow-hidden shadow-sm"
        >
          <div class="p-6 border-b border-border flex gap-4">
            <select
              class="px-4 py-2 bg-background border border-input rounded-lg text-foreground"
            >
              <option>Today</option>
              <option>This Week</option>
              <option>This Month</option>
            </select>
            <input
              type="text"
              :placeholder="$t('analytics.searchPlaceholder')"
              class="flex-1 px-4 py-2 bg-background border border-input rounded-lg text-foreground placeholder:text-muted-foreground"
            />
          </div>
          <table class="w-full">
            <thead class="bg-muted border-b border-border">
              <tr>
                <th
                  class="px-6 py-3 ltr:text-left text-xs font-medium uppercase text-muted-foreground"
                >
                  {{ $t("analytics.table.id") }}
                </th>
                <th
                  class="px-6 py-3 ltr:text-left text-xs font-medium uppercase text-muted-foreground"
                >
                  {{ $t("analytics.table.date") }}
                </th>
                <th
                  class="px-6 py-3 ltr:text-left text-xs font-medium uppercase text-muted-foreground"
                >
                  {{ $t("analytics.table.cashier") }}
                </th>
                <th
                  class="px-6 py-3 ltr:text-right text-xs font-medium uppercase text-muted-foreground"
                >
                  {{ $t("analytics.table.items") }}
                </th>
                <th
                  class="px-6 py-3 ltr:text-right text-xs font-medium uppercase text-muted-foreground"
                >
                  {{ $t("analytics.table.total") }}
                </th>
                <th
                  class="px-6 py-3 ltr:text-left text-xs font-medium uppercase text-muted-foreground"
                >
                  {{ $t("analytics.table.payment") }}
                </th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border">
              <tr class="hover:bg-accent/50">
                <td class="px-6 py-4 font-mono text-sm text-foreground">
                  #TXN-001
                </td>
                <td class="px-6 py-4 text-sm text-foreground">
                  Feb 5, 2:45 PM
                </td>
                <td class="px-6 py-4 text-sm text-foreground">John Doe</td>
                <td class="px-6 py-4 ltr:text-right text-foreground">5</td>
                <td class="px-6 py-4 ltr:text-right font-semibold text-primary">
                  $45.67
                </td>
                <td class="px-6 py-4 text-sm text-foreground">Cash</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- Employees Tab -->
      <div v-show="activeTab === 'emp'" class="space-y-6">
        <div
          class="bg-card rounded-xl border border-border overflow-hidden shadow-sm"
        >
          <div class="p-6 border-b border-border">
            <h3 class="font-semibold text-foreground">
              {{ $t("analytics.table.performance") }}
            </h3>
          </div>
          <table class="w-full">
            <thead class="bg-muted border-b border-border">
              <tr>
                <th
                  class="px-6 py-3 ltr:text-left text-xs font-medium uppercase text-muted-foreground"
                >
                  {{ $t("analytics.table.employee") }}
                </th>
                <th
                  class="px-6 py-3 ltr:text-right text-xs font-medium uppercase text-muted-foreground"
                >
                  {{ $t("analytics.table.transactions") }}
                </th>
                <th
                  class="px-6 py-3 ltr:text-right text-xs font-medium uppercase text-muted-foreground"
                >
                  {{ $t("analytics.table.totalSales") }}
                </th>
                <th
                  class="px-6 py-3 ltr:text-right text-xs font-medium uppercase text-muted-foreground"
                >
                  {{ $t("analytics.table.avg") }}
                </th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border">
              <tr class="hover:bg-accent/50">
                <td class="px-6 py-4 font-medium text-foreground">John Doe</td>
                <td class="px-6 py-4 ltr:text-right text-foreground">32</td>
                <td
                  class="px-6 py-4 ltr:text-right font-semibold text-green-600"
                >
                  $687.50
                </td>
                <td class="px-6 py-4 ltr:text-right text-foreground">$21.48</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  </div>
</template>
