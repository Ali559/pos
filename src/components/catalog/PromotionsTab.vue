<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Loader2, Pencil, Plus, Tag, Trash2, X } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import EditSheet from "@/components/EditSheet.vue";
import DateInput from "@/components/pickers/DateInput.vue";
import { listProducts, type Category, type Product } from "@/db/catalog";
import { isRunning, listPromotions, todayLocal, type PromotionRow } from "@/db/promotions";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import { formatDate } from "@/lib/locale";

// Promotions: percent off, buy X get Y free, or N for a price, on chosen
// products or whole categories, between two dates. Checkout applies the
// best one per line automatically (retail sales only).
const props = defineProps<{ categories: Category[] }>();
const { t } = useI18n();

const promos = ref<PromotionRow[]>([]);
const products = ref<Product[]>([]);
const load = async () => {
  [promos.value, products.value] = await Promise.all([listPromotions(), listProducts()]);
};
onMounted(load);

const today = todayLocal();
const status = (p: PromotionRow) =>
  !p.is_active ? "off" : p.ends_on && p.ends_on < today ? "ended" : p.starts_on && p.starts_on > today ? "upcoming" : "running";

const describe = (p: PromotionRow) =>
  p.kind === "percent"
    ? t("promotions.describe.percent", { n: p.percent })
    : p.kind === "multibuy"
      ? t("promotions.describe.multibuy", { buy: p.buy_qty, free: p.free_qty })
      : t("promotions.describe.bundle", { n: p.buy_qty, price: formatCurrency(p.bundle_price ?? 0) });

const targets = (p: PromotionRow) =>
  [
    ...p.category_ids.map((id) => props.categories.find((c) => c.id === id)?.name).filter(Boolean),
    ...p.product_ids.map((id) => products.value.find((x) => x.id === id)?.name).filter(Boolean),
  ].join("، ");

// ----------------------------------------------------------- editor
const open = ref(false);
const editingId = ref<number | null>(null);
const form = ref({
  name: "", kind: "percent" as PromotionRow["kind"], percent: "", buyQty: "2", freeQty: "1", bundlePrice: "",
  startsOn: "", endsOn: "", isActive: true, productIds: [] as number[], categoryIds: [] as number[],
});
const saving = ref(false);
const error = ref("");
const find = ref("");
const found = computed(() => {
  const q = find.value.trim().toLowerCase();
  return q ? products.value.filter((p) => !p.openPrice && p.name.toLowerCase().includes(q) && !form.value.productIds.includes(p.id)).slice(0, 8) : [];
});
watch(open, (v) => !v && (find.value = ""));

const start = (p: PromotionRow | null) => {
  editingId.value = p?.id ?? null;
  error.value = "";
  form.value = p
    ? {
        name: p.name, kind: p.kind, percent: String(p.percent ?? ""), buyQty: String(p.buy_qty ?? "2"), freeQty: String(p.free_qty ?? "1"),
        bundlePrice: String(p.bundle_price ?? ""), startsOn: p.starts_on ?? "", endsOn: p.ends_on ?? "", isActive: !!p.is_active,
        productIds: [...p.product_ids], categoryIds: [...p.category_ids],
      }
    : { name: "", kind: "percent", percent: "", buyQty: "2", freeQty: "1", bundlePrice: "", startsOn: "", endsOn: "", isActive: true, productIds: [], categoryIds: [] };
  open.value = true;
};

const save = async () => {
  const f = form.value;
  saving.value = true;
  error.value = "";
  try {
    await call("save_promotion", {
      id: editingId.value,
      promotion: {
        name: f.name,
        kind: f.kind,
        percent: f.percent === "" ? null : Number(f.percent),
        buyQty: Number(f.buyQty) || null,
        freeQty: Number(f.freeQty) || null,
        bundlePrice: f.bundlePrice === "" ? null : Number(f.bundlePrice),
        startsOn: f.startsOn || null,
        endsOn: f.endsOn || null,
        productIds: f.productIds,
        categoryIds: f.categoryIds,
        isActive: f.isActive,
      },
    });
    open.value = false;
    await load();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};

const remove = async (p: PromotionRow) => {
  await call("delete_promotion", { id: p.id });
  await load();
};

const toggleCategory = (id: number) => {
  const list = form.value.categoryIds;
  const i = list.indexOf(id);
  if (i >= 0) list.splice(i, 1);
  else list.push(id);
};
const productName = (id: number) => products.value.find((p) => p.id === id)?.name ?? `#${id}`;
</script>

<template>
  <div class="space-y-5">
    <div class="flex flex-wrap items-center gap-3">
      <p class="flex-1 text-sm text-muted-foreground">{{ $t("promotions.hint") }}</p>
      <Button @click="start(null)"><Plus />{{ $t("promotions.add") }}</Button>
    </div>

    <p v-if="!promos.length" class="rounded-xl border border-dashed p-12 text-center text-muted-foreground">{{ $t("promotions.none") }}</p>
    <ul v-else class="divide-y rounded-xl border bg-card">
      <li v-for="p in promos" :key="p.id" class="flex items-center gap-4 px-5 py-3" :class="isRunning(p) ? '' : 'opacity-70'">
        <span class="grid size-9 shrink-0 place-items-center rounded-full bg-primary/10 text-primary"><Tag :size="16" /></span>
        <div class="min-w-0 flex-1">
          <p class="font-medium"><bdi>{{ p.name }}</bdi> <span class="text-sm font-normal text-muted-foreground">· {{ describe(p) }}</span></p>
          <p class="truncate text-xs text-muted-foreground">
            <bdi>{{ targets(p) }}</bdi>
            <template v-if="p.starts_on || p.ends_on"> · {{ p.starts_on ? formatDate(new Date(`${p.starts_on}T00:00`)) : "…" }} – {{ p.ends_on ? formatDate(new Date(`${p.ends_on}T00:00`)) : "…" }}</template>
            <template v-if="p.uses"> · {{ $t("promotions.uses", { n: p.uses }) }}</template>
          </p>
        </div>
        <span class="rounded-md px-2 py-0.5 text-xs font-semibold" :class="status(p) === 'running' ? 'bg-success/12 text-success' : 'bg-muted text-muted-foreground'">{{ $t(`promotions.status.${status(p)}`) }}</span>
        <Button variant="ghost" size="icon-sm" :aria-label="$t('settings.edit')" @click="start(p)"><Pencil /></Button>
        <Button variant="ghost" size="icon-sm" :aria-label="$t('settings.delete')" :title="p.uses ? $t('promotions.archiveHint') : ''" @click="remove(p)"><Trash2 /></Button>
      </li>
    </ul>

    <EditSheet v-model:open="open" :title="editingId ? $t('promotions.edit') : $t('promotions.add')" @submit="save">
      <div class="space-y-2">
        <Label for="pr-name">{{ $t("promotions.name") }}</Label>
        <Input id="pr-name" v-model="form.name" required dir="auto" :placeholder="$t('promotions.namePlaceholder')" />
      </div>
      <div class="grid grid-cols-3 gap-1 rounded-lg bg-muted p-1" role="radiogroup">
        <button v-for="k in ['percent', 'multibuy', 'bundle'] as const" :key="k" type="button" role="radio" :aria-checked="form.kind === k"
          class="h-9 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm" @click="form.kind = k">{{ $t(`promotions.kinds.${k}`) }}</button>
      </div>
      <div v-if="form.kind === 'percent'" class="flex items-center gap-2 text-sm">
        <Input v-model="form.percent" type="number" min="1" max="100" step="any" dir="ltr" class="w-24" required :aria-label="$t('promotions.kinds.percent')" />
        <span>{{ $t("promotions.percentOff") }}</span>
      </div>
      <div v-else-if="form.kind === 'multibuy'" class="flex flex-wrap items-center gap-2 text-sm">
        <span>{{ $t("promotions.buy") }}</span>
        <Input v-model="form.buyQty" type="number" min="1" step="1" dir="ltr" class="w-20" required />
        <span>{{ $t("promotions.getFree") }}</span>
        <Input v-model="form.freeQty" type="number" min="1" step="1" dir="ltr" class="w-20" required />
        <span>{{ $t("promotions.free") }}</span>
      </div>
      <div v-else class="flex flex-wrap items-center gap-2 text-sm">
        <Input v-model="form.buyQty" type="number" min="2" step="1" dir="ltr" class="w-20" required />
        <span>{{ $t("promotions.for") }}</span>
        <Input v-model="form.bundlePrice" type="number" min="0" step="any" dir="ltr" class="w-32" required />
      </div>

      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label>{{ $t("promotions.startsOn") }}</Label>
          <DateInput v-model="form.startsOn" clearable :placeholder="$t('promotions.now')" />
        </div>
        <div class="space-y-2">
          <Label>{{ $t("promotions.endsOn") }}</Label>
          <DateInput v-model="form.endsOn" clearable :placeholder="$t('promotions.noEnd')" />
        </div>
      </div>

      <fieldset class="space-y-2">
        <legend class="text-sm font-medium">{{ $t("promotions.categories") }}</legend>
        <div class="flex flex-wrap gap-2">
          <button v-for="c in categories.filter((c) => c.is_active)" :key="c.id" type="button" :aria-pressed="form.categoryIds.includes(c.id)"
            class="h-8 rounded-full border px-3 text-sm aria-pressed:border-foreground aria-pressed:bg-foreground aria-pressed:text-background" @click="toggleCategory(c.id)">
            {{ c.parent_id ? "— " : "" }}{{ c.name }}
          </button>
        </div>
      </fieldset>
      <fieldset class="relative space-y-2">
        <legend class="text-sm font-medium">{{ $t("promotions.products") }}</legend>
        <div v-if="form.productIds.length" class="flex flex-wrap gap-2">
          <span v-for="id in form.productIds" :key="id" class="flex items-center gap-1 rounded-full bg-muted py-1 ps-3 pe-1 text-sm">
            <bdi>{{ productName(id) }}</bdi>
            <button type="button" class="rounded-full p-0.5 hover:bg-card" :aria-label="$t('common.remove')" @click="form.productIds.splice(form.productIds.indexOf(id), 1)"><X :size="12" /></button>
          </span>
        </div>
        <Input v-model="find" dir="auto" :placeholder="$t('promotions.findProduct')" :aria-label="$t('promotions.findProduct')" />
        <ul v-if="found.length" class="absolute inset-x-0 z-10 max-h-56 overflow-y-auto rounded-lg border bg-card shadow-lg">
          <li v-for="p in found" :key="p.id">
            <button type="button" class="w-full px-3 py-2 text-start text-sm hover:bg-muted" @click="form.productIds.push(p.id); find = ''"><bdi>{{ p.name }}</bdi></button>
          </li>
        </ul>
      </fieldset>
      <label class="flex items-center gap-3 text-sm">
        <input v-model="form.isActive" type="checkbox" class="size-4 accent-primary" />
        <span class="font-medium">{{ $t("promotions.active") }}</span>
      </label>
      <p class="text-xs text-muted-foreground">{{ $t("promotions.rules") }}</p>

      <template #footer>
        <Button type="submit" :disabled="saving"><Loader2 v-if="saving" class="animate-spin" />{{ $t("common.save") }}</Button>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      </template>
    </EditSheet>
  </div>
</template>
