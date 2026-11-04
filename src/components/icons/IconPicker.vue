<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { watchDebounced } from "@vueuse/core";
import { Search, X } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import AppIcon from "./AppIcon.vue";
import {
  recentIcons,
  rememberIcon,
  searchIcons,
  suggestIcons,
} from "@/lib/icons";

const icon = defineModel<string | null>({ required: true });
const props = defineProps<{
  /** Name typed so far, in any language: drives the suggestions */
  suggestFrom?: string;
  /** Parent category's icon, to favour related icons */
  boostFrom?: string | null;
  clearable?: boolean;
}>();

const COLUMNS = 8;
const suggestions = ref<string[]>([]);
const browsing = ref(false);
const query = ref("");
const results = ref<string[]>([]);
const grid = ref<HTMLElement | null>(null);

watchDebounced(
  () => [props.suggestFrom, props.boostFrom] as const,
  async ([text, boost]) => {
    suggestions.value = text?.trim()
      ? (await suggestIcons(text, boost)).map((i) => i.key)
      : [];
  },
  { debounce: 200, immediate: true },
);

const runSearch = async () => {
  results.value = query.value.trim()
    ? (await searchIcons(query.value)).map((i) => i.key)
    : [...new Set([...recentIcons(), ...(await searchIcons("")).map((i) => i.key)])].slice(0, 160);
};
watchDebounced(query, runSearch, { debounce: 150 });
watch(browsing, (open) => open && runSearch());

const pick = (key: string) => {
  icon.value = key;
  rememberIcon(key);
};

const label = (key: string) => key.split(":")[1]?.replace(/-/g, " ") ?? key;

// Arrow keys move through the grid (visual order, so it mirrors in RTL)
const onGridKey = (e: KeyboardEvent) => {
  const buttons = [...(grid.value?.querySelectorAll("button") ?? [])];
  const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
  if (i < 0) return;
  const rtl = getComputedStyle(grid.value!).direction === "rtl";
  const step = {
    ArrowRight: rtl ? -1 : 1,
    ArrowLeft: rtl ? 1 : -1,
    ArrowDown: COLUMNS,
    ArrowUp: -COLUMNS,
  }[e.key];
  if (step === undefined) return;
  e.preventDefault();
  buttons[Math.min(buttons.length - 1, Math.max(0, i + step))]?.focus();
};

const hasSuggestions = computed(() => suggestions.value.length > 0);
</script>

<template>
  <div class="space-y-3">
    <div class="flex items-center gap-3">
      <div
        class="grid size-12 shrink-0 place-items-center rounded-lg border bg-muted text-foreground"
        :class="!icon && 'border-dashed text-muted-foreground'"
      >
        <AppIcon v-if="icon" :icon="icon" :size="26" />
        <span v-else class="text-xs">—</span>
      </div>
      <div class="min-w-0 flex-1 text-sm">
        <div class="truncate font-medium capitalize" dir="ltr">
          {{ icon ? label(icon) : $t("icons.none") }}
        </div>
      </div>
      <Button type="button" variant="outline" size="sm" @click="browsing = !browsing">
        <Search />
        {{ browsing ? $t("icons.hide") : $t("icons.browse") }}
      </Button>
      <Button
        v-if="clearable && icon"
        type="button"
        variant="ghost"
        size="icon-sm"
        :aria-label="$t('common.remove')"
        @click="icon = null"
      >
        <X />
      </Button>
    </div>

    <div v-if="hasSuggestions">
      <p class="mb-1.5 text-xs text-muted-foreground">{{ $t("icons.suggested") }}</p>
      <div class="flex flex-wrap gap-1.5">
        <button
          v-for="key in suggestions"
          :key="key"
          type="button"
          class="grid size-10 place-items-center rounded-lg border bg-card hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none aria-pressed:border-primary aria-pressed:bg-primary/10 aria-pressed:text-primary"
          :aria-pressed="icon === key"
          :aria-label="label(key)"
          :title="label(key)"
          @click="pick(key)"
        >
          <AppIcon :icon="key" :size="22" />
        </button>
      </div>
    </div>

    <div v-if="browsing" class="space-y-2 rounded-lg border p-3">
      <div class="relative">
        <Search
          :size="16"
          class="pointer-events-none absolute start-2.5 top-1/2 -translate-y-1/2 text-muted-foreground"
        />
        <input
          v-model="query"
          type="search"
          :placeholder="$t('icons.searchPlaceholder')"
          :aria-label="$t('icons.searchPlaceholder')"
          class="h-9 w-full rounded-md border border-input bg-card ps-8 pe-3 text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        />
      </div>
      <p v-if="!query" class="text-xs text-muted-foreground">{{ $t("icons.recentFirst") }}</p>
      <div
        ref="grid"
        class="grid max-h-64 grid-cols-8 gap-1 overflow-y-auto"
        role="listbox"
        :aria-label="$t('icons.browse')"
        @keydown="onGridKey"
      >
        <button
          v-for="key in results"
          :key="key"
          type="button"
          role="option"
          :aria-selected="icon === key"
          :aria-label="label(key)"
          :title="label(key)"
          class="grid aspect-square place-items-center rounded-md hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none aria-selected:bg-primary/10 aria-selected:text-primary"
          @click="pick(key)"
        >
          <AppIcon :icon="key" :size="20" />
        </button>
      </div>
      <p v-if="query && results.length === 0" class="py-4 text-center text-sm text-muted-foreground">
        {{ $t("icons.noResults") }}
      </p>
    </div>
  </div>
</template>
