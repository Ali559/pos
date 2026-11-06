<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Banknote, Pencil, SlidersHorizontal } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import EditSheet from "@/components/EditSheet.vue";
import { getSupplier, getSupplierLedger, type Supplier, type SupplierEntry } from "@/db/suppliers";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import { formatDateTime } from "@/lib/locale";
import { useAuth } from "@/stores/auth";

// One supplier: what the shop owes them, every delivery and payment, and
// paying them.
const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ supplierId: number | null }>();
const emit = defineEmits<{ changed: []; edit: [supplier: Supplier] }>();

const auth = useAuth();
const supplier = ref<Supplier | null>(null);
const entries = ref<SupplierEntry[]>([]);
const newestFirst = computed(() => [...entries.value].reverse());

const load = async () => {
  if (props.supplierId === null) return;
  [supplier.value, entries.value] = await Promise.all([getSupplier(props.supplierId), getSupplierLedger(props.supplierId)]);
};
watch(open, (v) => v && load());

// Paying (or adjusting) — one small form, two modes
const mode = ref<null | "pay" | "adjust">(null);
const form = ref({ amount: "", method: "till" as "till" | "cash" | "bank", direction: "more" as "more" | "less", note: "" });
const error = ref("");
const start = (m: "pay" | "adjust") => {
  mode.value = mode.value === m ? null : m;
  form.value = { amount: m === "pay" && supplier.value ? String(Math.max(0, supplier.value.owed)) : "", method: "till", direction: "more", note: "" };
  error.value = "";
};
const submit = async () => {
  if (!supplier.value) return;
  error.value = "";
  const n = Number(form.value.amount) || 0;
  try {
    if (mode.value === "pay") {
      await call("pay_supplier", { supplierId: supplier.value.id, amount: n, method: form.value.method, note: form.value.note || null });
    } else {
      await call("adjust_supplier_balance", { supplierId: supplier.value.id, amount: form.value.direction === "more" ? n : -n, note: form.value.note });
    }
    mode.value = null;
    await load();
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  }
};

const signed = (n: number) => `${n > 0 ? "+" : "−"}${formatCurrency(Math.abs(n))}`;
</script>

<template>
  <EditSheet v-model:open="open" :title="supplier?.name ?? ''" :description="supplier?.phone ?? undefined" @submit="submit">
    <template v-if="supplier">
      <div class="rounded-xl border p-4">
        <p class="text-sm text-muted-foreground">{{ supplier.owed > 0 ? $t("suppliers.weOwe") : $t("suppliers.settled") }}</p>
        <p class="num text-3xl font-bold" :class="supplier.owed > 0 ? 'text-destructive' : ''">{{ formatCurrency(Math.max(0, supplier.owed)) }}</p>
        <p v-if="supplier.contact_person" class="mt-1 text-sm text-muted-foreground"><bdi>{{ supplier.contact_person }}</bdi></p>
        <div class="mt-3 flex flex-wrap gap-2">
          <Button v-if="auth.can('suppliers.pay') && supplier.owed > 0" type="button" @click="start('pay')"><Banknote />{{ $t("suppliers.pay") }}</Button>
          <Button v-if="auth.can('suppliers.pay')" type="button" variant="ghost" @click="start('adjust')"><SlidersHorizontal />{{ $t("customers.adjust") }}</Button>
          <Button v-if="auth.can('suppliers.edit')" type="button" variant="ghost" @click="emit('edit', supplier)"><Pencil />{{ $t("settings.edit") }}</Button>
        </div>
      </div>

      <div v-if="mode" class="space-y-3 rounded-xl border p-4">
        <p v-if="mode === 'adjust'" class="text-sm text-muted-foreground">{{ $t("suppliers.adjustHint") }}</p>
        <div v-if="mode === 'adjust'" class="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
          <button v-for="d in ['more', 'less'] as const" :key="d" type="button" role="radio" :aria-checked="form.direction === d"
            class="h-9 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm" @click="form.direction = d">
            {{ $t(`suppliers.adjust_${d}`) }}
          </button>
        </div>
        <div class="space-y-2">
          <Label for="sp-amount">{{ $t("customers.amount") }}</Label>
          <Input id="sp-amount" v-model="form.amount" type="number" min="0" step="any" dir="ltr" required />
        </div>
        <div v-if="mode === 'pay'" class="space-y-2">
          <Label>{{ $t("suppliers.paidFrom") }}</Label>
          <div class="grid grid-cols-3 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
            <button v-for="m in ['till', 'cash', 'bank'] as const" :key="m" type="button" role="radio" :aria-checked="form.method === m"
              class="h-9 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm" @click="form.method = m">
              {{ $t(`suppliers.methods.${m}`) }}
            </button>
          </div>
          <p class="text-xs text-muted-foreground">{{ $t(`suppliers.methodHints.${form.method}`) }}</p>
        </div>
        <div class="space-y-2">
          <Label for="sp-note">{{ mode === "adjust" ? $t("customers.reason") : $t("customers.noteOptional") }}</Label>
          <Input id="sp-note" v-model="form.note" dir="auto" :required="mode === 'adjust'" />
        </div>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <Button type="submit" :disabled="!(Number(form.amount) > 0)">{{ mode === "pay" ? $t("suppliers.recordPayment") : $t("common.save") }}</Button>
      </div>

      <section>
        <h3 class="mb-2 text-sm font-medium">{{ $t("customers.history") }}</h3>
        <p v-if="!entries.length" class="text-sm text-muted-foreground">{{ $t("customers.noHistory") }}</p>
        <ol v-else class="divide-y rounded-xl border">
          <li v-for="e in newestFirst" :key="e.id" class="flex items-start gap-3 px-4 py-2.5 text-sm">
            <div class="min-w-0 flex-1">
              <p class="font-medium">
                {{ $t(`suppliers.kinds.${e.kind}`) }}
                <span v-if="e.method" class="text-muted-foreground"> · {{ $t(`suppliers.methods.${e.method}`) }}</span>
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
</template>
