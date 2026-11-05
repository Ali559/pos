<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Clock, PauseCircle, Play, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { select } from "@/db/client";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatTime } from "@/lib/locale";

// Parking a cart to serve the next customer, and picking it back up.
// The cart is stored as the register keeps it; checkout re-prices it.
export interface HeldCart {
  items: { id: number; name: string; price: number; quantity: number; discount: number }[];
  cartDiscount: number;
}

const props = defineProps<{ cart: HeldCart; canHold: boolean; canResume: boolean }>();
const emit = defineEmits<{ held: []; resume: [cart: HeldCart] }>();

interface HeldRow {
  id: number;
  label: string | null;
  created_at: string;
  cashier: string;
  sale_data: string;
}

const rows = ref<HeldRow[]>([]);
const holdOpen = ref(false);
const listOpen = ref(false);
const label = ref("");
const error = ref("");

const load = async () => {
  rows.value = await select<HeldRow>(
    `SELECT h.id, h.customer_name AS label, h.created_at, u.full_name AS cashier, h.sale_data
     FROM held_sales h JOIN users u ON u.id = h.user_id ORDER BY h.id`,
  );
};
onMounted(load);

const summary = (r: HeldRow) => {
  try {
    const cart = JSON.parse(r.sale_data) as HeldCart;
    return cart.items.map((i) => (i.quantity > 1 ? `${i.quantity}× ${i.name}` : i.name)).join("، ");
  } catch {
    return "";
  }
};

const hold = async () => {
  error.value = "";
  try {
    await call("hold_sale", { label: label.value || null, cart: props.cart });
    label.value = "";
    holdOpen.value = false;
    emit("held");
    await load();
  } catch (e) {
    error.value = errorMessage(e);
  }
};

const take = async (id: number, resume: boolean) => {
  error.value = "";
  try {
    const cart = await call<HeldCart>("take_held_sale", { id });
    if (resume) {
      emit("resume", cart);
      listOpen.value = false;
    }
    await load();
  } catch (e) {
    error.value = errorMessage(e);
  }
};
</script>

<template>
  <div class="contents">
    <Popover v-model:open="holdOpen">
      <PopoverTrigger as-child>
        <Button variant="outline" size="lg" :disabled="!canHold">
          <PauseCircle />
          {{ $t("pos.hold") }}
        </Button>
      </PopoverTrigger>
      <PopoverContent class="w-72">
        <form class="space-y-3" @submit.prevent="hold">
          <label for="hold-label" class="text-sm font-medium">{{ $t("held.label") }}</label>
          <Input id="hold-label" v-model="label" dir="auto" autofocus :placeholder="$t('held.labelHint')" />
          <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
          <Button type="submit" class="w-full"><PauseCircle /> {{ $t("held.hold") }}</Button>
        </form>
      </PopoverContent>
    </Popover>

    <Popover v-model:open="listOpen">
      <PopoverTrigger as-child>
        <Button variant="outline" size="lg" :disabled="!rows.length">
          <Clock />
          {{ $t("held.count", { n: rows.length }) }}
        </Button>
      </PopoverTrigger>
      <PopoverContent class="w-96 p-0" align="end">
        <p class="border-b px-4 py-3 font-semibold">{{ $t("held.title") }}</p>
        <p v-if="!canResume" class="border-b bg-muted/60 px-4 py-2 text-xs text-muted-foreground">{{ $t("held.finishFirst") }}</p>
        <ul class="max-h-80 divide-y overflow-y-auto">
          <li v-for="r in rows" :key="r.id" class="flex items-center gap-3 px-4 py-2.5">
            <div class="min-w-0 flex-1">
              <p class="truncate font-medium"><bdi>{{ r.label || $t("held.untitled") }}</bdi></p>
              <p class="truncate text-xs text-muted-foreground">
                {{ formatTime(new Date(r.created_at)) }} — {{ r.cashier }} — <bdi>{{ summary(r) }}</bdi>
              </p>
            </div>
            <Button size="sm" :disabled="!canResume" @click="take(r.id, true)"><Play /> {{ $t("held.resume") }}</Button>
            <button
              type="button"
              class="grid size-8 place-items-center rounded-md text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              :aria-label="$t('held.discard')"
              @click="take(r.id, false)"
            >
              <Trash2 :size="16" />
            </button>
          </li>
        </ul>
        <p v-if="error" role="alert" class="px-4 py-2 text-sm text-destructive">{{ error }}</p>
      </PopoverContent>
    </Popover>
  </div>
</template>
