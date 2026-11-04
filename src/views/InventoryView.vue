<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { watchDebounced } from "@vueuse/core";
import { ChevronLeft, ChevronRight, PackagePlus, Pencil, Plus, Search } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import CategoriesTab from "@/components/catalog/CategoriesTab.vue";
import ProductSheet from "@/components/catalog/ProductSheet.vue";
import ProductThumb from "@/components/catalog/ProductThumb.vue";
import StockSheet from "@/components/catalog/StockSheet.vue";
import { useRoute } from "vue-router";
import {
  countProducts,
  listCategories,
  listProducts,
  stockState,
  type Category,
  type Product,
} from "@/db/catalog";
import { formatCurrency } from "@/lib/format";
import { useAuth } from "@/stores/auth";
import { localized } from "@/utils/i18n";

const PAGE_SIZE = 10;
const auth = useAuth();
const canEdit = computed(() => auth.can("inventory.edit"));
const canAdjust = computed(() => auth.can("inventory.adjust_stock"));
const route = useRoute();

const tab = ref("products");
// Alerts link here with ?q=<product name>
const search = ref(typeof route.query.q === "string" ? route.query.q : "");
const category = ref("all");
const showInactive = ref(false);
const page = ref(0);
const categories = ref<Category[]>([]);
const rows = ref<Product[]>([]);
const total = ref(0);
const catalogSize = ref<number | null>(null);

const query = computed(() => ({
  search: search.value,
  categoryId: category.value === "all" ? null : Number(category.value),
  includeInactive: showInactive.value,
}));

const load = async () => {
  const [list, count] = await Promise.all([
    listProducts({ ...query.value, limit: PAGE_SIZE, offset: page.value * PAGE_SIZE }),
    countProducts(query.value),
  ]);
  rows.value = list;
  total.value = count;
};

const loadCategories = async () => {
  categories.value = await listCategories();
};

watchDebounced([search, category, showInactive], () => (page.value === 0 ? load() : (page.value = 0)), {
  debounce: 150,
});
watch(page, load);

onMounted(async () => {
  await Promise.all([loadCategories(), load()]);
  catalogSize.value = await countProducts({ includeInactive: true });
});

const pageCount = computed(() => Math.max(1, Math.ceil(total.value / PAGE_SIZE)));
const rangeLabel = computed(() => {
  if (!total.value) return "0";
  const start = page.value * PAGE_SIZE + 1;
  return `${start}–${start + rows.value.length - 1}`;
});

// Fill of the stock bar: full at 4× the reorder level
const stockFill = (stock: number, min: number) =>
  `${Math.min(100, (stock / Math.max(1, min * 4)) * 100)}%`;

const statusClass = {
  ok: "bg-success/12 text-success",
  low: "bg-warning/15 text-warning",
  out: "bg-destructive/12 text-destructive",
};

// Product editor
const sheetOpen = ref(false);
const editingId = ref<number | null>(null);
const openEditor = (id: number | null) => {
  editingId.value = id;
  sheetOpen.value = true;
};
const onSaved = async () => {
  await load();
  catalogSize.value = await countProducts({ includeInactive: true });
};
// Stock sheet
const stockOpen = ref(false);
const stockProduct = ref<Product | null>(null);
const openStock = (p: Product) => {
  stockProduct.value = p;
  stockOpen.value = true;
};

const onCategoriesChanged = () => {
  loadCategories();
  load();
};
</script>

<template>
  <div class="mx-auto max-w-7xl p-6">
    <Tabs v-model="tab">
      <TabsList class="mb-6">
        <TabsTrigger value="products">{{ $t("catalog.tabs.products") }}</TabsTrigger>
        <TabsTrigger value="categories">{{ $t("catalog.tabs.categories") }}</TabsTrigger>
      </TabsList>

      <TabsContent value="products">
        <div class="mb-5 flex flex-wrap items-center gap-3">
          <div class="relative min-w-64 flex-1">
            <Search
              :size="18"
              class="pointer-events-none absolute start-3 top-1/2 -translate-y-1/2 text-muted-foreground"
            />
            <input
              v-model="search"
              type="search"
              :placeholder="$t('inventory.searchPlaceholder')"
              :aria-label="$t('inventory.searchPlaceholder')"
              class="h-10 w-full rounded-md border border-input bg-card ps-10 pe-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            />
          </div>
          <Select v-model="category" :key="$i18n.locale">
            <SelectTrigger class="w-52"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="all">{{ $t("inventory.allCategories") }}</SelectItem>
              <SelectItem v-for="c in categories" :key="c.id" :value="String(c.id)">
                {{ (c.parent_id ? "— " : "") + localized(c, "name") }}
              </SelectItem>
            </SelectContent>
          </Select>
          <label class="flex items-center gap-2 text-sm text-muted-foreground">
            <input v-model="showInactive" type="checkbox" class="size-4 accent-primary" />
            {{ $t("catalog.showInactive") }}
          </label>
          <Button v-if="canEdit" size="lg" class="ms-auto" @click="openEditor(null)">
            <Plus />
            {{ $t("inventory.addProduct") }}
          </Button>
        </div>

        <div class="overflow-hidden rounded-xl border bg-card">
          <Table>
            <TableHeader>
              <TableRow class="hover:bg-transparent">
                <TableHead class="ps-5">{{ $t("inventory.table.name") }}</TableHead>
                <TableHead>{{ $t("inventory.table.sku") }}</TableHead>
                <TableHead>{{ $t("inventory.table.barcode") }}</TableHead>
                <TableHead class="text-end">{{ $t("inventory.table.price") }}</TableHead>
                <TableHead class="w-48">{{ $t("inventory.table.stock") }}</TableHead>
                <TableHead>{{ $t("inventory.table.status") }}</TableHead>
                <TableHead class="w-24 pe-5"
                  ><span class="sr-only">{{ $t("inventory.table.actions") }}</span></TableHead
                >
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="rows.length === 0" class="hover:bg-transparent">
                <TableCell colspan="7" class="py-14 text-center text-muted-foreground">
                  {{ catalogSize === 0 ? $t("inventory.emptyCatalog") : $t("inventory.noResults") }}
                </TableCell>
              </TableRow>
              <TableRow v-for="p in rows" :key="p.id" :class="!p.isActive && 'opacity-55'">
                <TableCell class="ps-5">
                  <div class="flex items-center gap-3">
                    <ProductThumb :product="p" :size="40" />
                    <div class="min-w-0">
                      <div class="truncate font-medium"><bdi>{{ localized(p, "name") }}</bdi></div>
                      <div class="text-xs text-muted-foreground">
                        {{ p.categoryName ? localized(p, "categoryName") : $t("catalog.noCategory") }}
                        <template v-if="!p.isActive"> — {{ $t("catalog.notForSale") }}</template>
                      </div>
                    </div>
                  </div>
                </TableCell>
                <TableCell class="num text-muted-foreground" dir="ltr">{{ p.sku }}</TableCell>
                <TableCell class="num text-muted-foreground" dir="ltr">{{ p.barcode ?? "—" }}</TableCell>
                <TableCell class="num text-end font-medium">{{ formatCurrency(p.price) }}</TableCell>
                <TableCell>
                  <div class="flex items-center gap-3">
                    <span class="num w-8 text-end font-medium">{{ p.stock }}</span>
                    <div class="h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
                      <div
                        class="h-full rounded-full"
                        :class="{
                          'bg-primary': stockState(p) === 'ok',
                          'bg-warning': stockState(p) === 'low',
                        }"
                        :style="{ width: stockFill(p.stock, p.minStock) }"
                      />
                    </div>
                  </div>
                </TableCell>
                <TableCell>
                  <span
                    class="inline-flex h-6 items-center rounded-md px-2 text-xs font-semibold"
                    :class="statusClass[stockState(p)]"
                  >
                    {{ $t(`inventory.status.${stockState(p)}`) }}
                  </span>
                </TableCell>
                <TableCell class="pe-5">
                  <div class="flex justify-end gap-0.5">
                    <Button
                      v-if="canAdjust"
                      variant="ghost"
                      size="icon-sm"
                      :aria-label="$t('stock.title')"
                      :title="$t('stock.title')"
                      @click="openStock(p)"
                    >
                      <PackagePlus />
                    </Button>
                    <Button
                      v-if="canEdit"
                      variant="ghost"
                      size="icon-sm"
                      :aria-label="$t('inventory.edit')"
                      @click="openEditor(p.id)"
                    >
                      <Pencil />
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>

          <div class="flex items-center justify-between border-t px-5 py-3">
            <p class="num text-sm text-muted-foreground">
              {{ $t("inventory.pagination.showing") }} {{ rangeLabel }}
              {{ $t("inventory.pagination.of") }} {{ total }}
            </p>
            <div class="flex gap-2">
              <Button variant="outline" size="sm" :disabled="page === 0" @click="page--">
                <ChevronLeft class="rtl:rotate-180" />
                {{ $t("inventory.pagination.prev") }}
              </Button>
              <Button variant="outline" size="sm" :disabled="page >= pageCount - 1" @click="page++">
                {{ $t("inventory.pagination.next") }}
                <ChevronRight class="rtl:rotate-180" />
              </Button>
            </div>
          </div>
        </div>
      </TabsContent>

      <TabsContent value="categories">
        <CategoriesTab @changed="onCategoriesChanged" />
      </TabsContent>
    </Tabs>

    <StockSheet v-model:open="stockOpen" :product="stockProduct" @saved="load" />
    <ProductSheet
      v-model:open="sheetOpen"
      :product-id="editingId"
      :categories="categories"
      @saved="onSaved"
    />
  </div>
</template>
