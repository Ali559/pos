<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { CalendarDays, ChevronLeft, ChevronRight, X } from "lucide-vue-next";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { fromLocalDate, toLocalDate } from "@/lib/dateRange";
import { formatDate, formatMonthYear, shortWeekday } from "@/lib/locale";
import { cn } from "@/lib/utils";

// Replaces <input type="date">, which looks different in every webview and
// ignores the app's language. The value stays a "YYYY-MM-DD" local date.
const model = defineModel<string | null | undefined>({ required: true });
const props = defineProps<{
  id?: string;
  min?: string;
  max?: string;
  placeholder?: string;
  clearable?: boolean;
  required?: boolean;
  class?: string;
}>();

// Weeks start on Saturday, as in the schedule and reports
const WEEK = [6, 0, 1, 2, 3, 4, 5];

const open = ref(false);
const view = ref(new Date());
const grid = ref<HTMLElement | null>(null);

watch(open, (isOpen) => {
  if (!isOpen) return;
  const base = model.value ? fromLocalDate(model.value) : new Date();
  view.value = new Date(base.getFullYear(), base.getMonth(), 1);
  nextTick(() => (grid.value?.querySelector<HTMLButtonElement>("[aria-selected=true], [data-today]") ?? grid.value?.querySelector("button:not(:disabled)"))?.focus());
});

const days = computed(() => {
  const first = new Date(view.value.getFullYear(), view.value.getMonth(), 1);
  const lead = (first.getDay() - 6 + 7) % 7;
  return Array.from({ length: 42 }, (_, i) => new Date(first.getFullYear(), first.getMonth(), i - lead + 1));
});

const today = toLocalDate(new Date());
const inMonth = (d: Date) => d.getMonth() === view.value.getMonth();
const disabled = (d: Date) => {
  const s = toLocalDate(d);
  return (!!props.min && s < props.min) || (!!props.max && s > props.max);
};

const shiftMonth = (by: number) => {
  view.value = new Date(view.value.getFullYear(), view.value.getMonth() + by, 1);
};
const pick = (d: Date) => {
  if (disabled(d)) return;
  model.value = toLocalDate(d);
  open.value = false;
};

// Arrow keys move through days in visual order (mirrors in RTL)
const onKey = (e: KeyboardEvent) => {
  const buttons = [...(grid.value?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
  const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
  if (i < 0) return;
  const rtl = getComputedStyle(grid.value!).direction === "rtl";
  const step = { ArrowRight: rtl ? -1 : 1, ArrowLeft: rtl ? 1 : -1, ArrowDown: 7, ArrowUp: -7 }[e.key];
  if (step === undefined) return;
  e.preventDefault();
  buttons[Math.min(buttons.length - 1, Math.max(0, i + step))]?.focus();
};

const label = computed(() => (model.value ? formatDate(fromLocalDate(model.value)) : ""));
</script>

<template>
  <Popover v-model:open="open">
    <div :class="cn('relative', props.class)">
      <PopoverTrigger as-child>
        <button
          :id="id"
          type="button"
          class="flex h-10 w-full items-center gap-2 rounded-md border border-input bg-card ps-3 pe-9 text-start text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 aria-expanded:border-ring"
          :aria-required="required"
        >
          <CalendarDays :size="16" class="shrink-0 text-muted-foreground" />
          <span v-if="label" class="truncate">{{ label }}</span>
          <span v-else class="truncate text-muted-foreground">{{ placeholder ?? $t("picker.pickDate") }}</span>
        </button>
      </PopoverTrigger>
      <button
        v-if="clearable && model"
        type="button"
        class="absolute end-2 top-1/2 grid size-6 -translate-y-1/2 place-items-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
        :aria-label="$t('picker.clear')"
        @click="model = null"
      >
        <X :size="14" />
      </button>
    </div>

    <PopoverContent class="w-[19rem]">
      <div class="mb-2 flex items-center justify-between">
        <button type="button" class="grid size-8 place-items-center rounded-md hover:bg-muted" :aria-label="$t('picker.previousMonth')" @click="shiftMonth(-1)">
          <ChevronLeft :size="18" class="rtl:rotate-180" />
        </button>
        <span class="font-semibold" aria-live="polite">{{ formatMonthYear(view) }}</span>
        <button type="button" class="grid size-8 place-items-center rounded-md hover:bg-muted" :aria-label="$t('picker.nextMonth')" @click="shiftMonth(1)">
          <ChevronRight :size="18" class="rtl:rotate-180" />
        </button>
      </div>
      <div class="grid grid-cols-7 text-center text-xs text-muted-foreground">
        <span v-for="d in WEEK" :key="d" class="py-1">{{ shortWeekday(d) }}</span>
      </div>
      <div ref="grid" class="grid grid-cols-7 gap-0.5" role="grid" @keydown="onKey">
        <button
          v-for="d in days"
          :key="d.toISOString()"
          type="button"
          role="gridcell"
          :disabled="disabled(d)"
          :aria-selected="model === toLocalDate(d)"
          :data-today="toLocalDate(d) === today ? '' : undefined"
          :aria-label="formatDate(d)"
          class="num grid h-9 place-items-center rounded-md text-sm transition-colors hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none disabled:pointer-events-none disabled:opacity-30 aria-selected:bg-primary aria-selected:font-semibold aria-selected:text-primary-foreground"
          :class="[!inMonth(d) && 'text-muted-foreground/60', toLocalDate(d) === today && 'ring-1 ring-primary/50']"
          @click="pick(d)"
        >
          {{ d.getDate() }}
        </button>
      </div>
      <div class="mt-2 flex justify-between border-t pt-2">
        <button
          type="button"
          class="rounded-md px-2 py-1 text-sm font-medium text-primary hover:bg-primary/10 disabled:opacity-40"
          :disabled="disabled(new Date())"
          @click="pick(new Date())"
        >
          {{ $t("range.today") }}
        </button>
        <button
          v-if="clearable"
          type="button"
          class="rounded-md px-2 py-1 text-sm text-muted-foreground hover:bg-muted"
          @click="model = null; open = false"
        >
          {{ $t("picker.clear") }}
        </button>
      </div>
    </PopoverContent>
  </Popover>
</template>
