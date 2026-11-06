<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { Loader2, Tags } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import DateInput from "@/components/pickers/DateInput.vue";
import { listProducts, type Product, type ProductQuery } from "@/db/catalog";
import { select } from "@/db/client";
import { getSetting } from "@/db/settings";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import { perUnit, printLabels, renderLabel, type LabelItem } from "@/lib/labels";

// Which labels to print: what the Inventory list shows now, or everything
// whose price changed since a date (to re-label shelves after a price
// change). Then how many of each and where.
const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ query: ProductQuery }>();

const source = ref<"list" | "changed">("list");
const since = ref(new Date(Date.now() - 7 * 86_400_000).toLocaleDateString("en-CA"));
const copies = ref("1");
const format = ref<"sheet" | "printer">("sheet");
const hasPrinter = ref(false);
const products = ref<Product[]>([]);
const busy = ref(false);
const error = ref("");
const previewSrc = ref("");

onMounted(async () => {
  hasPrinter.value = !!(await getSetting("receipt_printer"));
});

const load = async () => {
  error.value = "";
  const all = await listProducts({ ...props.query, limit: undefined, offset: undefined });
  if (source.value === "list") {
    products.value = all.filter((p) => !p.openPrice);
  } else {
    const changed = await select<{ product_id: number }>(
      "SELECT DISTINCT product_id FROM price_history WHERE field = 'price' AND changed_at >= $1",
      [new Date(`${since.value}T00:00`).toISOString().replace(/\.\d{3}Z$/, "Z")],
    );
    const ids = new Set(changed.map((c) => c.product_id));
    const everything = await listProducts({ includeInactive: false });
    products.value = everything.filter((p) => ids.has(p.id) && !p.openPrice);
  }
};
watch(open, (v) => v && load());
watch([source, since], () => open.value && load());

const items = computed<LabelItem[]>(() => {
  const n = Math.max(1, Math.min(50, Math.round(Number(copies.value) || 1)));
  return products.value.flatMap((p) =>
    Array.from({ length: n }, () => ({ name: p.name, price: p.price, code: p.barcode || p.sku, per: perUnit(p.unit) })),
  );
});

// A picture of the first label, as the receipt printer would print it
watch(
  [items, format, open],
  async () => {
    if (!open.value || format.value !== "printer" || !items.value[0]) return (previewSrc.value = "");
    const width = (await getSetting("receipt_width")) === "58" ? 58 : 80;
    const r = renderLabel(items.value[0], width);
    const bytes = Uint8Array.from(atob(r.data), (c) => c.charCodeAt(0));
    const W = r.widthBytes * 8;
    const canvas = document.createElement("canvas");
    canvas.width = W;
    canvas.height = r.height;
    const img = canvas.getContext("2d")!.createImageData(W, r.height);
    for (let i = 0; i < W * r.height; i++) {
      const on = bytes[(Math.floor(i / W) * r.widthBytes) + ((i % W) >> 3)]! & (0x80 >> (i % W & 7));
      img.data.set(on ? [0, 0, 0, 255] : [255, 255, 255, 255], i * 4);
    }
    canvas.getContext("2d")!.putImageData(img, 0, 0);
    previewSrc.value = canvas.toDataURL();
  },
  { immediate: true },
);

const print = async () => {
  busy.value = true;
  error.value = "";
  try {
    await printLabels(items.value, format.value);
    open.value = false;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-lg">
      <form class="space-y-4" @submit.prevent="print">
        <div class="flex items-center gap-3">
          <span class="grid size-10 shrink-0 place-items-center rounded-full bg-primary/10 text-primary"><Tags :size="20" /></span>
          <div>
            <DialogTitle class="font-semibold">{{ $t("labels.title") }}</DialogTitle>
            <DialogDescription class="text-sm text-muted-foreground">{{ $t("labels.hint") }}</DialogDescription>
          </div>
        </div>
        <div class="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
          <button v-for="s in ['list', 'changed'] as const" :key="s" type="button" role="radio" :aria-checked="source === s"
            class="h-9 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm" @click="source = s">{{ $t(`labels.source.${s}`) }}</button>
        </div>
        <label v-if="source === 'changed'" class="flex items-center gap-2 text-sm">
          {{ $t("labels.since") }}
          <DateInput v-model="since" class="w-48" />
        </label>
        <div class="flex flex-wrap items-center gap-4 text-sm">
          <label class="flex items-center gap-2">{{ $t("labels.copies") }} <Input v-model="copies" type="number" min="1" max="50" dir="ltr" class="w-20" /></label>
          <div class="grid grid-cols-2 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
            <button v-for="f in ['sheet', 'printer'] as const" :key="f" type="button" role="radio" :aria-checked="format === f" :disabled="f === 'printer' && !hasPrinter"
              class="h-9 rounded-md px-3 text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm disabled:opacity-40" @click="format = f">{{ $t(`labels.format.${f}`) }}</button>
          </div>
        </div>
        <p v-if="!hasPrinter" class="text-xs text-muted-foreground">{{ $t("labels.noPrinter") }}</p>

        <div class="max-h-48 overflow-y-auto rounded-lg border">
          <p v-if="!products.length" class="p-4 text-center text-sm text-muted-foreground">{{ $t("labels.none") }}</p>
          <ul v-else class="divide-y text-sm">
            <li v-for="p in products" :key="p.id" class="flex justify-between px-3 py-1.5">
              <bdi>{{ p.name }}</bdi><span class="num">{{ formatCurrency(p.price) }}</span>
            </li>
          </ul>
        </div>
        <img v-if="previewSrc" :src="previewSrc" alt="" class="mx-auto max-h-40 rounded border" />

        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <div class="flex justify-end gap-2">
          <Button type="button" variant="ghost" @click="open = false">{{ $t("common.cancel") }}</Button>
          <Button type="submit" :disabled="busy || !items.length"><Loader2 v-if="busy" class="animate-spin" />{{ $t("labels.print", items.length) }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
