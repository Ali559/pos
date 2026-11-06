<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ArrowLeftRight, Minus, Plus, Undo2 } from "lucide-vue-next";
import { useRouter } from "vue-router";
import { useReturnCart } from "@/stores/returnCart";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import ManagerPinDialog from "@/components/sales/ManagerPinDialog.vue";
import type { SaleDetail } from "@/db/sales";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import { useAuth } from "@/stores/auth";
import { useShop } from "@/stores/shop";

// Pick what comes back and how many; Rust works out the refund from the
// sale as it was charged (discounts and tax included).
const props = defineProps<{ sale: SaleDetail }>();
const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ done: [refund: number] }>();
const auth = useAuth();
const { currency } = useShop();
const pinDialog = ref<InstanceType<typeof ManagerPinDialog> | null>(null);

const rows = ref<{ id: number; qty: number; restock: boolean }[]>([]);
const reason = ref("");
const method = ref<"cash" | "card" | "credit">("cash");
// A sale with a customer can be refunded onto their account (less debt)
const methods = computed(() => (props.sale.customer_id ? (["cash", "card", "credit"] as const) : (["cash", "card"] as const)));
const busy = ref(false);
const error = ref("");

watch(open, (isOpen) => {
  if (!isOpen) return;
  rows.value = props.sale.lines.map((l) => ({ id: l.id, qty: 0, restock: true }));
  reason.value = "";
  method.value = props.sale.method === "card" ? "card" : props.sale.method === "credit" ? "credit" : "cash";
  error.value = "";
});

const left = (i: number) => props.sale.lines[i]!.quantity - props.sale.lines[i]!.returned;
const step = (i: number, by: number) => {
  const r = rows.value[i]!;
  r.qty = Math.max(0, Math.min(left(i), r.qty + by));
};
const takeAll = () => rows.value.forEach((r, i) => (r.qty = left(i)));

const round = (n: number) => (currency.value === "IQD" ? Math.round(n) : Math.round(n * 100) / 100);
// Same arithmetic as returns::create_return, for the preview only
const lineRefund = (i: number, qty: number) => {
  const l = props.sale.lines[i]!;
  const sub = round((l.subtotal / l.quantity) * qty);
  return sub + round((sub * l.tax_rate) / 100);
};
const refund = computed(() => rows.value.reduce((sum, r, i) => sum + (r.qty ? lineRefund(i, r.qty) : 0), 0));

// Exchange: take the picked items to the register, where they pay for
// whatever the customer takes instead
const router = useRouter();
const returnCart = useReturnCart();
const exchange = () => {
  returnCart.fromSale(
    { id: props.sale.id, receipt: props.sale.receipt_number },
    rows.value
      .map((r, i) => ({ r, i, l: props.sale.lines[i]! }))
      .filter(({ r }) => r.qty > 0)
      .map(({ r, i, l }) => ({
        saleItemId: l.id,
        productId: l.product_id,
        name: l.name,
        quantity: r.qty,
        max: left(i),
        unitRefund: lineRefund(i, r.qty) / r.qty,
        restock: r.restock,
      })),
  );
  returnCart.reason.value = reason.value;
  open.value = false;
  router.push("/main/point-of-sale");
};
const any = computed(() => rows.value.some((r) => r.qty > 0));

const submit = async () => {
  if (!any.value || busy.value) return;
  error.value = "";
  let overrideToken: string | null = null;
  if (!auth.can("sales.refund")) {
    const grant = await pinDialog.value?.ask("sales.refund");
    if (!grant) return;
    overrideToken = grant.token;
  }
  busy.value = true;
  try {
    const r = await call<{ id: number; refund: number }>("create_return", {
      saleId: props.sale.id,
      lines: rows.value.filter((r) => r.qty > 0).map((r) => ({ saleItemId: r.id, quantity: r.qty, restock: r.restock })),
      reason: reason.value || null,
      refundMethod: method.value,
      overrideToken,
    });
    open.value = false;
    emit("done", r.refund);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-xl">
      <form class="space-y-5" @submit.prevent="submit">
        <div>
          <DialogTitle class="text-lg font-semibold">{{ $t("returns.title") }}</DialogTitle>
          <DialogDescription class="text-sm text-muted-foreground">
            <span class="num" dir="ltr">{{ sale.receipt_number }}</span> — {{ $t("returns.hint") }}
          </DialogDescription>
        </div>

        <ul class="divide-y rounded-lg border">
          <li v-for="(l, i) in sale.lines" :key="l.id" class="flex flex-wrap items-center gap-3 px-3 py-2.5">
            <div class="min-w-0 flex-1">
              <p class="truncate font-medium"><bdi>{{ l.name }}</bdi></p>
              <p class="text-xs text-muted-foreground">
                {{ $t("returns.soldLeft", { sold: l.quantity, left: left(i) }) }}
              </p>
            </div>
            <template v-if="left(i) > 0">
              <label class="flex items-center gap-1.5 text-xs text-muted-foreground">
                <input v-model="rows[i]!.restock" type="checkbox" class="size-4 accent-primary" :disabled="!rows[i]!.qty" />
                {{ $t("returns.restock") }}
              </label>
              <div class="flex items-center gap-1">
                <button type="button" class="grid size-8 place-items-center rounded-md border hover:bg-accent" :aria-label="`−1 ${l.name}`" @click="step(i, -1)">
                  <Minus :size="14" />
                </button>
                <span class="num w-8 text-center font-semibold">{{ rows[i]!.qty }}</span>
                <button type="button" class="grid size-8 place-items-center rounded-md border hover:bg-accent" :aria-label="`+1 ${l.name}`" @click="step(i, 1)">
                  <Plus :size="14" />
                </button>
              </div>
            </template>
            <span v-else class="text-xs font-medium text-muted-foreground">{{ $t("returns.allBack") }}</span>
          </li>
        </ul>
        <Button type="button" variant="link" size="sm" class="-mt-3 px-0" @click="takeAll">{{ $t("returns.everything") }}</Button>

        <div class="grid gap-4 sm:grid-cols-2">
          <div class="space-y-2">
            <Label for="ret-reason">{{ $t("returns.reason") }} <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span></Label>
            <Input id="ret-reason" v-model="reason" dir="auto" />
          </div>
          <div class="space-y-2">
            <Label>{{ $t("returns.refundBy") }}</Label>
            <div class="grid gap-1 rounded-lg bg-muted p-1" :class="methods.length === 3 ? 'grid-cols-3' : 'grid-cols-2'" role="radiogroup">
              <button
                v-for="m in methods"
                :key="m"
                type="button"
                role="radio"
                :aria-checked="method === m"
                class="h-8 rounded-md text-sm font-medium aria-checked:bg-card aria-checked:shadow-sm"
                @click="method = m"
              >
                {{ $t(`pos.${m}`) }}
              </button>
            </div>
          </div>
        </div>

        <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
        <div class="flex items-center justify-between gap-3 border-t pt-4">
          <p>
            <span class="block text-sm text-muted-foreground">{{ $t("returns.refund") }}</span>
            <span class="num text-2xl font-bold">{{ formatCurrency(refund) }}</span>
          </p>
          <div class="flex gap-2">
            <Button type="button" size="lg" variant="outline" :disabled="!any || busy" @click="exchange">
              <ArrowLeftRight /> {{ $t("returns.exchange") }}
            </Button>
            <Button type="submit" size="lg" :disabled="!any || busy"><Undo2 /> {{ $t("returns.confirm") }}</Button>
          </div>
        </div>
      </form>
      <ManagerPinDialog ref="pinDialog" />
    </DialogContent>
  </Dialog>
</template>
