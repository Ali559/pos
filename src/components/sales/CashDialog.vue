<script setup lang="ts">
import { computed, ref } from "vue";
import { Banknote, CreditCard } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { formatCurrency } from "@/lib/format";
import { useShop } from "@/stores/shop";

// Cash handed over → change due. Quick buttons are the notes in use for
// the shop's currency; typing works too. With card payments on, part can go
// on a card (a split payment) and the cash covers the rest. `ask` resolves
// with the cash tendered and the card part, or null if cancelled.
const NOTES: Record<string, number[]> = {
  IQD: [250, 500, 1000, 5000, 10000, 25000, 50000],
  USD: [1, 5, 10, 20, 50, 100],
};

const { currency } = useShop();
const open = ref(false);
const due = ref(0);
const tendered = ref("");
const card = ref("");
const splitAllowed = ref(false);
const splitting = ref(false);
const input = ref<HTMLInputElement | null>(null);
type Answer = { tendered: number; card: number };
let settle: ((a: Answer | null) => void) | null = null;

const ask = (amount: number, opts: { split?: boolean } = {}) =>
  new Promise<Answer | null>((resolve) => {
    settle?.(null);
    settle = resolve;
    due.value = amount;
    tendered.value = "";
    card.value = "";
    splitAllowed.value = !!opts.split;
    splitting.value = false;
    open.value = true;
  });

const cardPart = computed(() => (splitting.value ? Math.min(due.value, Math.max(0, Number(card.value) || 0)) : 0));
// What's left to pay in cash
const total = computed(() => due.value - cardPart.value);

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
  settle?.(n === null ? null : { tendered: n, card: cardPart.value });
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
        <div v-if="splitAllowed" class="space-y-2">
          <label class="flex items-center gap-2 text-sm font-medium">
            <input v-model="splitting" type="checkbox" class="size-4 accent-primary" />
            <CreditCard :size="16" class="text-muted-foreground" />
            {{ $t("cash.splitCard") }}
          </label>
          <div v-if="splitting" class="flex items-center gap-2">
            <input
              v-model="card"
              type="number"
              min="0"
              :max="due"
              step="any"
              dir="ltr"
              :aria-label="$t('cash.cardPart')"
              :placeholder="$t('cash.cardPart')"
              class="num h-11 w-full rounded-lg border border-input bg-card px-3 text-lg outline-none focus:border-primary"
            />
          </div>
          <p v-if="splitting && cardPart > 0" class="text-xs text-muted-foreground">
            {{ $t("cash.splitSummary", { card: formatCurrency(cardPart), cash: formatCurrency(total) }) }}
          </p>
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
