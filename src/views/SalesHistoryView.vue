<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowLeft, Printer, Search, Undo2 } from "lucide-vue-next";
import { RouterLink } from "vue-router";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import RangePicker, { resolveRange, type RangeModel } from "@/components/reports/RangePicker.vue";
import ReturnDialog from "@/components/sales/ReturnDialog.vue";
import { getSale, lineSold, listSales, paymentText, type SaleDetail, type SaleRow, type SaleStatus } from "@/db/sales";
import { errorMessage } from "@/lib/errors";
import { formatCurrency, formatUsd } from "@/lib/format";
import { formatDateTime, formatTime } from "@/lib/locale";
import { printReceipt } from "@/lib/print";
import { useAuth } from "@/stores/auth";

// Past sales: find one by receipt number (typed or scanned off the paper
// receipt), reprint it, or take items back.
const { t } = useI18n();
const auth = useAuth();
const canReprint = computed(() => auth.can("sales.reprint"));

const range = ref<RangeModel>({ preset: "today" });
const q = ref("");
const rows = ref<SaleRow[]>([]);
const selected = ref<SaleDetail | null>(null);
const returning = ref(false);
const loading = ref(false);
const error = ref("");
const notice = ref("");

const load = async () => {
  loading.value = true;
  error.value = "";
  try {
    rows.value = await listSales({ range: resolveRange(range.value), q: q.value });
    // A scanned receipt number opens straight away
    if (q.value && rows.value.length === 1) await open(rows.value[0]!.id);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
};

const open = async (id: number) => {
  notice.value = "";
  selected.value = await getSale(id);
};

watch(range, load, { deep: true });
let typing: ReturnType<typeof setTimeout> | undefined;
watch(q, () => {
  clearTimeout(typing);
  typing = setTimeout(load, 250);
});
onMounted(load);

const statusTone: Record<SaleStatus, "secondary" | "outline" | "destructive"> = {
  completed: "secondary",
  partially_refunded: "outline",
  refunded: "destructive",
  pending: "outline",
};
const anyLeft = computed(() => selected.value?.lines.some((l) => l.quantity > l.returned) ?? false);
const qty = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(3).replace(/0+$/, ""));

const onReturned = async (refund: number) => {
  const id = selected.value!.id;
  await Promise.all([open(id), load()]);
  notice.value = t("returns.done", { amount: formatCurrency(refund) });
};

const reprint = (id: number) => printReceipt(id, { copy: true }).catch((e) => (error.value = errorMessage(e)));
</script>

<template>
  <div class="flex h-full min-h-0">
    <section class="flex w-[440px] shrink-0 flex-col border-e">
      <div class="space-y-3 border-b p-5">
        <RouterLink to="/main/point-of-sale" class="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground">
          <ArrowLeft :size="15" class="rtl:rotate-180" /> {{ $t("history.back") }}
        </RouterLink>
        <form class="relative" @submit.prevent="load">
          <Search :size="18" class="absolute start-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            v-model="q"
            dir="ltr"
            autofocus
            autocomplete="off"
            :aria-label="$t('history.search')"
            :placeholder="$t('history.search')"
            class="num h-11 w-full rounded-lg border border-input bg-card ps-10 pe-3 outline-none focus:border-ring focus:ring-[3px] focus:ring-ring/50"
          />
        </form>
        <RangePicker v-if="!q" v-model="range" />
      </div>
      <p v-if="error" role="alert" class="px-5 py-3 text-sm text-destructive">{{ error }}</p>
      <p v-if="!loading && !rows.length" class="px-5 py-10 text-center text-sm text-muted-foreground">
        {{ q ? $t("history.noMatch") : $t("history.empty") }}
      </p>
      <ul class="min-h-0 flex-1 divide-y overflow-y-auto">
        <li v-for="r in rows" :key="r.id">
          <button
            type="button"
            class="flex w-full items-center gap-3 px-5 py-3 text-start hover:bg-muted aria-pressed:bg-secondary"
            :aria-pressed="selected?.id === r.id"
            @click="open(r.id)"
          >
            <div class="min-w-0 flex-1">
              <p class="flex items-center gap-2">
                <span class="num font-semibold" dir="ltr">{{ r.receipt_number }}</span>
                <Badge v-if="r.status !== 'completed'" :variant="statusTone[r.status]">{{ $t(`history.status.${r.status}`) }}</Badge>
              </p>
              <p class="truncate text-sm text-muted-foreground">
                {{ q ? formatDateTime(new Date(r.sale_date)) : formatTime(new Date(r.sale_date)) }} — {{ r.cashier }} — {{ $t("pos.items", r.items) }}
              </p>
            </div>
            <span class="text-end">
              <span class="num block font-semibold">{{ formatCurrency(r.total) }}</span>
              <span v-if="r.refunded" class="num block text-xs text-destructive">−{{ formatCurrency(r.refunded) }}</span>
            </span>
          </button>
        </li>
      </ul>
    </section>

    <section class="min-w-0 flex-1 overflow-y-auto p-6">
      <p v-if="!selected" class="grid h-full place-items-center text-muted-foreground">{{ $t("history.pick") }}</p>
      <div v-else class="mx-auto max-w-2xl space-y-5">
        <header class="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 class="num text-2xl font-semibold" dir="ltr">{{ selected.receipt_number }}</h2>
            <p class="text-sm text-muted-foreground">
              {{ formatDateTime(new Date(selected.sale_date)) }} — {{ selected.cashier }}
            </p>
            <p v-if="selected.customer_name" class="text-sm text-muted-foreground">
              {{ $t("receipt.customer") }}: <bdi class="font-medium text-foreground">{{ selected.customer_name }}</bdi>
            </p>
            <p v-if="selected.approver" class="text-sm text-muted-foreground">
              {{ $t("history.approvedBy", { name: selected.approver }) }}
            </p>
          </div>
          <div class="flex gap-2">
            <Button v-if="canReprint" variant="outline" @click="reprint(selected.id)"><Printer /> {{ $t("receipt.reprint") }}</Button>
            <Button v-if="anyLeft" @click="returning = true"><Undo2 /> {{ $t("returns.title") }}</Button>
          </div>
        </header>
        <p v-if="notice" role="status" class="rounded-lg bg-success/10 px-4 py-3 text-sm font-medium text-success">{{ notice }}</p>

        <div class="rounded-xl border bg-card">
          <table class="w-full text-sm">
            <thead class="border-b text-xs text-muted-foreground">
              <tr>
                <th class="px-4 py-2.5 text-start font-medium">{{ $t("history.item") }}</th>
                <th class="px-4 py-2.5 text-end font-medium">{{ $t("history.qty") }}</th>
                <th class="px-4 py-2.5 text-end font-medium">{{ $t("history.amount") }}</th>
              </tr>
            </thead>
            <tbody class="divide-y">
              <tr v-for="l in selected.lines" :key="l.id">
                <td class="px-4 py-2.5">
                  <bdi class="font-medium">{{ l.name }}</bdi>
                  <span class="num block text-xs text-muted-foreground"><bdi v-if="l.unit_name">{{ l.unit_name }} </bdi>× {{ formatCurrency(lineSold(l).each) }}</span>
                  <span v-if="l.promo_discount" class="num block text-xs text-success"><bdi>{{ l.promotion_name }}</bdi> −{{ formatCurrency(l.promo_discount) }}</span>
                  <span v-if="l.discount - l.promo_discount > 0.0001" class="num block text-xs text-primary">−{{ formatCurrency(l.discount - l.promo_discount) }}</span>
                </td>
                <td class="num px-4 py-2.5 text-end">
                  <bdi>{{ lineSold(l).label }}</bdi>
                  <span v-if="l.returned" class="block text-xs text-destructive">{{ $t("history.returned", { n: qty(l.returned) }) }}</span>
                </td>
                <td class="num px-4 py-2.5 text-end font-semibold">{{ formatCurrency(l.subtotal) }}</td>
              </tr>
            </tbody>
          </table>
          <dl class="space-y-1.5 border-t px-4 py-3 text-sm">
            <div class="flex justify-between"><dt class="text-muted-foreground">{{ $t("pos.subtotal") }}</dt><dd class="num">{{ formatCurrency(selected.subtotal) }}</dd></div>
            <div v-if="selected.discount" class="flex justify-between"><dt class="text-muted-foreground">{{ $t("receipt.discount") }}</dt><dd class="num">−{{ formatCurrency(selected.discount) }}</dd></div>
            <div v-if="selected.tax" class="flex justify-between"><dt class="text-muted-foreground">{{ $t("pos.tax") }}</dt><dd class="num">{{ formatCurrency(selected.tax) }}</dd></div>
            <div class="flex justify-between text-base font-semibold"><dt>{{ $t("pos.total") }}</dt><dd class="num">{{ formatCurrency(selected.total) }}</dd></div>
            <div v-for="(pay, i) in selected.payments" :key="i" class="flex justify-between">
              <dt class="text-muted-foreground">{{ $t(`pos.${pay.method}`) }}</dt>
              <dd class="num" dir="ltr">{{ paymentText(pay, formatCurrency, formatUsd) }}</dd>
            </div>
            <div v-if="selected.change" class="flex justify-between"><dt class="text-muted-foreground">{{ $t("cash.change") }}</dt><dd class="num">{{ formatCurrency(selected.change) }}</dd></div>
            <div v-if="selected.refunded" class="flex justify-between text-destructive"><dt>{{ $t("receipt.refunded") }}</dt><dd class="num">−{{ formatCurrency(selected.refunded) }}</dd></div>
          </dl>
        </div>

        <section v-if="selected.returns.length" class="space-y-2">
          <h3 class="font-semibold">{{ $t("history.returns") }}</h3>
          <ul class="divide-y rounded-xl border bg-card text-sm">
            <li v-for="r in selected.returns" :key="r.id" class="flex items-center gap-3 px-4 py-2.5">
              <div class="min-w-0 flex-1">
                <p>{{ formatDateTime(new Date(r.return_date)) }} — {{ r.user }}</p>
                <p v-if="r.reason" class="truncate text-muted-foreground"><bdi>{{ r.reason }}</bdi></p>
              </div>
              <span class="text-xs text-muted-foreground">{{ $t(`pos.${r.method}`) }}</span>
              <span class="num font-semibold text-destructive">−{{ formatCurrency(r.total_refund) }}</span>
            </li>
          </ul>
        </section>
      </div>
    </section>
    <ReturnDialog v-if="selected" v-model:open="returning" :sale="selected" @done="onReturned" />
  </div>
</template>
