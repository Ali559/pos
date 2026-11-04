<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Check, Loader2, PackagePlus, ScanBarcode, Sparkles } from "lucide-vue-next";
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
import DateInput from "@/components/pickers/DateInput.vue";
import MediaImage from "@/components/media/MediaImage.vue";
import ProductThumb from "@/components/catalog/ProductThumb.vue";
import { findProductByCode, type Category, type Product } from "@/db/catalog";
import { guessCategory, lookupBarcode } from "@/lib/barcodeLookup";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import type { MediaFile } from "@/lib/media";
import { useAuth } from "@/stores/auth";

// Scan → fill the few fields that matter → Enter → next scan. For
// entering a delivery or a whole shelf without opening the full editor.
const props = defineProps<{ categories: Category[] }>();
const emit = defineEmits<{ changed: [] }>();
const { t } = useI18n();
const auth = useAuth();
const canAdjust = computed(() => auth.can("inventory.adjust_stock"));

const scan = ref("");
const scanInput = ref<HTMLInputElement | null>(null);
const busy = ref(false);
const error = ref("");

// What's being entered right now
type Current =
  | { kind: "existing"; product: Product }
  | { kind: "new"; barcode: string; image: MediaFile | null; note: string };
const current = ref<Current | null>(null);
const form = ref({ name: "", categoryId: "none", price: "", cost: "", quantity: "1", expiry: "" });

// This session's work, newest first
const log = ref<{ id: number; name: string; quantity: number; isNew: boolean }[]>([]);

const focusScan = () => nextTick(() => scanInput.value?.focus());
onMounted(focusScan);

const reset = () => {
  current.value = null;
  error.value = "";
  scan.value = "";
  form.value = { name: "", categoryId: "none", price: "", cost: "", quantity: "1", expiry: "" };
  focusScan();
};

const onScan = async () => {
  const code = scan.value.trim();
  if (!code || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    const existing = await findProductByCode(code);
    if (existing) {
      current.value = { kind: "existing", product: existing };
      form.value.quantity = "1";
      nextTick(() => document.getElementById("qa-qty")?.focus());
      return;
    }
    const r = await lookupBarcode(code);
    const guess = r.source === "openfoodfacts" ? guessCategory(r.categoryTags, props.categories) : null;
    form.value = {
      name: r.name ?? "",
      categoryId: guess ? String(guess.id) : "none",
      price: "",
      cost: "",
      quantity: "1",
      expiry: "",
    };
    current.value = {
      kind: "new",
      barcode: code,
      image: r.image,
      note: r.source === "openfoodfacts" ? t("catalog.scan.filled") : r.offline ? t("catalog.scan.offline") : t("catalog.scan.unknown"),
    };
    nextTick(() => document.getElementById(form.value.name ? "qa-price" : "qa-name")?.focus());
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};

const save = async () => {
  const c = current.value;
  if (!c || busy.value) return;
  const qty = Number(form.value.quantity) || 0;
  busy.value = true;
  error.value = "";
  try {
    if (c.kind === "existing") {
      if (qty > 0 && canAdjust.value) {
        await call("receive_stock", {
          productId: c.product.id,
          quantity: qty,
          expiryDate: form.value.expiry || null,
          costPrice: null,
          note: null,
        });
      }
      log.value.unshift({ id: c.product.id, name: c.product.name, quantity: qty, isNew: false });
    } else {
      const id = await call<number>("create_product", {
        name: form.value.name,
        sku: null,
        categoryId: form.value.categoryId === "none" ? null : Number(form.value.categoryId),
        barcode: c.barcode,
        extraBarcodes: [],
        costPrice: Number(form.value.cost) || 0,
        sellingPrice: Number(form.value.price) || 0,
        minStock: 0,
        unit: "piece",
        iconKey: null,
        mediaIds: c.image ? [c.image.id] : [],
        isActive: true,
        initialStock: qty,
        initialExpiry: form.value.expiry || null,
      });
      log.value.unshift({ id, name: form.value.name, quantity: qty, isNew: true });
    }
    emit("changed");
    reset();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};

const categoryOptions = computed(() =>
  props.categories.filter((c) => c.is_active).map((c) => ({ id: c.id, label: (c.parent_id ? "— " : "") + c.name })),
);
const added = computed(() => log.value.filter((l) => l.isNew).length);
const received = computed(() => log.value.filter((l) => !l.isNew).length);
</script>

<template>
  <div class="grid gap-5 lg:grid-cols-[3fr_2fr]">
    <section class="space-y-4">
      <form @submit.prevent="onScan">
        <label for="qa-scan" class="sr-only">{{ $t("pos.scanBarcode") }}</label>
        <div class="flex h-16 items-center gap-3 rounded-xl border-2 border-input bg-card px-5 focus-within:border-primary">
          <ScanBarcode :size="26" :stroke-width="1.75" class="shrink-0 text-primary" />
          <input
            id="qa-scan"
            ref="scanInput"
            v-model="scan"
            dir="ltr"
            inputmode="numeric"
            autocomplete="off"
            :placeholder="$t('quickAdd.scanPlaceholder')"
            class="num h-full min-w-0 flex-1 bg-transparent text-xl outline-none placeholder:text-base placeholder:text-muted-foreground"
          />
          <Loader2 v-if="busy" class="animate-spin text-muted-foreground" />
        </div>
      </form>
      <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>

      <p v-if="!current" class="rounded-xl border border-dashed p-10 text-center text-muted-foreground">
        {{ $t("quickAdd.idle") }}
      </p>

      <!-- Already in the catalog: how many arrived -->
      <form v-else-if="current.kind === 'existing'" class="space-y-4 rounded-xl border bg-card p-5" @submit.prevent="save">
        <div class="flex items-center gap-3">
          <ProductThumb :product="current.product" :size="52" />
          <div class="min-w-0 flex-1">
            <p class="text-xs font-medium text-success">{{ $t("quickAdd.known") }}</p>
            <p class="truncate text-lg font-semibold"><bdi>{{ current.product.name }}</bdi></p>
            <p class="text-sm text-muted-foreground">
              {{ formatCurrency(current.product.price) }} — {{ $t("stock.onHand", { n: current.product.stock }) }}
            </p>
          </div>
        </div>
        <div v-if="canAdjust" class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-2">
            <Label for="qa-qty">{{ $t("quickAdd.arrived") }}</Label>
            <Input id="qa-qty" v-model="form.quantity" type="number" min="0" step="any" dir="ltr" class="h-12 text-lg" />
          </div>
          <div class="space-y-2">
            <Label>{{ $t("stock.expiry") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
            <DateInput v-model="form.expiry" clearable :placeholder="$t('stock.noExpiry')" />
          </div>
        </div>
        <p v-else class="text-sm text-muted-foreground">{{ $t("quickAdd.noAdjust") }}</p>
        <div class="flex gap-2">
          <Button type="submit" size="lg" :disabled="busy"><PackagePlus /> {{ $t("quickAdd.saveNext") }}</Button>
          <Button type="button" size="lg" variant="ghost" @click="reset">{{ $t("quickAdd.skip") }}</Button>
        </div>
      </form>

      <!-- New: the few fields that matter, the rest can wait for the full editor -->
      <form v-else class="space-y-4 rounded-xl border bg-card p-5" @submit.prevent="save">
        <div class="flex items-start gap-3">
          <div class="grid size-14 shrink-0 place-items-center overflow-hidden rounded-lg border bg-muted">
            <MediaImage v-if="current.image" :media="current.image" class="size-full" />
            <Sparkles v-else :size="20" class="text-muted-foreground" />
          </div>
          <div class="min-w-0 flex-1">
            <p class="text-xs font-medium text-primary">{{ $t("quickAdd.new") }} — <span class="num" dir="ltr">{{ current.barcode }}</span></p>
            <p class="text-sm text-muted-foreground">{{ current.note }}</p>
          </div>
        </div>
        <div class="space-y-2">
          <Label for="qa-name">{{ $t("catalog.productName") }}</Label>
          <Input id="qa-name" v-model="form.name" required dir="auto" />
        </div>
        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-2">
            <Label for="qa-price">{{ $t("inventory.table.price") }}</Label>
            <Input id="qa-price" v-model="form.price" type="number" min="0" step="any" required dir="ltr" class="h-12 text-lg" />
          </div>
          <div class="space-y-2">
            <Label>{{ $t("inventory.table.category") }}</Label>
            <Select v-model="form.categoryId" :key="$i18n.locale">
              <SelectTrigger class="h-12 w-full"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="none">{{ $t("catalog.noCategory") }}</SelectItem>
                <SelectItem v-for="c in categoryOptions" :key="c.id" :value="String(c.id)">{{ c.label }}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="space-y-2">
            <Label for="qa-cost">{{ $t("catalog.costPrice") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
            <Input id="qa-cost" v-model="form.cost" type="number" min="0" step="any" dir="ltr" />
          </div>
          <div class="space-y-2">
            <Label for="qa-qty2">{{ $t("catalog.initialStock") }}</Label>
            <Input id="qa-qty2" v-model="form.quantity" type="number" min="0" step="any" dir="ltr" />
          </div>
          <div class="space-y-2 sm:col-span-2">
            <Label>{{ $t("stock.expiry") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
            <DateInput v-model="form.expiry" clearable :placeholder="$t('stock.noExpiry')" />
          </div>
        </div>
        <div class="flex gap-2">
          <Button type="submit" size="lg" :disabled="busy"><Check /> {{ $t("quickAdd.saveNext") }}</Button>
          <Button type="button" size="lg" variant="ghost" @click="reset">{{ $t("quickAdd.skip") }}</Button>
        </div>
      </form>
    </section>

    <aside class="rounded-xl border bg-card">
      <div class="border-b px-5 py-4">
        <h2 class="font-semibold">{{ $t("quickAdd.session") }}</h2>
        <p class="text-sm text-muted-foreground">{{ $t("quickAdd.sessionCounts", { added, received }) }}</p>
      </div>
      <p v-if="!log.length" class="px-5 py-8 text-center text-sm text-muted-foreground">{{ $t("quickAdd.sessionEmpty") }}</p>
      <ul class="max-h-[60vh] divide-y overflow-y-auto">
        <li v-for="(l, i) in log" :key="i" class="flex items-center gap-3 px-5 py-2.5 text-sm">
          <span
            class="rounded px-1.5 py-0.5 text-[11px] font-semibold"
            :class="l.isNew ? 'bg-primary/10 text-primary' : 'bg-success/12 text-success'"
          >
            {{ l.isNew ? $t("quickAdd.tagNew") : $t("quickAdd.tagStock") }}
          </span>
          <bdi class="min-w-0 flex-1 truncate">{{ l.name }}</bdi>
          <span class="num text-muted-foreground">+{{ l.quantity }}</span>
        </li>
      </ul>
    </aside>
  </div>
</template>
