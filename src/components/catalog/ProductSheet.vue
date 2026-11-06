<script setup lang="ts">
import DateInput from "@/components/pickers/DateInput.vue";
import { computed, nextTick, ref, watch } from "vue";
import { Loader2, Package, Plus, ScanBarcode, X } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { guessCategory, lookupBarcode } from "@/lib/barcodeLookup";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import EditSheet from "@/components/EditSheet.vue";
import IconPicker from "@/components/icons/IconPicker.vue";
import MediaGallery from "@/components/media/MediaGallery.vue";
import { getProduct, isWeighed, type Category } from "@/db/catalog";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import type { MediaFile } from "@/lib/media";

const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{
  /** null creates a new product */
  productId: number | null;
  categories: Category[];
}>();
const emit = defineEmits<{
  saved: [];
  /** The scanned barcode already belongs to this product: open it instead */
  switch: [id: number];
}>();

const { t } = useI18n();
const UNITS = ["piece", "kg", "liter", "pack", "box"] as const;

const blank = () => ({
  name: "", sku: "", categoryId: "none",
  barcode: "", extraBarcodes: [] as string[], costPrice: "", sellingPrice: "",
  minStock: "0", unit: "piece" as string, iconKey: null as string | null,
  isActive: true, initialStock: "0", initialExpiry: "", stock: 0,
  units: [] as PackDraft[], plu: "", openPrice: false, trackStock: true,
});

/** A pack size being edited; id is kept for existing packs */
interface PackDraft { id: number | null; name: string; factor: string; barcode: string; price: string }
const addPack = () =>
  form.value.units.push({ id: null, name: form.value.units.length ? "" : t("catalog.packs.carton"), factor: "", barcode: "", price: "" });
/** Per-item price inside a pack, to compare with the single price */
const perItem = (u: PackDraft) => {
  const f = Number(u.factor);
  const pr = Number(u.price);
  return f > 1 && u.price !== "" ? pr / f : null;
};

const form = ref(blank());
const images = ref<MediaFile[]>([]);
const saving = ref(false);
const uploading = ref(false);
const loading = ref(false);
const error = ref("");
const scanInput = ref<HTMLInputElement | null>(null);
const looking = ref(false);
// What the last scan did, shown under the barcode field
const scanNote = ref<{ tone: "info" | "warn"; text: string } | null>(null);
// Survives the reload when a scan switches to an existing product
let carriedNote: typeof scanNote.value = null;

const load = async () => {
  error.value = "";
  scanNote.value = carriedNote;
  carriedNote = null;
  form.value = blank();
  images.value = [];
  if (props.productId === null) {
    // New product: the cursor waits in the barcode field for a scan
    nextTick(() => scanInput.value?.focus());
    return;
  }
  loading.value = true;
  const p = await getProduct(props.productId);
  loading.value = false;
  if (!p) return;
  form.value = {
    name: p.name,
    sku: p.sku,
    categoryId: p.category_id ? String(p.category_id) : "none",
    barcode: p.barcode ?? "",
    extraBarcodes: [...p.extraBarcodes],
    costPrice: String(p.cost_price),
    sellingPrice: String(p.selling_price),
    minStock: String(p.min_stock_level),
    unit: p.unit,
    iconKey: p.icon_key,
    isActive: !!p.is_active,
    initialStock: "0",
    initialExpiry: "",
    stock: p.stock_quantity,
    units: p.units.map((u) => ({ id: u.id, name: u.name, factor: String(u.factor), barcode: u.barcode ?? "", price: String(u.price) })),
    plu: p.plu ?? "",
    openPrice: !!p.open_price,
    trackStock: !!p.track_stock,
  };
  images.value = p.media;
};
watch(open, (isOpen) => isOpen && load());
watch(() => props.productId, () => open.value && load());

// Scanners type the code and press Enter. A known barcode opens that
// product; a new one is looked up online to fill what's still empty.
const onScan = async () => {
  const code = form.value.barcode.trim();
  if (!code || props.productId !== null || looking.value) return;
  looking.value = true;
  scanNote.value = null;
  try {
    const r = await lookupBarcode(code);
    if (r.existingProductId) {
      carriedNote = { tone: "info", text: t("catalog.scan.exists") };
      emit("switch", r.existingProductId);
      return;
    }
    if (r.source === "openfoodfacts") {
      if (!form.value.name && r.name) form.value.name = r.name;
      if (!images.value.length && r.image) images.value = [r.image];
      if (form.value.categoryId === "none") {
        const guess = guessCategory(r.categoryTags, props.categories);
        if (guess) form.value.categoryId = String(guess.id);
      }
      scanNote.value = { tone: "info", text: t("catalog.scan.filled") };
    } else {
      scanNote.value = { tone: "warn", text: r.offline ? t("catalog.scan.offline") : t("catalog.scan.unknown") };
    }
    nextTick(() => document.getElementById(form.value.name ? "p-price" : "p-name")?.focus());
  } catch (e) {
    scanNote.value = { tone: "warn", text: errorMessage(e) };
  } finally {
    looking.value = false;
  }
};

// Subcategories listed under their parent
const categoryOptions = computed(() =>
  props.categories.filter((c) => c.is_active).map((c) => ({
    id: c.id,
    label: (c.parent_id ? "— " : "") + c.name,
  })),
);
const selectedCategory = computed(() =>
  props.categories.find((c) => c.id === Number(form.value.categoryId)),
);
const suggestText = computed(() => form.value.name);

const margin = computed(() => {
  const cost = Number(form.value.costPrice);
  const price = Number(form.value.sellingPrice);
  if (!price || form.value.costPrice === "") return null;
  return { amount: price - cost, percent: Math.round(((price - cost) / price) * 100) };
});

const save = async () => {
  const f = form.value;
  const product = {
    name: f.name,
    sku: f.sku,
    categoryId: f.categoryId === "none" ? null : Number(f.categoryId),
    barcode: f.barcode,
    extraBarcodes: f.extraBarcodes.filter((b) => b.trim()),
    costPrice: Number(f.costPrice) || 0,
    sellingPrice: Number(f.sellingPrice) || 0,
    minStock: Number(f.minStock) || 0,
    unit: f.unit,
    iconKey: f.iconKey,
    mediaIds: images.value.map((m) => m.id),
    isActive: f.isActive,
    initialStock: props.productId === null ? Number(f.initialStock) || 0 : null,
    initialExpiry: props.productId === null && f.initialExpiry ? f.initialExpiry : null,
    units: f.units
      .filter((u) => u.name.trim() || u.factor || u.price)
      .map((u) => ({ id: u.id, name: u.name, factor: Number(u.factor) || 0, barcode: u.barcode.trim() || null, price: Number(u.price) || 0 })),
    plu: isWeighed(f.unit) ? f.plu.trim() || null : null,
    openPrice: f.openPrice,
    trackStock: f.trackStock,
  };
  saving.value = true;
  error.value = "";
  try {
    if (props.productId === null) await call("create_product", product);
    else await call("update_product", { id: props.productId, product });
    open.value = false;
    emit("saved");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};
</script>

<template>
  <EditSheet
    v-model:open="open"
    :title="productId === null ? $t('inventory.addProduct') : $t('inventory.edit')"
    @submit="save"
  >
    <div v-if="loading" class="grid place-items-center py-20"><Loader2 class="animate-spin" /></div>
    <template v-else>
      <!-- Barcode first: scanning fills the rest when it can -->
      <div class="space-y-2">
        <Label for="p-barcode">{{ $t("inventory.table.barcode") }}</Label>
        <div class="relative">
          <ScanBarcode :size="18" class="pointer-events-none absolute start-3 top-1/2 -translate-y-1/2 text-primary" />
          <input
            id="p-barcode"
            ref="scanInput"
            v-model="form.barcode"
            dir="ltr"
            inputmode="numeric"
            autocomplete="off"
            :placeholder="productId === null ? $t('catalog.scan.placeholder') : ''"
            class="num h-11 w-full rounded-md border-2 border-input bg-card ps-10 pe-10 text-base outline-none focus-visible:border-primary"
            @keydown.enter.prevent="onScan"
          />
          <Loader2 v-if="looking" :size="18" class="absolute end-3 top-1/2 -translate-y-1/2 animate-spin text-muted-foreground" />
        </div>
        <p
          v-if="scanNote"
          role="status"
          class="text-sm"
          :class="scanNote.tone === 'warn' ? 'text-warning' : 'text-muted-foreground'"
        >
          {{ scanNote.text }}
        </p>
        <p v-else-if="productId === null" class="text-xs text-muted-foreground">{{ $t("catalog.scan.hint") }}</p>
      </div>

      <div class="space-y-2">
        <Label>{{ $t("catalog.photos") }}</Label>
        <MediaGallery v-model="images" v-model:busy="uploading" />
      </div>

      <div class="space-y-2">
        <Label for="p-name">{{ $t("catalog.productName") }}</Label>
        <Input id="p-name" v-model="form.name" required dir="auto" />
      </div>
      <div class="space-y-2">
        <Label>{{ $t("inventory.table.category") }}</Label>
        <Select v-model="form.categoryId" :key="$i18n.locale">
          <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="none">{{ $t("catalog.noCategory") }}</SelectItem>
            <SelectItem v-for="c in categoryOptions" :key="c.id" :value="String(c.id)">
              {{ c.label }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div class="space-y-2">
        <Label>{{ $t("catalog.icon") }}</Label>
        <IconPicker
          v-model="form.iconKey"
          clearable
          :suggest-from="suggestText"
          :boost-from="selectedCategory?.icon_key"
        />
        <p class="text-xs text-muted-foreground">{{ $t("catalog.iconHint") }}</p>
      </div>

      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label for="p-cost">{{ $t("catalog.costPrice") }}</Label>
          <Input id="p-cost" v-model="form.costPrice" type="number" min="0" step="any" dir="ltr" />
        </div>
        <div class="space-y-2">
          <Label for="p-price">{{ $t("inventory.table.price") }}</Label>
          <Input id="p-price" v-model="form.sellingPrice" type="number" min="0" step="any" required dir="ltr" />
        </div>
      </div>
      <p v-if="margin" class="-mt-2 text-sm" :class="margin.amount < 0 ? 'text-destructive' : 'text-muted-foreground'">
        {{ $t("catalog.margin", { amount: formatCurrency(margin.amount), percent: margin.percent }) }}
      </p>

      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label for="p-sku">{{ $t("inventory.table.sku") }}</Label>
          <Input id="p-sku" v-model="form.sku" dir="ltr" :placeholder="$t('catalog.skuAuto')" />
        </div>
        <div class="space-y-2">
          <Label>{{ $t("catalog.unit") }}</Label>
          <Select v-model="form.unit" :key="$i18n.locale">
            <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem v-for="u in UNITS" :key="u" :value="u">{{ $t(`catalog.units.${u}`) }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <div v-if="isWeighed(form.unit)" class="space-y-2">
        <Label for="p-plu">{{ $t("catalog.plu") }}</Label>
        <Input id="p-plu" v-model="form.plu" dir="ltr" inputmode="numeric" maxlength="6" class="w-40" />
        <p class="text-xs text-muted-foreground">{{ $t("catalog.pluHint") }}</p>
      </div>

      <!-- Pack sizes: the same product sold as a 6-pack or a carton -->
      <fieldset class="space-y-3">
        <legend class="mb-1 flex items-center gap-2 text-sm font-medium"><Package :size="16" />{{ $t("catalog.packs.title") }}</legend>
        <p class="text-xs text-muted-foreground">{{ $t("catalog.packs.hint", { unit: $t(`catalog.units.${form.unit}`).toLocaleLowerCase() }) }}</p>
        <div v-for="(u, i) in form.units" :key="u.id ?? `new${i}`" class="space-y-2 rounded-lg border p-3">
          <div class="flex gap-2">
            <Input v-model="u.name" dir="auto" required :placeholder="$t('catalog.packs.name')" :aria-label="$t('catalog.packs.name')" />
            <Button type="button" variant="ghost" size="icon" :aria-label="$t('common.remove')" @click="form.units.splice(i, 1)"><X /></Button>
          </div>
          <div class="grid grid-cols-3 gap-2">
            <div class="space-y-1">
              <Label class="text-xs">{{ $t("catalog.packs.factor", { unit: $t(`catalog.units.${form.unit}`) }) }}</Label>
              <Input v-model="u.factor" type="number" min="2" step="any" required dir="ltr" />
            </div>
            <div class="space-y-1">
              <Label class="text-xs">{{ $t("catalog.packs.price") }}</Label>
              <Input v-model="u.price" type="number" min="0" step="any" required dir="ltr" />
            </div>
            <div class="space-y-1">
              <Label class="text-xs">{{ $t("inventory.table.barcode") }}</Label>
              <Input v-model="u.barcode" dir="ltr" inputmode="numeric" />
            </div>
          </div>
          <p v-if="perItem(u) !== null" class="text-xs text-muted-foreground">
            {{ $t("catalog.packs.perItem", { price: formatCurrency(perItem(u)!) }) }}
          </p>
        </div>
        <Button type="button" variant="ghost" size="sm" @click="addPack">
          <Plus /> {{ $t("catalog.packs.add") }}
        </Button>
      </fieldset>

      <fieldset class="space-y-2">
        <legend class="mb-2 text-sm font-medium">{{ $t("catalog.extraBarcodes") }}</legend>
        <div v-for="(_, i) in form.extraBarcodes" :key="i" class="flex gap-2">
          <Input v-model="form.extraBarcodes[i]" dir="ltr" inputmode="numeric" :aria-label="$t('catalog.extraBarcode')" />
          <Button
            type="button"
            variant="ghost"
            size="icon"
            :aria-label="$t('common.remove')"
            @click="form.extraBarcodes.splice(i, 1)"
          >
            <X />
          </Button>
        </div>
        <Button type="button" variant="ghost" size="sm" @click="form.extraBarcodes.push('')">
          <Plus /> {{ $t("catalog.addBarcode") }}
        </Button>
      </fieldset>

      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label for="p-min">{{ $t("catalog.minStock") }}</Label>
          <Input id="p-min" v-model="form.minStock" type="number" min="0" step="any" dir="ltr" />
          <p class="text-xs text-muted-foreground">{{ $t("catalog.minStockHint") }}</p>
        </div>
        <div v-if="productId === null" class="space-y-2">
          <Label for="p-stock">{{ $t("catalog.initialStock") }}</Label>
          <Input id="p-stock" v-model="form.initialStock" type="number" min="0" step="any" dir="ltr" />
        </div>
        <div v-if="productId === null" class="space-y-2">
          <Label for="p-expiry">
            {{ $t("stock.expiry") }}
            <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
          </Label>
          <DateInput id="p-expiry" v-model="form.initialExpiry" clearable :placeholder="$t('stock.noExpiry')" />
        </div>
        <div v-else class="space-y-2">
          <Label>{{ $t("catalog.currentStock") }}</Label>
          <p class="num flex h-10 items-center text-lg font-semibold">{{ form.stock }}</p>
          <p class="text-xs text-muted-foreground">{{ $t("catalog.stockHint") }}</p>
        </div>
      </div>

      <label class="flex items-center gap-3 text-sm">
        <input v-model="form.isActive" type="checkbox" class="size-4 accent-primary" />
        <span>
          <span class="font-medium">{{ $t("catalog.forSale") }}</span>
          <span class="block text-muted-foreground">{{ $t("catalog.forSaleHint") }}</span>
        </span>
      </label>
      <label class="flex items-center gap-3 text-sm">
        <input v-model="form.openPrice" type="checkbox" class="size-4 accent-primary" />
        <span>
          <span class="font-medium">{{ $t("catalog.openPrice") }}</span>
          <span class="block text-muted-foreground">{{ $t("catalog.openPriceHint") }}</span>
        </span>
      </label>
      <label class="flex items-center gap-3 text-sm">
        <input v-model="form.trackStock" type="checkbox" class="size-4 accent-primary" />
        <span>
          <span class="font-medium">{{ $t("catalog.trackStock") }}</span>
          <span class="block text-muted-foreground">{{ $t("catalog.trackStockHint") }}</span>
        </span>
      </label>
    </template>

    <template #footer>
      <Button type="submit" :disabled="saving || loading || uploading">
        <Loader2 v-if="saving || uploading" class="animate-spin" />
        {{ uploading ? $t("media.uploading") : productId === null ? $t("inventory.addProduct") : $t("common.save") }}
      </Button>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
    </template>
  </EditSheet>
</template>
