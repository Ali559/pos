<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Loader2, Plus, X } from "lucide-vue-next";
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
import { getProduct, type Category } from "@/db/catalog";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import type { MediaFile } from "@/lib/media";
import { localized } from "@/utils/i18n";

const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{
  /** null creates a new product */
  productId: number | null;
  categories: Category[];
}>();
const emit = defineEmits<{ saved: [] }>();

const UNITS = ["piece", "kg", "liter", "pack", "box"] as const;

const blank = () => ({
  name: "", name_ku: "", name_ar: "", sku: "", categoryId: "none",
  barcode: "", extraBarcodes: [] as string[], costPrice: "", sellingPrice: "",
  minStock: "0", unit: "piece" as string, iconKey: null as string | null,
  isActive: true, initialStock: "0", initialExpiry: "", stock: 0,
});

const form = ref(blank());
const images = ref<MediaFile[]>([]);
const saving = ref(false);
const uploading = ref(false);
const loading = ref(false);
const error = ref("");

watch(open, async (isOpen) => {
  if (!isOpen) return;
  error.value = "";
  form.value = blank();
  images.value = [];
  if (props.productId === null) return;
  loading.value = true;
  const p = await getProduct(props.productId);
  loading.value = false;
  if (!p) return;
  form.value = {
    name: p.name,
    name_ku: p.name_ku ?? "",
    name_ar: p.name_ar ?? "",
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
  };
  images.value = p.media;
});

// Subcategories listed under their parent
const categoryOptions = computed(() =>
  props.categories.filter((c) => c.is_active).map((c) => ({
    id: c.id,
    label: (c.parent_id ? "— " : "") + localized(c, "name"),
  })),
);
const selectedCategory = computed(() =>
  props.categories.find((c) => c.id === Number(form.value.categoryId)),
);
const suggestText = computed(() => [form.value.name, form.value.name_ku, form.value.name_ar].join(" "));

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
    nameKu: f.name_ku,
    nameAr: f.name_ar,
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
      <div class="space-y-2">
        <Label>{{ $t("catalog.photos") }}</Label>
        <MediaGallery v-model="images" v-model:busy="uploading" />
      </div>

      <div class="space-y-2">
        <Label for="p-name">{{ $t("catalog.productName") }}</Label>
        <Input id="p-name" v-model="form.name" required dir="auto" />
      </div>
      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label for="p-ku">
            {{ $t("catalog.nameKu") }}
            <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
          </Label>
          <Input id="p-ku" v-model="form.name_ku" lang="ku" dir="rtl" />
        </div>
        <div class="space-y-2">
          <Label for="p-ar">
            {{ $t("catalog.nameAr") }}
            <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
          </Label>
          <Input id="p-ar" v-model="form.name_ar" lang="ar" dir="rtl" />
        </div>
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

      <fieldset class="space-y-2">
        <legend class="mb-2 text-sm font-medium">{{ $t("inventory.table.barcode") }}</legend>
        <Input v-model="form.barcode" dir="ltr" inputmode="numeric" :aria-label="$t('inventory.table.barcode')" />
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
          <Input id="p-expiry" v-model="form.initialExpiry" type="date" dir="ltr" />
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
