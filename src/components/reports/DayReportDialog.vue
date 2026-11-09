<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Loader2, Printer } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import DateInput from "@/components/pickers/DateInput.vue";
import { dayReport, type DayReport } from "@/db/reports";
import { fromLocalDate, toLocalDate } from "@/lib/dateRange";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import { formatDate } from "@/lib/locale";
import { daySlip } from "@/lib/reportSlips";
import { printSlip } from "@/lib/slip";
import { i18n } from "@/utils/i18n";

// The end-of-day (Z) report: the day's takings by payment method, refunds,
// discounts, and each cashier's share, with their drawer over/short.
const open = defineModel<boolean>("open", { required: true });

const day = ref<string | null>(toLocalDate(new Date()));
const report = ref<DayReport | null>(null);
const loading = ref(false);
const error = ref("");

const range = computed(() => {
  const from = fromLocalDate(day.value ?? toLocalDate(new Date()));
  return { from, to: new Date(from.getFullYear(), from.getMonth(), from.getDate() + 1) };
});
const label = computed(() => formatDate(range.value.from));

const load = async () => {
  loading.value = true;
  error.value = "";
  try {
    report.value = await dayReport(range.value);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
};
watch([open, day], ([isOpen]) => isOpen && load());

const methodName = (m: string) => (i18n.global.te(`pos.${m}`) ? i18n.global.t(`pos.${m}`) : m);

const print = async () => {
  if (!report.value) return;
  error.value = "";
  try {
    await printSlip(daySlip(report.value, label.value));
  } catch (e) {
    error.value = errorMessage(e);
  }
};
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-h-[90vh] max-w-2xl overflow-y-auto">
      <div class="space-y-5">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div>
            <DialogTitle class="text-lg font-semibold">{{ $t("reports.dayTitle") }}</DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">{{ $t("reports.dayHint") }}</DialogDescription>
          </div>
          <DateInput v-model="day" class="w-60" />
        </div>

        <div v-if="loading && !report" class="grid h-40 place-items-center text-muted-foreground"><Loader2 class="animate-spin" /></div>
        <template v-else-if="report">
          <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <div class="rounded-lg bg-muted/60 p-3">
              <p class="text-xs text-muted-foreground">{{ $t("reports.salesTotal") }}</p>
              <p class="num text-lg font-bold">{{ formatCurrency(report.totals.total) }}</p>
            </div>
            <div class="rounded-lg bg-muted/60 p-3">
              <p class="text-xs text-muted-foreground">{{ $t("reports.sales") }}</p>
              <p class="num text-lg font-bold">{{ report.totals.sales }}</p>
            </div>
            <div class="rounded-lg bg-muted/60 p-3">
              <p class="text-xs text-muted-foreground">{{ $t("reports.discounts") }}</p>
              <p class="num text-lg font-bold">{{ formatCurrency(report.totals.discounts + report.totals.promotions) }}</p>
            </div>
            <div class="rounded-lg bg-muted/60 p-3">
              <p class="text-xs text-muted-foreground">{{ $t("reports.refunds") }}</p>
              <p class="num text-lg font-bold">{{ formatCurrency(report.totals.refunded) }}</p>
            </div>
          </div>

          <section class="space-y-2">
            <h3 class="text-sm font-semibold">{{ $t("reports.byMethod") }}</h3>
            <p v-if="!report.methods.length" class="text-sm text-muted-foreground">{{ $t("reports.nothing") }}</p>
            <dl v-else class="divide-y rounded-lg border text-sm">
              <div v-for="m in report.methods" :key="m.method" class="flex justify-between gap-2 px-3 py-2">
                <dt>{{ methodName(m.method) }}</dt>
                <dd class="num text-end">
                  {{ formatCurrency(m.taken + m.refunded) }}
                  <span v-if="m.refunded" class="block text-xs text-muted-foreground">{{ $t("reports.ofWhichRefunded") }} {{ formatCurrency(m.refunded) }}</span>
                </dd>
              </div>
              <div v-if="report.totals.debtCash || report.totals.debtCard" class="flex justify-between px-3 py-2 text-muted-foreground">
                <dt>{{ $t("reports.debtRepaid") }}</dt><dd class="num">{{ formatCurrency(report.totals.debtCash + report.totals.debtCard) }}</dd>
              </div>
              <div v-if="report.totals.suppliers" class="flex justify-between px-3 py-2 text-muted-foreground">
                <dt>{{ $t("shiftEnd.paidToSuppliers") }}</dt><dd class="num">{{ formatCurrency(report.totals.suppliers) }}</dd>
              </div>
            </dl>
          </section>

          <section class="space-y-2">
            <h3 class="text-sm font-semibold">{{ $t("reports.byCashier") }}</h3>
            <p v-if="!report.cashiers.length" class="text-sm text-muted-foreground">{{ $t("reports.nothing") }}</p>
            <div v-else class="overflow-x-auto rounded-lg border">
              <table class="w-full text-sm">
                <thead class="bg-muted/60 text-xs text-muted-foreground">
                  <tr>
                    <th class="px-3 py-2 text-start font-medium">{{ $t("receipt.cashier") }}</th>
                    <th class="px-3 py-2 text-end font-medium">{{ $t("reports.sales") }}</th>
                    <th class="px-3 py-2 text-end font-medium">{{ $t("reports.salesTotal") }}</th>
                    <th class="px-3 py-2 text-end font-medium">{{ $t("pos.cash") }}</th>
                    <th class="px-3 py-2 text-end font-medium">{{ $t("pos.card") }}</th>
                    <th class="px-3 py-2 text-end font-medium">{{ $t("reports.discounts") }}</th>
                    <th class="px-3 py-2 text-end font-medium">{{ $t("reports.refunds") }}</th>
                    <th class="px-3 py-2 text-end font-medium">{{ $t("reports.drawer") }}</th>
                  </tr>
                </thead>
                <tbody class="divide-y">
                  <tr v-for="c in report.cashiers" :key="c.user_id">
                    <td class="px-3 py-2 font-medium"><bdi>{{ c.full_name }}</bdi></td>
                    <td class="num px-3 py-2 text-end">{{ c.sales }}</td>
                    <td class="num px-3 py-2 text-end font-semibold">{{ formatCurrency(c.total) }}</td>
                    <td class="num px-3 py-2 text-end">{{ formatCurrency(c.cash) }}</td>
                    <td class="num px-3 py-2 text-end">{{ formatCurrency(c.card) }}</td>
                    <td class="num px-3 py-2 text-end">{{ formatCurrency(c.discounts) }}</td>
                    <td class="num px-3 py-2 text-end">{{ formatCurrency(c.refunded) }}</td>
                    <td class="num px-3 py-2 text-end" :class="c.drawer ? (c.drawer > 0 ? 'text-success' : 'text-destructive') : ''">
                      {{ c.drawer === null ? "—" : c.drawer === 0 ? $t("shiftEnd.balanced") : (c.drawer > 0 ? "+" : "−") + formatCurrency(Math.abs(c.drawer)) }}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </section>
        </template>

        <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="open = false">{{ $t("common.close") }}</Button>
          <Button type="button" :disabled="!report" @click="print"><Printer /> {{ $t("reports.print") }}</Button>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
