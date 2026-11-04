<script setup lang="ts">
import { computed, ref } from "vue";
import { Clock } from "lucide-vue-next";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

// Replaces <input type="time">: a 24-hour "HH:MM" value picked from an hour
// grid and a few minute marks, fast to tap on a touch till.
const model = defineModel<string>({ required: true });
const props = defineProps<{ id?: string; class?: string; minuteStep?: number }>();

const open = ref(false);
const pad = (n: number) => String(n).padStart(2, "0");
const hour = computed(() => Number(model.value?.split(":")[0] ?? 0));
const minute = computed(() => Number(model.value?.split(":")[1] ?? 0));
const minutes = computed(() => {
  const step = props.minuteStep ?? 15;
  return Array.from({ length: 60 / step }, (_, i) => i * step);
});

const setHour = (h: number) => {
  model.value = `${pad(h)}:${pad(minute.value)}`;
};
const setMinute = (m: number) => {
  model.value = `${pad(hour.value)}:${pad(m)}`;
  open.value = false;
};
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <button
        :id="id"
        type="button"
        :class="cn('num flex h-10 w-full items-center gap-2 rounded-md border border-input bg-card px-3 text-start text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 aria-expanded:border-ring', props.class)"
        dir="ltr"
      >
        <Clock :size="16" class="text-muted-foreground" />
        {{ model || "--:--" }}
      </button>
    </PopoverTrigger>
    <PopoverContent class="w-72">
      <p class="mb-1.5 text-xs text-muted-foreground">{{ $t("picker.hour") }}</p>
      <div class="grid grid-cols-6 gap-1" dir="ltr">
        <button
          v-for="h in 24"
          :key="h"
          type="button"
          class="num h-8 rounded-md text-sm hover:bg-muted aria-pressed:bg-primary aria-pressed:text-primary-foreground"
          :aria-pressed="hour === h - 1"
          @click="setHour(h - 1)"
        >
          {{ pad(h - 1) }}
        </button>
      </div>
      <p class="mt-3 mb-1.5 text-xs text-muted-foreground">{{ $t("picker.minute") }}</p>
      <div class="grid grid-cols-4 gap-1" dir="ltr">
        <button
          v-for="m in minutes"
          :key="m"
          type="button"
          class="num h-8 rounded-md text-sm hover:bg-muted aria-pressed:bg-primary aria-pressed:text-primary-foreground"
          :aria-pressed="minute === m"
          @click="setMinute(m)"
        >
          :{{ pad(m) }}
        </button>
      </div>
    </PopoverContent>
  </Popover>
</template>
