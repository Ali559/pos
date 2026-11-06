<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Check, Copy, Download, PackageCheck } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { reorderList, suggestedOrder, type ReorderRow } from "@/db/suppliers";
import { exportCsv } from "@/lib/exportCsv";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import { useAuth } from "@/stores/auth";
import { useShop } from "@/stores/shop";

// What to order: everything at or under its reorder level, grouped by the
// supplier it last came from, with a suggested quantity. Each group can be
// copied as a message for WhatsApp/Viber, or the whole list exported.
const { t } = useI18n();
const auth = useAuth();
const { profile } = useShop();
const rows = ref<ReorderRow[]>([]);
const loaded = ref(false);
const message = ref("");
const copied = ref<string | null>(null);

onMounted(async () => {
  rows.value = await reorderList();
  loaded.value = true;
});

const groups = computed(() => {
  const map = new Map<string, { name: string | null; phone: string | null; rows: ReorderRow[] }>();
  for (const r of rows.value) {
    const key = String(r.supplier_id ?? "none");
    if (!map.has(key)) map.set(key, { name: r.supplier, phone: r.supplier_phone, rows: [] });
    map.get(key)!.rows.push(r);
  }
  return [...map.entries()];
});

const unitLabel = (u: string) => t(`catalog.units.${u}`).toLocaleLowerCase();
const fmt = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(2).replace(/0+$/, "").replace(/\.$/, ""));
const orderText = (r: ReorderRow) => {
  const o = suggestedOrder(r);
  return o.packs != null ? `${o.packs} × ${r.pack_name} (${fmt(o.units)} ${unitLabel(r.unit)})` : `${fmt(o.units)} ${unitLabel(r.unit)}`;
};
const groupCost = (list: ReorderRow[]) => list.reduce((n, r) => n + suggestedOrder(r).units * r.cost, 0);

const copy = async (key: string, list: ReorderRow[]) => {
  const lines = list.map((r) => `- ${r.name}: ${orderText(r)}`);
  const text = [t("reorder.messageHeader", { shop: profile.value?.name ?? "" }), ...lines].join("\n");
  try {
    await navigator.clipboard.writeText(text);
    copied.value = key;
    setTimeout(() => (copied.value = null), 2000);
  } catch (e) {
    message.value = errorMessage(e);
  }
};

const exportAll = async () => {
  message.value = "";
  try {
    await exportCsv(`reorder-${new Date().toLocaleDateString("en-CA")}`, [
      [t("deliveries.supplier"), t("catalog.productName"), t("reorder.inStock"), t("catalog.minStock"), t("reorder.perWeek"), t("reorder.order"), t("reorder.estCost")],
      ...rows.value.map((r) => [r.supplier ?? "", r.name, r.stock, r.min_stock, fmt(r.per_day * 7), orderText(r), Math.round(suggestedOrder(r).units * r.cost)]),
    ]);
  } catch (e) {
    message.value = errorMessage(e);
  }
};
</script>

<template>
  <div class="space-y-5">
    <div class="flex flex-wrap items-center gap-3">
      <p class="flex-1 text-sm text-muted-foreground">{{ $t("reorder.hint") }}</p>
      <Button v-if="rows.length && auth.can('analytics.export')" variant="outline" @click="exportAll"><Download />{{ $t("reorder.export") }}</Button>
    </div>
    <p v-if="message" role="alert" class="text-sm text-destructive">{{ message }}</p>

    <div v-if="loaded && !rows.length" class="flex flex-col items-center gap-2 rounded-xl border border-dashed p-12 text-center text-muted-foreground">
      <PackageCheck :size="28" />
      {{ $t("reorder.nothing") }}
    </div>

    <section v-for="[key, g] in groups" :key="key" class="overflow-hidden rounded-xl border bg-card">
      <header class="flex flex-wrap items-center gap-3 border-b px-5 py-3">
        <div class="min-w-0 flex-1">
          <h2 class="font-semibold"><bdi>{{ g.name ?? $t("reorder.noSupplier") }}</bdi></h2>
          <p class="text-xs text-muted-foreground">
            <span v-if="g.phone" class="num" dir="ltr">{{ g.phone }} · </span>{{ $t("reorder.estimate", { amount: formatCurrency(groupCost(g.rows)) }) }}
          </p>
        </div>
        <Button variant="outline" size="sm" @click="copy(key, g.rows)">
          <Check v-if="copied === key" /><Copy v-else />{{ copied === key ? $t("reorder.copied") : $t("reorder.copy") }}
        </Button>
      </header>
      <table class="w-full text-sm">
        <thead class="text-xs text-muted-foreground">
          <tr class="border-b">
            <th class="px-5 py-2 text-start font-medium">{{ $t("catalog.productName") }}</th>
            <th class="px-3 py-2 text-end font-medium">{{ $t("reorder.inStock") }}</th>
            <th class="px-3 py-2 text-end font-medium">{{ $t("reorder.perWeek") }}</th>
            <th class="px-5 py-2 text-end font-medium">{{ $t("reorder.order") }}</th>
          </tr>
        </thead>
        <tbody class="divide-y">
          <tr v-for="r in g.rows" :key="r.id">
            <td class="px-5 py-2.5 font-medium"><bdi>{{ r.name }}</bdi></td>
            <td class="num px-3 py-2.5 text-end" :class="r.stock <= 0 ? 'text-destructive' : ''">{{ fmt(r.stock) }} <span class="text-xs text-muted-foreground">/ {{ fmt(r.min_stock) }}</span></td>
            <td class="num px-3 py-2.5 text-end text-muted-foreground">{{ fmt(r.per_day * 7) }}</td>
            <td class="px-5 py-2.5 text-end font-semibold"><bdi>{{ orderText(r) }}</bdi></td>
          </tr>
        </tbody>
      </table>
    </section>
  </div>
</template>
