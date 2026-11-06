<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Plus, Search, UserRound } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import CustomerSheet from "./CustomerSheet.vue";
import { getCustomer, listCustomers, type Customer } from "@/db/customers";
import { formatCurrency } from "@/lib/format";
import { useAuth } from "@/stores/auth";

// Finds the customer for the sale at the till, by phone or name, or adds a
// new one. Emits the chosen customer.
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ pick: [customer: Customer] }>();

const auth = useAuth();
const canCreate = computed(() => auth.can("customers.edit") || auth.can("customers.credit"));
const q = ref("");
const results = ref<Customer[]>([]);
const input = ref<HTMLInputElement | null>(null);
const sheetOpen = ref(false);

const search = async () => {
  results.value = await listCustomers({ search: q.value, limit: 8 });
};
let timer: ReturnType<typeof setTimeout>;
watch(q, () => {
  clearTimeout(timer);
  timer = setTimeout(search, 150);
});
watch(open, (v) => {
  if (!v) return;
  q.value = "";
  search();
  nextTick(() => input.value?.focus());
});

const pick = (c: Customer) => {
  emit("pick", c);
  open.value = false;
};

// The sheet is its own modal: close this one so it isn't left on top
const addNew = () => {
  open.value = false;
  sheetOpen.value = true;
};
const created = async (id: number) => {
  const c = await getCustomer(id);
  if (c) pick(c);
};
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-md">
      <div class="space-y-3">
        <div>
          <DialogTitle class="flex items-center gap-2 font-semibold"><UserRound :size="18" />{{ $t("customers.pickTitle") }}</DialogTitle>
          <DialogDescription class="text-sm text-muted-foreground">{{ $t("customers.pickHint") }}</DialogDescription>
        </div>
        <form class="relative" @submit.prevent="results[0] && pick(results[0])">
          <Search :size="18" class="pointer-events-none absolute start-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            ref="input"
            v-model="q"
            type="search"
            autocomplete="off"
            :placeholder="$t('customers.searchPlaceholder')"
            :aria-label="$t('customers.searchPlaceholder')"
            class="h-11 w-full rounded-md border-2 border-input bg-card ps-10 pe-3 outline-none focus:border-primary"
          />
        </form>
        <ul v-if="results.length" class="max-h-72 divide-y overflow-y-auto rounded-lg border">
          <li v-for="c in results" :key="c.id">
            <button type="button" class="flex w-full items-center gap-3 px-3 py-2.5 text-start hover:bg-muted" @click="pick(c)">
              <span class="min-w-0 flex-1">
                <bdi class="block font-medium">{{ c.name }}</bdi>
                <span v-if="c.phone" class="num text-xs text-muted-foreground" dir="ltr">{{ c.phone }}</span>
              </span>
              <span v-if="c.balance > 0" class="num text-sm font-semibold text-destructive">{{ formatCurrency(c.balance) }}</span>
            </button>
          </li>
        </ul>
        <p v-else class="py-4 text-center text-sm text-muted-foreground">{{ $t("customers.noneFound") }}</p>
        <Button v-if="canCreate" type="button" variant="outline" class="w-full" @click="addNew">
          <Plus />{{ q.trim() ? $t("customers.addNamed", { name: q.trim() }) : $t("customers.add") }}
        </Button>
      </div>
    </DialogContent>
  </Dialog>
  <CustomerSheet v-model:open="sheetOpen" :customer="null" :initial-name="q" @saved="created" />
</template>
