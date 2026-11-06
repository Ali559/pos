<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { HandCoins, Pencil, Printer, SlidersHorizontal } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import EditSheet from "@/components/EditSheet.vue";
import CustomerPaymentDialog from "./CustomerPaymentDialog.vue";
import { getCustomer, getLedger, type Customer, type LedgerEntry } from "@/db/customers";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import { formatDateTime } from "@/lib/locale";
import { printStatement } from "@/lib/print";
import { useAuth } from "@/stores/auth";

// One customer's account: what they owe, every entry with the running
// balance, and the actions on it.
const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ customerId: number | null }>();
const emit = defineEmits<{ changed: []; edit: [customer: Customer] }>();

const auth = useAuth();
const customer = ref<Customer | null>(null);
const entries = ref<LedgerEntry[]>([]);
const payOpen = ref(false);

const load = async () => {
  if (props.customerId === null) return;
  [customer.value, entries.value] = await Promise.all([getCustomer(props.customerId), getLedger(props.customerId)]);
};
watch(open, (v) => v && load());

const newestFirst = computed(() => [...entries.value].reverse());
const overLimit = computed(() => customer.value?.credit_limit != null && customer.value.balance > customer.value.credit_limit);

const changed = async () => {
  await load();
  emit("changed");
};

// Opening balance from the old notebook, or a correction
const adjusting = ref(false);
const adjust = ref({ direction: "more" as "more" | "less", amount: "", note: "" });
const adjustError = ref("");
const saveAdjust = async () => {
  if (!customer.value) return;
  adjustError.value = "";
  const n = Number(adjust.value.amount) || 0;
  try {
    await call("adjust_customer_balance", {
      customerId: customer.value.id,
      amount: adjust.value.direction === "more" ? n : -n,
      note: adjust.value.note,
    });
    adjusting.value = false;
    adjust.value = { direction: "more", amount: "", note: "" };
    await changed();
  } catch (e) {
    adjustError.value = errorMessage(e);
  }
};

const signed = (n: number) => `${n > 0 ? "+" : "−"}${formatCurrency(Math.abs(n))}`;
</script>

<template>
  <EditSheet v-model:open="open" :title="customer?.name ?? ''" :description="customer?.phone ?? undefined" @submit="() => {}">
    <template v-if="customer">
      <div class="rounded-xl border p-4">
        <p class="text-sm text-muted-foreground">{{ customer.balance > 0 ? $t("customers.owesNow") : $t("customers.settled") }}</p>
        <p class="num text-3xl font-bold" :class="customer.balance > 0 ? 'text-destructive' : ''">{{ formatCurrency(Math.max(0, customer.balance)) }}</p>
        <p v-if="customer.balance < 0" class="text-sm text-success">{{ $t("customers.inCredit", { amount: formatCurrency(-customer.balance) }) }}</p>
        <p v-if="customer.credit_limit != null" class="mt-1 text-sm" :class="overLimit ? 'text-destructive' : 'text-muted-foreground'">
          {{ $t("customers.limitIs", { amount: formatCurrency(customer.credit_limit) }) }}
        </p>
        <div class="mt-3 flex flex-wrap gap-2">
          <Button v-if="auth.can('customers.credit') && customer.balance > 0" type="button" @click="payOpen = true"><HandCoins />{{ $t("customers.takePayment") }}</Button>
          <Button type="button" variant="outline" :disabled="!entries.length" @click="printStatement(customer, entries)"><Printer />{{ $t("customers.printStatement") }}</Button>
          <Button v-if="auth.can('customers.edit')" type="button" variant="ghost" @click="adjusting = !adjusting"><SlidersHorizontal />{{ $t("customers.adjust") }}</Button>
          <Button v-if="auth.can('customers.edit')" type="button" variant="ghost" @click="emit('edit', customer)"><Pencil />{{ $t("settings.edit") }}</Button>
        </div>
      </div>

      <div v-if="adjusting" class="space-y-3 rounded-xl border p-4">
        <p class="text-sm text-muted-foreground">{{ $t("customers.adjustHint") }}</p>
        <div class="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
          <button
            v-for="d in ['more', 'less'] as const"
            :key="d"
            type="button"
            role="radio"
            :aria-checked="adjust.direction === d"
            class="h-9 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm"
            @click="adjust.direction = d"
          >
            {{ $t(`customers.adjust_${d}`) }}
          </button>
        </div>
        <div class="space-y-2">
          <Label for="adj-amount">{{ $t("customers.amount") }}</Label>
          <Input id="adj-amount" v-model="adjust.amount" type="number" min="0" step="any" dir="ltr" />
        </div>
        <div class="space-y-2">
          <Label for="adj-note">{{ $t("customers.reason") }}</Label>
          <Input id="adj-note" v-model="adjust.note" dir="auto" :placeholder="$t('customers.reasonPlaceholder')" />
        </div>
        <p v-if="adjustError" role="alert" class="text-sm text-destructive">{{ adjustError }}</p>
        <Button type="button" :disabled="!(Number(adjust.amount) > 0) || !adjust.note.trim()" @click="saveAdjust">{{ $t("common.save") }}</Button>
      </div>

      <section>
        <h3 class="mb-2 text-sm font-medium">{{ $t("customers.history") }}</h3>
        <p v-if="!entries.length" class="text-sm text-muted-foreground">{{ $t("customers.noHistory") }}</p>
        <ol v-else class="divide-y rounded-xl border">
          <li v-for="e in newestFirst" :key="e.id" class="flex items-start gap-3 px-4 py-2.5 text-sm">
            <div class="min-w-0 flex-1">
              <p class="font-medium">
                {{ $t(`customers.kinds.${e.kind}`) }}
                <span v-if="e.receipt_number" class="num text-muted-foreground" dir="ltr">{{ e.receipt_number }}</span>
                <span v-if="e.method" class="text-muted-foreground"> · {{ $t(`pos.${e.method}`) }}</span>
              </p>
              <p class="text-xs text-muted-foreground">{{ formatDateTime(new Date(e.created_at)) }} · {{ e.user }}</p>
              <p v-if="e.note" class="text-xs italic text-muted-foreground"><bdi>{{ e.note }}</bdi></p>
            </div>
            <div class="text-end">
              <p class="num font-semibold" :class="e.amount > 0 ? 'text-destructive' : 'text-success'">{{ signed(e.amount) }}</p>
              <p class="num text-xs text-muted-foreground">{{ formatCurrency(e.running) }}</p>
            </div>
          </li>
        </ol>
      </section>
    </template>
    <template #footer>
      <Button type="button" variant="ghost" @click="open = false">{{ $t("common.close") }}</Button>
    </template>
  </EditSheet>
  <CustomerPaymentDialog v-model:open="payOpen" :customer="customer" @paid="changed" />
</template>
