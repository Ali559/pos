<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Package, ScanSearch } from "lucide-vue-next";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import ProductThumb from "@/components/catalog/ProductThumb.vue";
import { getProductById, isWeighed, stockState, type Product } from "@/db/catalog";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import type { ScanResult } from "@/lib/scan";

// Scan or type a code to see the price, pack prices and stock without
// touching the cart. Stays open for the next scan.
const open = defineModel<boolean>("open", { required: true });
const { t } = useI18n();

const code = ref("");
const input = ref<HTMLInputElement | null>(null);
const found = ref<{ product: Product; scan: ScanResult } | null>(null);
const message = ref("");

watch(open, (v) => {
  if (!v) return;
  code.value = "";
  found.value = null;
  message.value = "";
  nextTick(() => input.value?.focus());
});

const look = async () => {
  const c = code.value.trim();
  if (!c) return;
  message.value = "";
  try {
    const scan = await call<ScanResult | null>("resolve_scan", { code: c });
    const product = scan ? await getProductById(scan.productId) : null;
    found.value = scan && product ? { product, scan } : null;
    if (!found.value) message.value = t("pos.notFound", { code: c });
  } catch (e) {
    message.value = errorMessage(e);
  }
  code.value = "";
  input.value?.focus();
};

const unitLabel = (u: string) => t(`catalog.units.${u}`).toLocaleLowerCase();
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-md">
      <div class="space-y-4">
        <div>
          <DialogTitle class="flex items-center gap-2 font-semibold"><ScanSearch :size="18" />{{ $t("priceCheck.title") }}</DialogTitle>
          <DialogDescription class="text-sm text-muted-foreground">{{ $t("priceCheck.hint") }}</DialogDescription>
        </div>
        <form @submit.prevent="look">
          <input
            ref="input"
            v-model="code"
            inputmode="numeric"
            autocomplete="off"
            dir="ltr"
            :aria-label="$t('pos.scanBarcode')"
            :placeholder="$t('pos.barcodePlaceholder')"
            class="num h-12 w-full rounded-xl border-2 border-input bg-card px-4 text-lg outline-none focus:border-primary"
          />
        </form>
        <p v-if="message" role="alert" class="text-sm font-medium text-destructive">{{ message }}</p>
        <div v-if="found" class="space-y-3 rounded-xl border p-4" role="status">
          <div class="flex items-center gap-3">
            <ProductThumb :product="found.product" :size="48" />
            <div class="min-w-0 flex-1">
              <p class="text-lg leading-snug font-semibold"><bdi>{{ found.product.name }}</bdi></p>
              <p v-if="found.product.trackStock" class="text-sm" :class="stockState(found.product) !== 'ok' ? 'text-destructive' : 'text-muted-foreground'">
                {{ $t("priceCheck.inStock", { n: found.product.stock, unit: unitLabel(found.product.unit) }) }}
              </p>
            </div>
          </div>
          <p class="flex items-baseline justify-between">
            <span class="text-muted-foreground">
              {{ found.product.openPrice ? $t("pos.priceAtTill") : isWeighed(found.product.unit) ? $t("priceCheck.perUnit", { unit: unitLabel(found.product.unit) }) : $t("priceCheck.single") }}
            </span>
            <span v-if="!found.product.openPrice" class="num text-3xl font-bold">{{ formatCurrency(found.product.price) }}</span>
          </p>
          <!-- A scale label: what this label would charge -->
          <p v-if="found.scan.scaleBarcode" class="flex justify-between rounded-lg bg-muted px-3 py-2 text-sm">
            <span>{{ $t("priceCheck.label", { n: found.scan.quantity, unit: unitLabel(found.product.unit) }) }}</span>
            <span class="num font-semibold">{{ formatCurrency(found.scan.amount ?? found.scan.quantity * found.product.price) }}</span>
          </p>
          <ul v-if="found.product.units.length" class="space-y-1 border-t pt-3 text-sm">
            <li v-for="u in found.product.units" :key="u.id" class="flex items-center gap-2" :class="u.id === found.scan.unitId ? 'font-semibold' : ''">
              <Package :size="14" class="text-muted-foreground" />
              <bdi class="flex-1">{{ u.name }} ({{ u.factor }})</bdi>
              <span class="num">{{ formatCurrency(u.price) }}</span>
            </li>
          </ul>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
