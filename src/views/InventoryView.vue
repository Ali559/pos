<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ChevronLeft, ChevronRight, Pencil, Plus, Search } from "lucide-vue-next";
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
import {
  countProducts,
  listCategories,
  listProducts,
  stockState,
  type Category,
  type Product,
} from "@/db/catalog";
import { formatCurrency } from "@/lib/format";
import { categoryInk } from "@/lib/categoryColor";
import { localized } from "@/utils/i18n";
import { watchDebounced } from "@vueuse/core";

const PAGE_SIZE = 10;

const search = ref("");
const category = ref("all");
const page = ref(0);
const categories = ref<Category[]>([]);
const rows = ref<Product[]>([]);
const total = ref(0);
const catalogSize = ref<number | null>(null);

const query = computed(() => ({
  search: search.value,
  categoryId: category.value === "all" ? null : Number(category.value),
}));

const load = async () => {
  const [list, count] = await Promise.all([
    listProducts({ ...query.value, limit: PAGE_SIZE, offset: page.value * PAGE_SIZE }),
    countProducts(query.value),
  ]);
  rows.value = list;
  total.value = count;
};

watchDebounced([search, category], () => (page.value === 0 ? load() : (page.value = 0)), {
  debounce: 150,
});
watch(page, load);

onMounted(async () => {
  categories.value = await listCategories();
  await load();
  catalogSize.value = await countProducts();
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
</script>

<template>
  <div class="mx-auto max-w-7xl p-6">
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
        <SelectTrigger class="w-48">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="all">{{ $t("inventory.allCategories") }}</SelectItem>
          <SelectItem v-for="c in categories" :key="c.id" :value="String(c.id)">
            {{ localized(c, "name") }}
          </SelectItem>
        </SelectContent>
      </Select>
      <Button size="lg" class="ms-auto">
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
            <TableHead class="w-14 pe-5"
              ><span class="sr-only">{{ $t("inventory.table.actions") }}</span></TableHead
            >
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-if="rows.length === 0" class="hover:bg-transparent">
            <TableCell colspan="7" class="py-14 text-center text-muted-foreground">
              {{
                catalogSize === 0
                  ? $t("inventory.emptyCatalog")
                  : $t("inventory.noResults")
              }}
            </TableCell>
          </TableRow>
          <TableRow v-for="p in rows" :key="p.id">
            <TableCell class="ps-5">
              <div class="flex items-center gap-3">
                <span
                  class="size-2.5 shrink-0 rounded-full"
                  :style="{ background: categoryInk(p.categoryColor) }"
                />
                <div>
                  <div class="font-medium"><bdi>{{ p.name }}</bdi></div>
                  <div class="text-xs text-muted-foreground">
                    {{ localized(p, "categoryName") }}
                  </div>
                </div>
              </div>
            </TableCell>
            <TableCell class="num text-muted-foreground">{{ p.sku }}</TableCell>
            <TableCell class="num text-muted-foreground">{{ p.barcode }}</TableCell>
            <TableCell class="num text-end font-medium">{{
              formatCurrency(p.price)
            }}</TableCell>
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
              <Button
                variant="ghost"
                size="icon-sm"
                :aria-label="$t('inventory.edit')"
              >
                <Pencil />
              </Button>
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
          <Button
            variant="outline"
            size="sm"
            :disabled="page === 0"
            @click="page--"
          >
            <ChevronLeft class="rtl:rotate-180" />
            {{ $t("inventory.pagination.prev") }}
          </Button>
          <Button
            variant="outline"
            size="sm"
            :disabled="page >= pageCount - 1"
            @click="page++"
          >
            {{ $t("inventory.pagination.next") }}
            <ChevronRight class="rtl:rotate-180" />
          </Button>
        </div>
      </div>
    </div>
  </div>
</template>
