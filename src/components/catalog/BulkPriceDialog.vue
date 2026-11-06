<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { TrendingUp } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { listProducts, type Product, type ProductQuery } from "@/db/catalog";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import { roundMoney } from "@/lib/pricing";
import { useShop } from "@/stores/shop";

// Raise or lower the price of every product the Inventory filter shows, by
// a percentage or an amount, rounded to a step. Shows the new prices
// before anything changes. Rust does the same sums (pricing::bulk_new_price).
const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ query: ProductQuery }>();
const emit = defineEmits<{ done: [changed: number] }>();
const { currency } = useShop();

const products = ref<Product[]>([]);
const mode = ref<"percent" | "amount">("percent");
const direction = ref<"up" | "down">("up");
const value = ref("");
const roundTo = ref("0");
const busy = ref(false);
const error = ref("");

watch(open, async (v) => {
  if (!v) return;
  error.value = "";
  value.value = "";
  products.value = (await listProducts({ ...props.query, limit: undefined, offset: undefined })).filter((p) => !p.openPrice);
});

const signed = computed(() => (direction.value === "up" ? 1 : -1) * (Number(value.value) || 0));
const newPrice = (old: number) => {
  const raw = mode.value === "percent" ? old * (1 + signed.value / 100) : old + signed.value;
  const step = Number(roundTo.value) || 0;
  return Math.max(0, step > 0 ? Math.round(raw / step) * step : roundMoney(raw, currency.value));
};
const changes = computed(() => products.value.map((p) => ({ p, to: newPrice(p.price) })).filter((c) => Math.abs(c.to - c.p.price) > 1e-9));

const apply = async () => {
  busy.value = true;
  error.value = "";
  try {
    const n = await call<number>("bulk_update_prices", {
      productIds: products.value.map((p) => p.id),
      mode: mode.value,
      value: signed.value,
      roundTo: Number(roundTo.value) || 0,
    });
    open.value = false;
    emit("done", n);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-lg">
      <form class="space-y-4" @submit.prevent="apply">
        <div class="flex items-center gap-3">
          <span class="grid size-10 shrink-0 place-items-center rounded-full bg-primary/10 text-primary"><TrendingUp :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold">{{ $t("bulkPrices.title") }}</DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">{{ $t("bulkPrices.scope", { n: products.length }) }}</DialogDescription>
          </div>
        </div>
        <div class="flex flex-wrap items-center gap-2">
          <div class="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
            <button v-for="d in ['up', 'down'] as const" :key="d" type="button" role="radio" :aria-checked="direction === d"
              class="h-9 rounded-md px-3 text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm" @click="direction = d">{{ $t(`bulkPrices.${d}`) }}</button>
          </div>
          <Input v-model="value" type="number" min="0" step="any" dir="ltr" class="w-28" required :aria-label="$t('bulkPrices.by')" />
          <div class="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
            <button v-for="m in ['percent', 'amount'] as const" :key="m" type="button" role="radio" :aria-checked="mode === m"
              class="h-9 rounded-md px-3 text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm" @click="mode = m">{{ m === "percent" ? "%" : currency }}</button>
          </div>
        </div>
        <label class="flex items-center gap-2 text-sm">
          {{ $t("bulkPrices.roundTo") }}
          <select v-model="roundTo" class="h-9 rounded-md border bg-card px-2 text-sm">
            <option value="0">{{ $t("bulkPrices.noRounding") }}</option>
            <option v-for="r in ['50', '100', '250', '500', '1000']" :key="r" :value="r">{{ formatCurrency(Number(r)) }}</option>
          </select>
        </label>
        <div v-if="Number(value) > 0" class="max-h-60 overflow-y-auto rounded-lg border">
          <p v-if="!changes.length" class="p-4 text-center text-sm text-muted-foreground">{{ $t("bulkPrices.nothing") }}</p>
          <table v-else class="w-full text-sm">
            <tbody class="divide-y">
              <tr v-for="c in changes" :key="c.p.id">
                <td class="px-3 py-1.5"><bdi>{{ c.p.name }}</bdi></td>
                <td class="num px-3 py-1.5 text-end text-muted-foreground line-through">{{ formatCurrency(c.p.price) }}</td>
                <td class="num px-3 py-1.5 text-end font-semibold">{{ formatCurrency(c.to) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="open = false">{{ $t("common.cancel") }}</Button>
          <Button type="submit" :disabled="busy || !changes.length">{{ $t("bulkPrices.apply", { n: changes.length }) }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
