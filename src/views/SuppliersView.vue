<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ChevronDown, PackagePlus, Plus, Search } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import SupplierSheet from "@/components/suppliers/SupplierSheet.vue";
import SupplierAccountSheet from "@/components/suppliers/SupplierAccountSheet.vue";
import DeliverySheet from "@/components/suppliers/DeliverySheet.vue";
import ReorderTab from "@/components/suppliers/ReorderTab.vue";
import {
  getDeliveryItems,
  listDeliveries,
  listSuppliers,
  owedToSuppliers,
  type DeliveryItem,
  type DeliveryRow,
  type Supplier,
} from "@/db/suppliers";
import { formatCurrency } from "@/lib/format";
import { formatDate, formatDateTime } from "@/lib/locale";
import { useAuth } from "@/stores/auth";

const auth = useAuth();
const tab = ref("suppliers");
const canReceive = computed(() => auth.can("inventory.adjust_stock"));

// Suppliers
const suppliers = ref<Supplier[]>([]);
const owed = ref(0);
const search = ref("");
const showInactive = ref(false);
const loadSuppliers = async () => {
  [suppliers.value, owed.value] = await Promise.all([listSuppliers({ search: search.value, includeInactive: showInactive.value }), owedToSuppliers()]);
};
watch([search, showInactive], loadSuppliers);

const sheetOpen = ref(false);
const editing = ref<Supplier | null>(null);
const startCreate = () => {
  editing.value = null;
  sheetOpen.value = true;
};
const startEdit = (s: Supplier) => {
  accountOpen.value = false;
  editing.value = s;
  sheetOpen.value = true;
};
const accountOpen = ref(false);
const accountId = ref<number | null>(null);
const openAccount = (s: Supplier) => {
  accountId.value = s.id;
  accountOpen.value = true;
};

// Deliveries
const deliveries = ref<DeliveryRow[]>([]);
const expanded = ref<number | null>(null);
const items = ref<DeliveryItem[]>([]);
const loadDeliveries = async () => {
  deliveries.value = await listDeliveries();
};
const toggle = async (d: DeliveryRow) => {
  if (expanded.value === d.id) {
    expanded.value = null;
    return;
  }
  items.value = await getDeliveryItems(d.id);
  expanded.value = d.id;
};
const deliveryOpen = ref(false);
const received = async () => {
  await Promise.all([loadSuppliers(), loadDeliveries()]);
  tab.value = "deliveries";
};

onMounted(() => Promise.all([loadSuppliers(), loadDeliveries()]));
const fmt = (n: number) => (Number.isInteger(n) ? String(n) : n.toFixed(3).replace(/0+$/, ""));
</script>

<template>
  <div class="mx-auto max-w-7xl p-6">
    <Tabs v-model="tab">
      <div class="mb-6 flex flex-wrap items-center gap-3">
        <TabsList>
          <TabsTrigger value="suppliers">{{ $t("suppliers.tabs.suppliers") }}</TabsTrigger>
          <TabsTrigger value="deliveries">{{ $t("suppliers.tabs.deliveries") }}</TabsTrigger>
          <TabsTrigger value="reorder">{{ $t("suppliers.tabs.reorder") }}</TabsTrigger>
        </TabsList>
        <Button v-if="canReceive" size="lg" class="ms-auto" @click="deliveryOpen = true"><PackagePlus />{{ $t("deliveries.receive") }}</Button>
      </div>

      <TabsContent value="suppliers" class="space-y-5">
        <div class="flex flex-wrap items-center gap-3">
          <div class="rounded-xl border bg-card px-5 py-3">
            <p class="text-xs text-muted-foreground">{{ $t("suppliers.totalOwed") }}</p>
            <p class="num text-2xl font-bold">{{ formatCurrency(owed) }}</p>
          </div>
          <div class="relative min-w-64 flex-1">
            <Search :size="18" class="pointer-events-none absolute start-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
            <input v-model="search" type="search" :placeholder="$t('suppliers.searchPlaceholder')" :aria-label="$t('suppliers.searchPlaceholder')"
              class="h-10 w-full rounded-md border border-input bg-card ps-10 pe-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring" />
          </div>
          <label class="flex items-center gap-2 text-sm"><input v-model="showInactive" type="checkbox" class="size-4 accent-primary" />{{ $t("customers.showInactive") }}</label>
          <Button v-if="auth.can('suppliers.edit')" variant="outline" @click="startCreate"><Plus />{{ $t("suppliers.add") }}</Button>
        </div>
        <div class="overflow-hidden rounded-xl border bg-card">
          <Table>
            <TableHeader>
              <TableRow class="hover:bg-transparent">
                <TableHead class="ps-5">{{ $t("suppliers.fields.name") }}</TableHead>
                <TableHead>{{ $t("suppliers.fields.phone") }}</TableHead>
                <TableHead class="text-end">{{ $t("suppliers.products") }}</TableHead>
                <TableHead>{{ $t("suppliers.lastDelivery") }}</TableHead>
                <TableHead class="pe-5 text-end">{{ $t("suppliers.weOwe") }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="!suppliers.length" class="hover:bg-transparent">
                <TableCell colspan="5" class="py-12 text-center text-muted-foreground">{{ $t("suppliers.empty") }}</TableCell>
              </TableRow>
              <TableRow v-for="s in suppliers" :key="s.id" class="cursor-pointer" :class="s.is_active ? '' : 'opacity-60'" @click="openAccount(s)">
                <TableCell class="ps-5 font-medium"><bdi>{{ s.name }}</bdi><span v-if="s.contact_person" class="block text-xs font-normal text-muted-foreground"><bdi>{{ s.contact_person }}</bdi></span></TableCell>
                <TableCell class="num text-muted-foreground" dir="ltr">{{ s.phone ?? "—" }}</TableCell>
                <TableCell class="num text-end">{{ s.products }}</TableCell>
                <TableCell class="text-muted-foreground">{{ s.last_delivery ? formatDate(new Date(s.last_delivery)) : "—" }}</TableCell>
                <TableCell class="num pe-5 text-end font-semibold" :class="s.owed > 0 ? 'text-destructive' : 'text-muted-foreground'">{{ s.owed > 0 ? formatCurrency(s.owed) : "—" }}</TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </TabsContent>

      <TabsContent value="deliveries">
        <div class="overflow-hidden rounded-xl border bg-card">
          <p v-if="!deliveries.length" class="py-12 text-center text-muted-foreground">{{ $t("deliveries.none") }}</p>
          <ul v-else class="divide-y">
            <li v-for="d in deliveries" :key="d.id">
              <button type="button" class="flex w-full items-center gap-4 px-5 py-3 text-start hover:bg-muted/60" :aria-expanded="expanded === d.id" @click="toggle(d)">
                <div class="min-w-0 flex-1">
                  <p class="font-medium"><bdi>{{ d.supplier ?? $t("deliveries.noSupplier") }}</bdi><span v-if="d.invoice_number" class="num ms-2 text-sm text-muted-foreground" dir="ltr">{{ d.invoice_number }}</span></p>
                  <p class="text-xs text-muted-foreground">{{ formatDateTime(new Date(d.received_at)) }} · {{ d.user }} · {{ $t("deliveries.lines", { n: d.items }) }}</p>
                </div>
                <span class="num font-semibold">{{ formatCurrency(d.total) }}</span>
                <ChevronDown :size="16" class="transition-transform" :class="expanded === d.id ? 'rotate-180' : ''" />
              </button>
              <table v-if="expanded === d.id" class="mx-5 mb-3 w-[calc(100%-2.5rem)] text-sm">
                <tbody class="divide-y">
                  <tr v-for="it in items" :key="it.id">
                    <td class="py-1.5"><bdi>{{ it.name }}</bdi></td>
                    <td class="num py-1.5 text-end text-muted-foreground">
                      <template v-if="it.unit_name && it.unit_factor">{{ fmt(it.quantity / it.unit_factor) }} × <bdi>{{ it.unit_name }}</bdi></template>
                      <template v-else>{{ fmt(it.quantity) }}</template>
                    </td>
                    <td class="num py-1.5 text-end text-muted-foreground">{{ it.expiry_date ?? "" }}</td>
                    <td class="num py-1.5 text-end">{{ formatCurrency(it.quantity * it.cost) }}</td>
                  </tr>
                </tbody>
              </table>
            </li>
          </ul>
        </div>
      </TabsContent>

      <TabsContent value="reorder"><ReorderTab v-if="tab === 'reorder'" /></TabsContent>
    </Tabs>

    <SupplierSheet v-model:open="sheetOpen" :supplier="editing" @saved="loadSuppliers" />
    <SupplierAccountSheet v-model:open="accountOpen" :supplier-id="accountId" @changed="loadSuppliers" @edit="startEdit" />
    <DeliverySheet v-model:open="deliveryOpen" @saved="received" />
  </div>
</template>
