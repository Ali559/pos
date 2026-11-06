<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { HandCoins, Plus, Search, TriangleAlert } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import CustomerSheet from "@/components/customers/CustomerSheet.vue";
import CustomerAccountSheet from "@/components/customers/CustomerAccountSheet.vue";
import CustomerPaymentDialog from "@/components/customers/CustomerPaymentDialog.vue";
import { debtTotals, listCustomers, type Customer } from "@/db/customers";
import { formatCurrency } from "@/lib/format";
import { formatDate } from "@/lib/locale";
import { useAuth } from "@/stores/auth";

const auth = useAuth();
const canCreate = computed(() => auth.can("customers.edit") || auth.can("customers.credit"));

const customers = ref<Customer[]>([]);
const totals = ref({ total: 0, people: 0 });
const search = ref("");
const owing = ref(false);
const showInactive = ref(false);

const load = async () => {
  [customers.value, totals.value] = await Promise.all([
    listCustomers({ search: search.value, owing: owing.value, includeInactive: showInactive.value }),
    debtTotals(),
  ]);
};
onMounted(load);
let timer: ReturnType<typeof setTimeout>;
watch(search, () => {
  clearTimeout(timer);
  timer = setTimeout(load, 200);
});
watch([owing, showInactive], load);

const sheetOpen = ref(false);
const editing = ref<Customer | null>(null);
const startCreate = () => {
  editing.value = null;
  sheetOpen.value = true;
};
const startEdit = (c: Customer) => {
  accountOpen.value = false;
  editing.value = c;
  sheetOpen.value = true;
};

const accountOpen = ref(false);
const accountId = ref<number | null>(null);
const openAccount = (c: Customer) => {
  accountId.value = c.id;
  accountOpen.value = true;
};

const payOpen = ref(false);
const paying = ref<Customer | null>(null);
const pay = (c: Customer) => {
  paying.value = c;
  payOpen.value = true;
};

const overLimit = (c: Customer) => c.credit_limit != null && c.balance > c.credit_limit + 1e-6;
</script>

<template>
  <div class="mx-auto max-w-7xl space-y-5 p-6">
    <div class="grid gap-4 sm:grid-cols-2">
      <div class="rounded-xl border bg-card p-5">
        <p class="text-sm text-muted-foreground">{{ $t("customers.totalOwed") }}</p>
        <p class="num text-3xl font-bold">{{ formatCurrency(totals.total) }}</p>
      </div>
      <button
        type="button"
        class="rounded-xl border bg-card p-5 text-start transition-colors hover:border-foreground/30"
        :aria-pressed="owing"
        @click="owing = !owing"
      >
        <p class="text-sm text-muted-foreground">{{ $t("customers.whoOwes") }}</p>
        <p class="num text-3xl font-bold">{{ totals.people }}</p>
        <p class="text-xs text-primary">{{ owing ? $t("customers.showAll") : $t("customers.showOwing") }}</p>
      </button>
    </div>

    <div class="flex flex-wrap items-center gap-3">
      <div class="relative min-w-64 flex-1">
        <Search :size="18" class="pointer-events-none absolute start-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
        <input
          v-model="search"
          type="search"
          :placeholder="$t('customers.searchPlaceholder')"
          :aria-label="$t('customers.searchPlaceholder')"
          class="h-10 w-full rounded-md border border-input bg-card ps-10 pe-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        />
      </div>
      <label class="flex items-center gap-2 text-sm">
        <input v-model="owing" type="checkbox" class="size-4 accent-primary" />{{ $t("customers.onlyOwing") }}
      </label>
      <label class="flex items-center gap-2 text-sm">
        <input v-model="showInactive" type="checkbox" class="size-4 accent-primary" />{{ $t("customers.showInactive") }}
      </label>
      <Button v-if="canCreate" size="lg" class="ms-auto" @click="startCreate"><Plus />{{ $t("customers.add") }}</Button>
    </div>

    <div class="overflow-hidden rounded-xl border bg-card">
      <Table>
        <TableHeader>
          <TableRow class="hover:bg-transparent">
            <TableHead class="ps-5">{{ $t("customers.fields.name") }}</TableHead>
            <TableHead>{{ $t("customers.fields.phone") }}</TableHead>
            <TableHead class="text-end">{{ $t("customers.balance") }}</TableHead>
            <TableHead>{{ $t("customers.lastActivity") }}</TableHead>
            <TableHead class="w-14 pe-5"><span class="sr-only">{{ $t("settings.actions") }}</span></TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-if="!customers.length" class="hover:bg-transparent">
            <TableCell colspan="5" class="py-12 text-center text-muted-foreground">
              {{ search || owing ? $t("customers.noneFound") : $t("customers.empty") }}
            </TableCell>
          </TableRow>
          <TableRow v-for="c in customers" :key="c.id" class="cursor-pointer" :class="c.is_active ? '' : 'opacity-60'" @click="openAccount(c)">
            <TableCell class="ps-5 font-medium"><bdi>{{ c.name }}</bdi></TableCell>
            <TableCell class="num text-muted-foreground" dir="ltr">{{ c.phone ?? "—" }}</TableCell>
            <TableCell class="num text-end font-semibold" :class="c.balance > 0 ? 'text-destructive' : 'text-muted-foreground'">
              <span class="inline-flex items-center gap-1">
                <TriangleAlert v-if="overLimit(c)" :size="14" :aria-label="$t('customers.overLimit')" />
                {{ c.balance > 0 ? formatCurrency(c.balance) : "—" }}
              </span>
            </TableCell>
            <TableCell class="text-muted-foreground">{{ c.last_activity ? formatDate(new Date(c.last_activity)) : "—" }}</TableCell>
            <TableCell class="pe-5">
              <Button
                v-if="auth.can('customers.credit') && c.balance > 0"
                variant="ghost"
                size="icon-sm"
                :aria-label="$t('customers.takePayment')"
                :title="$t('customers.takePayment')"
                @click.stop="pay(c)"
              >
                <HandCoins />
              </Button>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>

    <CustomerSheet v-model:open="sheetOpen" :customer="editing" @saved="load" />
    <CustomerAccountSheet v-model:open="accountOpen" :customer-id="accountId" @changed="load" @edit="startEdit" />
    <CustomerPaymentDialog v-model:open="payOpen" :customer="paying" @paid="load" />
  </div>
</template>
