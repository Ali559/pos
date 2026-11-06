<script setup lang="ts">
import { computed, ref } from "vue";
import { Scale } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { formatCurrency } from "@/lib/format";

// How much of something sold by weight or volume (kg, liter). `ask`
// resolves with the quantity, or null when cancelled.
const open = ref(false);
const name = ref("");
const unit = ref("");
const price = ref(0);
const max = ref(Infinity);
const value = ref("");
let settle: ((n: number | null) => void) | null = null;

const ask = (opts: { name: string; unit: string; price: number; current?: number; max?: number }) =>
  new Promise<number | null>((resolve) => {
    settle?.(null);
    settle = resolve;
    name.value = opts.name;
    unit.value = opts.unit;
    price.value = opts.price;
    max.value = opts.max ?? Infinity;
    value.value = opts.current ? String(opts.current) : "";
    open.value = true;
  });

const qty = computed(() => Math.round(Math.max(0, Number(value.value) || 0) * 1000) / 1000);
const tooMuch = computed(() => qty.value > max.value + 1e-9);

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
      <form class="space-y-4" @submit.prevent="qty > 0 && !tooMuch && finish(qty)">
        <div class="flex items-center gap-3">
          <span class="grid size-10 shrink-0 place-items-center rounded-full bg-primary/10 text-primary"><Scale :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold"><bdi>{{ name }}</bdi></DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">{{ $t("weigh.per", { price: formatCurrency(price), unit }) }}</DialogDescription>
          </div>
        </div>
        <div class="relative">
          <input
            v-model="value"
            type="number"
            min="0"
            step="0.001"
            dir="ltr"
            autofocus
            :aria-label="$t('weigh.quantity', { unit })"
            class="num h-16 w-full rounded-xl border-2 border-input bg-card px-4 pe-16 text-center text-3xl font-semibold outline-none focus:border-primary"
          />
          <span class="absolute end-4 top-1/2 -translate-y-1/2 text-muted-foreground">{{ unit }}</span>
        </div>
        <div class="flex gap-2">
          <Button v-for="q in [0.25, 0.5, 1, 2]" :key="q" type="button" variant="outline" size="sm" class="num flex-1" @click="value = String(q)">{{ q }}</Button>
        </div>
        <p class="flex justify-between text-sm">
          <span class="text-muted-foreground">{{ $t("pos.total") }}</span>
          <span class="num font-semibold">{{ formatCurrency(price * qty) }}</span>
        </p>
        <p v-if="tooMuch" role="alert" class="text-sm text-destructive">{{ $t("weigh.tooMuch", { n: max, unit }) }}</p>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="finish(null)">{{ $t("common.cancel") }}</Button>
          <Button type="submit" :disabled="!(qty > 0) || tooMuch">{{ $t("weigh.add") }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
