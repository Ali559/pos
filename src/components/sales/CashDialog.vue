<script setup lang="ts">
import { computed, ref } from "vue";
import { Banknote, CreditCard, DollarSign } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { formatCurrency, formatRate, formatUsd } from "@/lib/format";
import { useShop } from "@/stores/shop";
import { toIqd } from "@/stores/rate";

// Cash handed over → change due. Quick buttons are the notes in use for
// the shop's currency; typing works too. With card payments on, part can go
// on a card (a split payment) and the cash covers the rest. When the shop
// takes dollars (`usd`: the rate, IQD per 100 USD), the customer can hand
// over dollars, with or without dinars; change is always given in dinars.
// `ask` resolves with the dinars and dollars tendered and the card part, or
// null if cancelled.
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
const rate = ref<number | null>(null);
const dollars = ref("");
const input = ref<HTMLInputElement | null>(null);
type Answer = { tendered: number; card: number; usd: number };
let settle: ((a: Answer | null) => void) | null = null;

const ask = (amount: number, opts: { split?: boolean; usd?: number | null } = {}) =>
  new Promise<Answer | null>((resolve) => {
    settle?.(null);
    settle = resolve;
    due.value = amount;
    tendered.value = "";
    card.value = "";
    dollars.value = "";
    splitAllowed.value = !!opts.split;
    splitting.value = false;
    rate.value = opts.usd ?? null;
    open.value = true;
  });

const USD_NOTES = [1, 5, 10, 20, 50, 100];
const usd = computed(() => Math.max(0, Number(dollars.value) || 0));
const usdValue = computed(() => (rate.value && usd.value > 0 ? toIqd(usd.value, rate.value) : 0));
const addUsd = (n: number) => {
  dollars.value = String(Math.round((usd.value + n) * 100) / 100);
};

const cardPart = computed(() => (splitting.value ? Math.min(due.value, Math.max(0, Number(card.value) || 0)) : 0));
// What's left to pay in cash
const total = computed(() => due.value - cardPart.value);

const notes = computed(() => (NOTES[currency.value] ?? NOTES.USD!).filter((n) => n * 2 > total.value || n >= total.value / 10));
const given = computed(() => Number(tendered.value) || 0);
const change = computed(() => given.value + usdValue.value - total.value);
// Dollars given while the dinars already cover it
const usdNotNeeded = computed(() => usd.value > 0 && given.value >= total.value);
// Exact (blank) or enough
const ok = computed(() => !usdNotNeeded.value && ((tendered.value === "" && usd.value === 0) || change.value >= 0));
const showChange = computed(() => tendered.value !== "" || usd.value > 0);

const add = (n: number) => {
  tendered.value = String(given.value + n);
  input.value?.focus();
};

const submit = () => {
  if (!ok.value) return;
  // With dollars, blank dinars means none
  if (usd.value > 0) return finish(given.value);
  finish(tendered.value === "" ? total.value : given.value);
};

const finish = (n: number | null) => {
  settle?.(n === null ? null : { tendered: n, card: cardPart.value, usd: usd.value });
  settle = null;
  open.value = false;
};

defineExpose({ ask });
</script>

<template>
  <Dialog :open="open" @update:open="(v) => !v && finish(null)">
    <DialogContent class="max-w-md">
      <form class="space-y-5" @submit.prevent="submit">
        <div class="flex items-center gap-3">
          <span class="grid size-10 place-items-center rounded-full bg-success/12 text-success"><Banknote :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold">{{ $t("cash.title") }}</DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">
              {{ $t("cash.due", { amount: formatCurrency(total) }) }}
              <span v-if="rate" class="num" dir="ltr"> ≈ {{ formatUsd(Math.ceil((total / rate) * 10000) / 100) }}</span>
            </DialogDescription>
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
        <div v-if="rate" class="space-y-2 rounded-xl border border-border p-3">
          <div class="flex items-center justify-between gap-2">
            <label for="cash-usd" class="flex items-center gap-1.5 text-sm font-medium"><DollarSign :size="16" class="text-muted-foreground" />{{ $t("cash.dollars") }}</label>
            <span class="num text-xs text-muted-foreground" dir="ltr">{{ $t("cash.rate", { rate: formatRate(rate) }) }}</span>
          </div>
          <input
            id="cash-usd"
            v-model="dollars"
            type="number"
            min="0"
            step="any"
            dir="ltr"
            placeholder="$0"
            class="num h-12 w-full rounded-lg border border-input bg-card px-3 text-center text-2xl font-semibold outline-none focus:border-primary"
          />
          <div class="grid grid-cols-4 gap-2">
            <Button v-for="n in USD_NOTES" :key="n" type="button" variant="outline" class="num h-11" dir="ltr" @click="addUsd(n)">+${{ n }}</Button>
            <Button type="button" variant="ghost" class="h-11" @click="dollars = ''">{{ $t("picker.clear") }}</Button>
          </div>
          <p v-if="usd > 0" class="num text-sm text-muted-foreground">
            {{ $t("cash.dollarsWorth", { usd: formatUsd(usd), amount: formatCurrency(usdValue) }) }}
          </p>
          <p v-if="usdNotNeeded" role="alert" class="text-sm text-destructive">{{ $t("cash.usdNotNeeded") }}</p>
        </div>
        <div class="flex items-baseline justify-between rounded-xl px-4 py-3" :class="ok ? 'bg-success/10' : 'bg-destructive/10'">
          <span class="font-medium">{{ ok ? $t("cash.change") : $t("cash.short") }}</span>
          <span class="num text-3xl font-bold" :class="ok ? 'text-success' : 'text-destructive'">
            {{ formatCurrency(showChange ? Math.abs(change) : 0) }}
          </span>
        </div>
        <p v-if="rate && usd > 0 && change > 0" class="-mt-3 text-xs text-muted-foreground">{{ $t("cash.changeInDinars") }}</p>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="finish(null)">{{ $t("common.cancel") }}</Button>
          <Button type="submit" variant="sun" size="lg" :disabled="!ok">{{ $t("cash.complete") }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
