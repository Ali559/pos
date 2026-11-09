<script setup lang="ts">
import { computed } from "vue";
import { printingReport } from "@/lib/exportTable";
import { numberLocale, formatDateTime } from "@/lib/locale";
import { useShop } from "@/stores/shop";

// A report table on A4, for "Save as PDF" in the print dialog. Shares
// #print-root with receipts (only one prints at a time).
const { profile } = useShop();
const r = computed(() => printingReport.value);
const fmt = computed(() => new Intl.NumberFormat(numberLocale(), { numberingSystem: "latn", maximumFractionDigits: 2 }));
const cell = (c: string | number | null) => (c === null ? "" : typeof c === "number" ? fmt.value.format(c) : c);
</script>

<template>
  <Teleport to="body">
    <div v-if="r" id="print-root" class="report-page text-[10pt] text-black" style="width: 210mm">
      <header class="mb-4 flex items-end justify-between border-b-2 border-black pb-2">
        <div>
          <p class="text-[16pt] font-bold">{{ r.title }}</p>
          <p>{{ r.subtitle }}</p>
        </div>
        <div class="text-end">
          <p class="font-semibold">{{ profile?.name }}</p>
          <p class="text-[8pt]">{{ formatDateTime(new Date()) }}</p>
        </div>
      </header>
      <table class="w-full border-collapse">
        <thead>
          <tr>
            <th v-for="(h, i) in r.rows[0]" :key="i" class="border-b border-black px-2 py-1 font-bold" :class="i ? 'text-end' : 'text-start'">{{ cell(h) }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, j) in r.rows.slice(1)" :key="j" class="[break-inside:avoid] odd:bg-black/[0.04]">
            <td v-for="(c, i) in row" :key="i" class="px-2 py-1" :class="i ? 'text-end' : 'text-start'"><bdi>{{ cell(c) }}</bdi></td>
          </tr>
        </tbody>
      </table>
    </div>
  </Teleport>
</template>
