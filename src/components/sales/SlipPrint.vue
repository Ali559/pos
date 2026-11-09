<script setup lang="ts">
import { computed } from "vue";
import { printingSlip } from "@/lib/slip";

// A report slip (end of shift, end of day) on receipt paper. Shares
// #print-root with receipts (only one prints at a time).
const s = computed(() => printingSlip.value);
</script>

<template>
  <Teleport to="body">
    <div v-if="s" id="print-root" :style="{ width: `${s.width}mm` }" :class="s.width === 58 ? 'text-[10px]' : 'text-[12px]'">
      <template v-for="(l, i) in s.lines" :key="i">
        <p v-if="l.kind === 'title'" class="text-center text-[1.35em] font-bold">{{ l.text }}</p>
        <p v-else-if="l.kind === 'center'" class="text-center">{{ l.text }}</p>
        <p v-else-if="l.kind === 'heading'" class="mt-1.5 font-bold">{{ l.text }}</p>
        <div v-else-if="l.kind === 'rule'" class="rule my-1.5" />
        <p v-else class="flex justify-between gap-2" :class="[l.bold && 'font-bold', l.indent && 'ps-3']">
          <span>{{ l.text }}</span><span class="whitespace-nowrap">{{ l.value }}</span>
        </p>
      </template>
    </div>
  </Teleport>
</template>
