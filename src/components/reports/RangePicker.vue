<script setup lang="ts">
import { computed } from "vue";
import DateInput from "@/components/pickers/DateInput.vue";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { fromLocalDate, presetRange, toLocalDate, type Preset, type Range } from "@/lib/dateRange";

// v-model holds plain strings so it can live in the route query
export interface RangeModel {
  preset: Preset;
  from?: string; // custom only, YYYY-MM-DD inclusive
  to?: string; // custom only, YYYY-MM-DD inclusive
}

const model = defineModel<RangeModel>({ required: true });
defineProps<{ presets?: Preset[] }>();

const ALL: Preset[] = ["today", "yesterday", "thisWeek", "lastWeek", "thisMonth", "lastMonth", "last3Months", "thisYear", "custom"];

const setPreset = (p: string) => {
  const preset = p as Preset;
  if (preset === "custom") {
    const r = resolveRange(model.value);
    model.value = { preset, from: toLocalDate(r.from), to: toLocalDate(new Date(r.to.getTime() - 1)) };
  } else {
    model.value = { preset };
  }
};

const custom = computed(() => model.value.preset === "custom");
</script>

<script lang="ts">
/** The [from, to) range a RangeModel means. */
export const resolveRange = (m: { preset: Preset; from?: string; to?: string }): Range => {
  if (m.preset === "custom" && m.from && m.to) {
    const from = fromLocalDate(m.from);
    const toIncl = fromLocalDate(m.to);
    return { from, to: new Date(toIncl.getFullYear(), toIncl.getMonth(), toIncl.getDate() + 1) };
  }
  return presetRange(m.preset === "custom" ? "thisMonth" : m.preset);
};
</script>

<template>
  <div class="flex flex-wrap items-center gap-2">
    <Select :model-value="model.preset" :key="$i18n.locale" @update:model-value="setPreset(String($event))">
      <SelectTrigger class="w-44"><SelectValue /></SelectTrigger>
      <SelectContent>
        <SelectItem v-for="p in presets ?? ALL" :key="p" :value="p">{{ $t(`range.${p}`) }}</SelectItem>
      </SelectContent>
    </Select>
    <template v-if="custom">
      <DateInput
        :model-value="model.from"
        class="w-48"
        :max="model.to"
        :aria-label="$t('range.from')"
        @update:model-value="model = { ...model, from: $event ?? undefined }"
      />
      <span class="text-muted-foreground">–</span>
      <DateInput
        :model-value="model.to"
        class="w-48"
        :min="model.from"
        :aria-label="$t('range.to')"
        @update:model-value="model = { ...model, to: $event ?? undefined }"
      />
    </template>
  </div>
</template>
