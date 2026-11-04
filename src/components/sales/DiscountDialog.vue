<script setup lang="ts">
import { computed, ref } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { formatCurrency } from "@/lib/format";

// Asks for a discount on one line or the whole sale, as an amount or a
// percentage of `base`. `ask` resolves with the amount (0 removes it).
const open = ref(false);
const title = ref("");
const base = ref(0);
const mode = ref<"amount" | "percent">("percent");
const value = ref("");
let settle: ((n: number | null) => void) | null = null;

const ask = (opts: { title: string; base: number; current: number }) =>
  new Promise<number | null>((resolve) => {
    settle?.(null);
    settle = resolve;
    title.value = opts.title;
    base.value = opts.base;
    mode.value = opts.current ? "amount" : "percent";
    value.value = opts.current ? String(opts.current) : "";
    open.value = true;
  });

const amount = computed(() => {
  const n = Math.max(0, Number(value.value) || 0);
  return Math.min(base.value, mode.value === "percent" ? (base.value * Math.min(n, 100)) / 100 : n);
});

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
      <form class="space-y-4" @submit.prevent="finish(Math.round(amount * 100) / 100)">
        <div>
          <DialogTitle class="font-semibold">{{ $t("discount.title") }}</DialogTitle>
          <DialogDescription class="text-sm text-muted-foreground"><bdi>{{ title }}</bdi> — {{ formatCurrency(base) }}</DialogDescription>
        </div>
        <div class="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
          <button
            v-for="m in ['percent', 'amount'] as const"
            :key="m"
            type="button"
            role="radio"
            :aria-checked="mode === m"
            class="h-9 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm"
            @click="mode = m"
          >
            {{ $t(`discount.${m}`) }}
          </button>
        </div>
        <div class="relative">
          <Input v-model="value" type="number" min="0" step="any" dir="ltr" autofocus class="h-12 pe-12 text-xl" :aria-label="$t(`discount.${mode}`)" />
          <span class="absolute end-3 top-1/2 -translate-y-1/2 text-muted-foreground">{{ mode === "percent" ? "%" : "" }}</span>
        </div>
        <div v-if="mode === 'percent'" class="flex gap-2">
          <Button v-for="p in [5, 10, 15, 20]" :key="p" type="button" variant="outline" size="sm" class="num flex-1" @click="value = String(p)">{{ p }}%</Button>
        </div>
        <p class="flex justify-between text-sm">
          <span class="text-muted-foreground">{{ $t("discount.off") }}</span>
          <span class="num font-semibold">−{{ formatCurrency(amount) }}</span>
        </p>
        <div class="flex justify-between gap-2">
          <Button type="button" variant="ghost" class="text-destructive" @click="finish(0)">{{ $t("discount.remove") }}</Button>
          <Button type="submit">{{ $t("discount.apply") }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
