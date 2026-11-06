<script setup lang="ts">
import { ref, watch } from "vue";
import { Banknote, CreditCard, HandCoins } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import type { Customer } from "@/db/customers";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";

// A customer paying off (part of) their debt. Cash goes in the drawer.
const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ customer: Customer | null }>();
const emit = defineEmits<{ paid: [balance: number] }>();

const amount = ref("");
const method = ref<"cash" | "card">("cash");
const note = ref("");
const busy = ref(false);
const error = ref("");

watch(open, (v) => {
  if (!v) return;
  amount.value = props.customer ? String(props.customer.balance) : "";
  method.value = "cash";
  note.value = "";
  error.value = "";
});

const submit = async () => {
  if (!props.customer) return;
  busy.value = true;
  error.value = "";
  try {
    const r = await call<{ balance: number }>("record_customer_payment", {
      customerId: props.customer.id,
      amount: Number(amount.value) || 0,
      method: method.value,
      note: note.value || null,
    });
    open.value = false;
    emit("paid", r.balance);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-sm">
      <form v-if="customer" class="space-y-4" @submit.prevent="submit">
        <div class="flex items-center gap-3">
          <span class="grid size-10 shrink-0 place-items-center rounded-full bg-success/12 text-success"><HandCoins :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold">{{ $t("customers.takePayment") }}</DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">
              <bdi>{{ customer.name }}</bdi> — {{ $t("customers.owes", { amount: formatCurrency(customer.balance) }) }}
            </DialogDescription>
          </div>
        </div>
        <input
          v-model="amount"
          type="number"
          min="0"
          :max="customer.balance"
          step="any"
          dir="ltr"
          autofocus
          :aria-label="$t('customers.amount')"
          class="num h-16 w-full rounded-xl border-2 border-input bg-card px-4 text-center text-3xl font-semibold outline-none focus:border-primary"
        />
        <div class="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
          <button
            v-for="m in [{ id: 'cash', icon: Banknote }, { id: 'card', icon: CreditCard }] as const"
            :key="m.id"
            type="button"
            role="radio"
            :aria-checked="method === m.id"
            class="flex h-9 items-center justify-center gap-2 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm"
            @click="method = m.id"
          >
            <component :is="m.icon" :size="16" />{{ $t(`pos.${m.id}`) }}
          </button>
        </div>
        <Input v-model="note" dir="auto" :placeholder="$t('customers.noteOptional')" :aria-label="$t('customers.noteOptional')" />
        <p v-if="Number(amount) > 0 && Number(amount) < customer.balance" class="text-sm text-muted-foreground">
          {{ $t("customers.leftAfter", { amount: formatCurrency(customer.balance - Number(amount)) }) }}
        </p>
        <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="open = false">{{ $t("common.cancel") }}</Button>
          <Button type="submit" :disabled="busy || !(Number(amount) > 0)">{{ $t("customers.record") }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
