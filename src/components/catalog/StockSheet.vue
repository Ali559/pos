<script setup lang="ts">
import DateInput from "@/components/pickers/DateInput.vue";
import { computed, ref, watch } from "vue";
import { Loader2 } from "lucide-vue-next";
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
import { listBatches, type Batch } from "@/db/stock";
import type { Product } from "@/db/catalog";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatDateTime } from "@/lib/locale";

const open = defineModel<boolean>("open", { required: true });
const props = defineProps<{ product: Product | null }>();
const emit = defineEmits<{ saved: [] }>();

const mode = ref<"receive" | "remove">("receive");
const batches = ref<Batch[]>([]);
const form = ref({ quantity: "", expiry: "", cost: "", reason: "damaged", batchId: "fifo", note: "" });
const saving = ref(false);
const error = ref("");

const today = () => new Date().toLocaleDateString("en-CA"); // YYYY-MM-DD, local
const soon = (d: string) => {
  const t = new Date(`${d}T00:00`);
  return t.getTime() - Date.now() < 7 * 86_400_000;
};

watch(open, async (isOpen) => {
  if (!isOpen || !props.product) return;
  mode.value = "receive";
  error.value = "";
  form.value = { quantity: "", expiry: "", cost: "", reason: "damaged", batchId: "fifo", note: "" };
  batches.value = await listBatches(props.product.id);
});

const tracked = computed(() => batches.value.reduce((n, b) => n + b.quantity_remaining, 0));

const save = async () => {
  if (!props.product) return;
  const f = form.value;
  saving.value = true;
  error.value = "";
  try {
    if (mode.value === "receive") {
      await call("receive_stock", {
        productId: props.product.id,
        quantity: Number(f.quantity),
        expiryDate: f.expiry || null,
        costPrice: f.cost === "" ? null : Number(f.cost),
        note: f.note || null,
      });
    } else {
      await call("remove_stock", {
        productId: props.product.id,
        quantity: Number(f.quantity),
        reason: f.reason,
        batchId: f.batchId === "fifo" ? null : Number(f.batchId),
        note: f.note || null,
      });
    }
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
    :title="$t('stock.title')"
    :description="product ? `${product.name} — ${$t('stock.onHand', { n: product.stock })}` : ''"
    @submit="save"
  >
    <div class="grid grid-cols-2 rounded-lg bg-muted p-1" role="tablist">
      <button
        v-for="m in ['receive', 'remove'] as const"
        :key="m"
        type="button"
        role="tab"
        :aria-selected="mode === m"
        class="h-9 rounded-md text-sm font-medium text-muted-foreground aria-selected:bg-card aria-selected:text-foreground aria-selected:shadow-sm"
        @click="mode = m"
      >
        {{ $t(`stock.${m}`) }}
      </button>
    </div>

    <div class="space-y-2">
      <Label for="s-qty">{{ $t("stock.quantity") }}</Label>
      <Input id="s-qty" v-model="form.quantity" type="number" min="0" step="any" required dir="ltr" />
    </div>

    <template v-if="mode === 'receive'">
      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label for="s-exp">
            {{ $t("stock.expiry") }}
            <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
          </Label>
          <DateInput id="s-exp" v-model="form.expiry" :min="today()" clearable :placeholder="$t('stock.noExpiry')" />
        </div>
        <div class="space-y-2">
          <Label for="s-cost">
            {{ $t("catalog.costPrice") }}
            <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
          </Label>
          <Input id="s-cost" v-model="form.cost" type="number" min="0" step="any" dir="ltr" />
        </div>
      </div>
      <p class="-mt-2 text-xs text-muted-foreground">{{ $t("stock.costHint") }}</p>
    </template>

    <template v-else>
      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label>{{ $t("stock.reason") }}</Label>
          <Select v-model="form.reason" :key="$i18n.locale">
            <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem v-for="r in ['damaged', 'expired', 'correction', 'other']" :key="r" :value="r">
                {{ $t(`stock.reasons.${r}`) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-2">
          <Label>{{ $t("stock.fromBatch") }}</Label>
          <Select v-model="form.batchId" :key="$i18n.locale">
            <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="fifo">{{ $t("stock.soonestFirst") }}</SelectItem>
              <SelectItem v-for="b in batches" :key="b.id" :value="String(b.id)">
                {{ b.expiry_date ?? $t("stock.noExpiry") }} ({{ b.quantity_remaining }})
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
    </template>

    <div class="space-y-2">
      <Label for="s-note">
        {{ $t("schedule.note") }}
        <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
      </Label>
      <Input id="s-note" v-model="form.note" />
    </div>

    <section class="space-y-2">
      <h3 class="text-sm font-semibold">{{ $t("stock.batches") }}</h3>
      <p v-if="!batches.length" class="text-sm text-muted-foreground">{{ $t("stock.noBatches") }}</p>
      <ul v-else class="divide-y rounded-lg border text-sm">
        <li v-for="b in batches" :key="b.id" class="flex items-center gap-3 px-3 py-2">
          <span
            class="num w-28"
            dir="ltr"
            :class="{
              'font-semibold text-destructive': b.expiry_date && b.expiry_date < today(),
              'font-semibold text-warning': b.expiry_date && b.expiry_date >= today() && soon(b.expiry_date),
            }"
          >
            {{ b.expiry_date ?? $t("stock.noExpiry") }}
          </span>
          <span class="flex-1 text-muted-foreground">
            {{ $t("stock.received", { date: formatDateTime(new Date(b.received_at)) }) }}
          </span>
          <span class="num font-medium">{{ b.quantity_remaining }}</span>
        </li>
      </ul>
      <p v-if="product && tracked < product.stock" class="text-xs text-muted-foreground">
        {{ $t("stock.untracked", { n: product.stock - tracked }) }}
      </p>
    </section>

    <template #footer>
      <Button type="submit" :disabled="saving">
        <Loader2 v-if="saving" class="animate-spin" />
        {{ mode === "receive" ? $t("stock.receive") : $t("stock.remove") }}
      </Button>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
    </template>
  </EditSheet>
</template>
