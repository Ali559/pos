<script setup lang="ts">
import { computed, ref } from "vue";
import { NotebookPen, TriangleAlert } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import type { Customer } from "@/db/customers";
import { formatCurrency } from "@/lib/format";

// Selling on account (قەرز): how much the customer pays now in cash, if
// anything; the rest goes on their account. Resolves with the cash paid
// now, or null when cancelled.
const open = ref(false);
const due = ref(0);
const customer = ref<Customer | null>(null);
const paid = ref("");
let settle: ((n: number | null) => void) | null = null;

const ask = (opts: { due: number; customer: Customer }) =>
  new Promise<number | null>((resolve) => {
    settle?.(null);
    settle = resolve;
    due.value = opts.due;
    customer.value = opts.customer;
    paid.value = "";
    open.value = true;
  });

const paidNow = computed(() => Math.min(due.value, Math.max(0, Number(paid.value) || 0)));
const onAccount = computed(() => due.value - paidNow.value);
const after = computed(() => (customer.value?.balance ?? 0) + onAccount.value);
const overLimit = computed(() => customer.value?.credit_limit != null && after.value > customer.value.credit_limit + 1e-6);

const finish = (n: number | null) => {
  settle?.(n);
  settle = null;
  open.value = false;
};

defineExpose({ ask });
</script>

<template>
  <Dialog :open="open" @update:open="(v) => !v && finish(null)">
    <DialogContent class="max-w-sm">
      <form v-if="customer" class="space-y-4" @submit.prevent="onAccount > 0 && finish(paidNow)">
        <div class="flex items-center gap-3">
          <span class="grid size-10 shrink-0 place-items-center rounded-full bg-primary/10 text-primary"><NotebookPen :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold">{{ $t("onAccount.title") }}</DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground"><bdi>{{ customer.name }}</bdi></DialogDescription>
          </div>
        </div>
        <label class="block space-y-1.5">
          <span class="text-sm font-medium">{{ $t("onAccount.paidNow") }}</span>
          <input
            v-model="paid"
            type="number"
            min="0"
            :max="due"
            step="any"
            dir="ltr"
            autofocus
            placeholder="0"
            class="num h-14 w-full rounded-xl border-2 border-input bg-card px-4 text-center text-2xl font-semibold outline-none focus:border-primary"
          />
        </label>
        <dl class="space-y-1 rounded-lg bg-muted p-3 text-sm">
          <div class="flex justify-between"><dt>{{ $t("pos.total") }}</dt><dd class="num">{{ formatCurrency(due) }}</dd></div>
          <div class="flex justify-between font-semibold"><dt>{{ $t("onAccount.onAccount") }}</dt><dd class="num">{{ formatCurrency(onAccount) }}</dd></div>
          <div class="flex justify-between text-muted-foreground"><dt>{{ $t("onAccount.balanceAfter") }}</dt><dd class="num">{{ formatCurrency(after) }}</dd></div>
        </dl>
        <p v-if="overLimit" role="alert" class="flex items-start gap-2 text-sm text-destructive">
          <TriangleAlert :size="16" class="mt-0.5 shrink-0" />{{ $t("onAccount.overLimit", { limit: formatCurrency(customer.credit_limit ?? 0) }) }}
        </p>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="finish(null)">{{ $t("common.cancel") }}</Button>
          <Button type="submit" :disabled="!(onAccount > 0)">{{ $t("onAccount.confirm") }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
