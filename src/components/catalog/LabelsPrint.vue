<script setup lang="ts">
import { computed } from "vue";
import { barcodeBars } from "@/lib/ean13";
import { formatCurrency } from "@/lib/format";
import { printingLabels } from "@/lib/labels";

// An A4 sheet of shelf labels, 3 across. Shares #print-root with receipts
// (only one prints at a time) so the same print CSS applies.
const labels = computed(() =>
  (printingLabels.value?.labels ?? []).map((l) => ({ ...l, barcode: barcodeBars(l.code) })),
);
</script>

<template>
  <Teleport to="body">
    <div v-if="printingLabels" id="print-root" style="width: 210mm">
      <div class="grid grid-cols-3 gap-[2mm] text-black">
        <div
          v-for="(l, i) in labels"
          :key="i"
          class="flex h-[36mm] flex-col items-center justify-between overflow-hidden rounded-[1mm] border border-black/60 p-[2mm] text-center [break-inside:avoid]"
        >
          <p class="line-clamp-2 text-[10pt] leading-tight font-bold"><bdi>{{ l.name }}</bdi></p>
          <p class="text-[18pt] leading-none font-extrabold">
            <span dir="ltr">{{ formatCurrency(l.price) }}</span><span v-if="l.per" class="text-[9pt] font-semibold"> / {{ l.per }}</span>
          </p>
          <div class="w-full">
            <svg class="mx-auto block h-[9mm] w-[85%]" :viewBox="`-4 0 ${l.barcode.width + 8} 40`" preserveAspectRatio="none" role="img" :aria-label="l.barcode.text">
              <rect v-for="([x, w], j) in l.barcode.bars" :key="j" :x="x" :width="w" y="0" height="40" fill="#000" />
            </svg>
            <p class="text-[7pt] tracking-wider" dir="ltr">{{ l.barcode.text }}</p>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>
