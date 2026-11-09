<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import { formatTime } from "@/lib/locale";
import { useAuth } from "@/stores/auth";
import { Printer } from "lucide-vue-next";
import { printSlip } from "@/lib/slip";
import { shiftSlip, type ShiftSummary } from "@/lib/reportSlips";

// Before clocking out: what the shift took, what the drawer should hold,
// and the cashier's count. The difference is saved with the drawer, and
// the shift report can be printed (before, or as the shift ends).
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ ended: [] }>();
const auth = useAuth();

const summary = ref<ShiftSummary | null>(null);
const counted = ref("");
const note = ref("");
const busy = ref(false);
const printAtEnd = ref(true);
const error = ref("");

watch(open, async (isOpen) => {
  if (!isOpen) return;
  counted.value = "";
  note.value = "";
  error.value = "";
  summary.value = null;
  try {
    summary.value = await call<ShiftSummary | null>("shift_summary");
  } catch (e) {
    error.value = errorMessage(e);
  }
});

const difference = computed(() =>
  counted.value === "" || !summary.value ? null : Number(counted.value) - summary.value.expectedCash,
);

const countedValue = () => (counted.value === "" ? null : Number(counted.value));
const printReport = async () => {
  if (!summary.value) return;
  error.value = "";
  try {
    await printSlip(shiftSlip(summary.value, auth.user.value?.fullName ?? "", countedValue()));
  } catch (e) {
    error.value = errorMessage(e);
  }
};

const submit = async () => {
  busy.value = true;
  error.value = "";
  try {
    // Printed first: the summary is gone once the shift has ended
    if (printAtEnd.value) await printReport();
    await auth.endShift({ countedCash: countedValue(), note: note.value || null });
    open.value = false;
    emit("ended");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-md">
      <form class="space-y-5" @submit.prevent="submit">
        <div>
          <DialogTitle class="text-lg font-semibold">{{ $t("shiftEnd.title") }}</DialogTitle>
          <DialogDescription v-if="summary" class="text-sm text-muted-foreground">
            {{ $t("shiftEnd.since", { time: formatTime(new Date(summary.clockedInAt)), n: summary.sales }) }}
          </DialogDescription>
        </div>

        <dl v-if="summary" class="space-y-1.5 rounded-lg bg-muted/60 p-4 text-sm">
          <div class="flex justify-between"><dt class="text-muted-foreground">{{ $t("shiftEnd.float") }}</dt><dd class="num">{{ formatCurrency(summary.openingFloat) }}</dd></div>
          <div class="flex justify-between"><dt class="text-muted-foreground">{{ $t("shiftEnd.cashSales") }}</dt><dd class="num">+{{ formatCurrency(summary.cashSales) }}</dd></div>
          <div v-if="summary.cashRefunds" class="flex justify-between"><dt class="text-muted-foreground">{{ $t("shiftEnd.cashRefunds") }}</dt><dd class="num">{{ formatCurrency(summary.cashRefunds) }}</dd></div>
          <div v-if="summary.paidToSuppliers" class="flex justify-between"><dt class="text-muted-foreground">{{ $t("shiftEnd.paidToSuppliers") }}</dt><dd class="num">{{ formatCurrency(summary.paidToSuppliers) }}</dd></div>
          <div v-if="summary.debtPaidCash" class="flex justify-between"><dt class="text-muted-foreground">{{ $t("shiftEnd.debtPaid") }}</dt><dd class="num">+{{ formatCurrency(summary.debtPaidCash) }}</dd></div>
          <div class="flex justify-between border-t pt-2 font-semibold"><dt>{{ $t("shiftEnd.expected") }}</dt><dd class="num">{{ formatCurrency(summary.expectedCash) }}</dd></div>
          <div class="flex justify-between pt-1 text-muted-foreground"><dt>{{ $t("shiftEnd.cardSales") }}</dt><dd class="num">{{ formatCurrency(summary.cardSales + summary.cardRefunds + summary.debtPaidCard) }}</dd></div>
          <div v-if="summary.onAccount" class="flex justify-between text-muted-foreground"><dt>{{ $t("shiftEnd.onAccount") }}</dt><dd class="num">{{ formatCurrency(summary.onAccount) }}</dd></div>
          <div v-if="summary.pointsPaid" class="flex justify-between text-muted-foreground"><dt>{{ $t("reports.pointsPaid") }}</dt><dd class="num">{{ formatCurrency(summary.pointsPaid) }}</dd></div>
          <div class="mt-1 space-y-1 border-t pt-2 text-muted-foreground">
            <div class="flex justify-between"><dt>{{ $t("reports.salesTotal") }}</dt><dd class="num">{{ formatCurrency(summary.salesTotal) }}</dd></div>
            <div v-if="summary.discounts" class="flex justify-between"><dt>{{ $t("reports.discounts") }}</dt><dd class="num">−{{ formatCurrency(summary.discounts) }}</dd></div>
            <div v-if="summary.promotions" class="flex justify-between"><dt>{{ $t("reports.promotions") }}</dt><dd class="num">−{{ formatCurrency(summary.promotions) }}</dd></div>
            <div v-if="summary.returns" class="flex justify-between"><dt>{{ $t("reports.returns", { n: summary.returns }) }}</dt><dd class="num">−{{ formatCurrency(summary.returnsTotal) }}</dd></div>
          </div>
        </dl>

        <div v-if="summary" class="space-y-2">
          <Label for="counted">{{ $t("shiftEnd.counted") }}</Label>
          <Input id="counted" v-model="counted" type="number" min="0" step="any" dir="ltr" autofocus class="h-12 text-xl" />
          <p
            v-if="difference !== null"
            class="text-sm font-medium"
            :class="difference === 0 ? 'text-success' : 'text-destructive'"
          >
            {{ difference === 0 ? $t("shiftEnd.balanced") : difference > 0 ? $t("shiftEnd.over", { amount: formatCurrency(difference) }) : $t("shiftEnd.short", { amount: formatCurrency(-difference) }) }}
          </p>
        </div>
        <div v-if="summary && difference" class="space-y-2">
          <Label for="shift-note">{{ $t("shiftEnd.note") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
          <Input id="shift-note" v-model="note" dir="auto" />
        </div>

        <label v-if="summary" class="flex items-center gap-2 text-sm">
          <input v-model="printAtEnd" type="checkbox" class="size-4 accent-primary" />
          {{ $t("reports.printAtEnd") }}
        </label>
        <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
        <div class="flex justify-end gap-2">
          <Button v-if="summary" type="button" variant="outline" class="me-auto" @click="printReport"><Printer /> {{ $t("reports.print") }}</Button>
          <Button type="button" variant="ghost" @click="open = false">{{ $t("common.cancel") }}</Button>
          <Button type="submit" variant="destructive" :disabled="busy">
            <Loader2 v-if="busy" class="animate-spin" />
            {{ $t("shiftEnd.confirm") }}
          </Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
