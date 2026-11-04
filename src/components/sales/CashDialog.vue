<script setup lang="ts">
import { computed, ref } from "vue";
import { Banknote } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { formatCurrency } from "@/lib/format";
import { useShop } from "@/stores/shop";

// Cash handed over → change due. Quick buttons are the notes in use for
// the shop's currency; typing works too. `ask` resolves with the amount
// tendered, or null if cancelled.
const NOTES: Record<string, number[]> = {
  IQD: [250, 500, 1000, 5000, 10000, 25000, 50000],
  USD: [1, 5, 10, 20, 50, 100],
};

const { currency } = useShop();
const open = ref(false);
const total = ref(0);
const tendered = ref("");
const input = ref<HTMLInputElement | null>(null);
let settle: ((n: number | null) => void) | null = null;

const ask = (amount: number) =>
  new Promise<number | null>((resolve) => {
    settle?.(null);
    settle = resolve;
    total.value = amount;
    tendered.value = "";
    open.value = true;
  });

const notes = computed(() => (NOTES[currency.value] ?? NOTES.USD!).filter((n) => n * 2 > total.value || n >= total.value / 10));
const given = computed(() => Number(tendered.value) || 0);
const change = computed(() => given.value - total.value);
// Exact (blank) or enough
const ok = computed(() => tendered.value === "" || change.value >= 0);

const add = (n: number) => {
  tendered.value = String(given.value + n);
  input.value?.focus();
};

const finish = (n: number | null) => {
  settle?.(n);
  settle = null;
  open.value = false;
};

defineExpose({ ask });
</script>

<template>
  <Dialog :open="open" @update:open="(v) => !v && finish(null)">
    <DialogContent class="max-w-md">
      <form class="space-y-5" @submit.prevent="ok && finish(tendered === '' ? total : given)">
        <div class="flex items-center gap-3">
          <span class="grid size-10 place-items-center rounded-full bg-success/12 text-success"><Banknote :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold">{{ $t("cash.title") }}</DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">{{ $t("cash.due", { amount: formatCurrency(total) }) }}</DialogDescription>
          </div>
        </div>
        <input
          ref="input"
          v-model="tendered"
          type="number"
          min="0"
          step="any"
          dir="ltr"
          autofocus
          :placeholder="formatCurrency(total)"
          :aria-label="$t('cash.tendered')"
          class="num h-16 w-full rounded-xl border-2 border-input bg-card px-4 text-center text-3xl font-semibold outline-none focus:border-primary"
        />
        <div class="grid grid-cols-4 gap-2">
          <Button type="button" variant="outline" class="h-11" @click="tendered = String(total)">{{ $t("cash.exact") }}</Button>
          <Button v-for="n in notes" :key="n" type="button" variant="outline" class="num h-11" dir="ltr" @click="add(n)">+{{ n.toLocaleString("en") }}</Button>
          <Button type="button" variant="ghost" class="h-11" @click="tendered = ''">{{ $t("picker.clear") }}</Button>
        </div>
        <div class="flex items-baseline justify-between rounded-xl px-4 py-3" :class="ok ? 'bg-success/10' : 'bg-destructive/10'">
          <span class="font-medium">{{ ok ? $t("cash.change") : $t("cash.short") }}</span>
          <span class="num text-3xl font-bold" :class="ok ? 'text-success' : 'text-destructive'">
            {{ formatCurrency(tendered === "" ? 0 : Math.abs(change)) }}
          </span>
        </div>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="finish(null)">{{ $t("common.cancel") }}</Button>
          <Button type="submit" variant="sun" size="lg" :disabled="!ok">{{ $t("cash.complete") }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
