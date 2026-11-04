<script setup lang="ts">
import DateInput from "@/components/pickers/DateInput.vue";
import { computed, onMounted, ref, watch } from "vue";
import { Check, Image, Loader2, Pencil, Plus, Repeat, Trash2 } from "lucide-vue-next";
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
import AppIcon from "@/components/icons/AppIcon.vue";
import MediaGallery from "@/components/media/MediaGallery.vue";
import RangePicker, { resolveRange, type RangeModel } from "@/components/reports/RangePicker.vue";
import { listExpenses, listExpenseTypes, type Expense, type ExpenseType } from "@/db/expenses";
import { categoryInk, categoryTint } from "@/lib/categoryColor";
import { toLocalDate } from "@/lib/dateRange";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatCurrency } from "@/lib/format";
import type { MediaFile } from "@/lib/media";
import { useAuth } from "@/stores/auth";

const auth = useAuth();
const canEdit = computed(() => auth.can("expenses.edit"));

const range = ref<RangeModel>({ preset: "thisMonth" });
const typeFilter = ref("all");
const types = ref<ExpenseType[]>([]);
const rows = ref<Expense[]>([]);
const listError = ref("");

const load = async () => {
  const r = resolveRange(range.value);
  [types.value, rows.value] = await Promise.all([
    listExpenseTypes(),
    listExpenses({
      from: toLocalDate(r.from),
      to: toLocalDate(r.to),
      typeId: typeFilter.value === "all" ? null : Number(typeFilter.value),
    }),
  ]);
};
onMounted(load);
watch([range, typeFilter], load, { deep: true });

const drafts = computed(() => rows.value.filter((e) => e.status === "draft"));
const confirmed = computed(() => rows.value.filter((e) => e.status === "confirmed"));
const total = computed(() => confirmed.value.reduce((n, e) => n + e.amount, 0));

// Per-type totals for the summary bars, largest first
const byType = computed(() => {
  const map = new Map<number, { e: Expense; sum: number }>();
  for (const e of confirmed.value) {
    const cur = map.get(e.type_id);
    map.set(e.type_id, { e, sum: (cur?.sum ?? 0) + e.amount });
  }
  return [...map.values()].sort((a, b) => b.sum - a.sum);
});

const run = async (fn: () => Promise<unknown>) => {
  listError.value = "";
  try {
    await fn();
    await load();
  } catch (e) {
    listError.value = errorMessage(e);
  }
};
const confirm = (e: Expense) => run(() => call("confirm_expense", { id: e.id }));
const remove = (e: Expense) => run(() => call("delete_expense", { id: e.id }));

// ----------------------------------------------------------- edit sheet

const open = ref(false);
const editing = ref<Expense | null>(null);
const form = ref({ typeId: "", amount: "", spentOn: "", note: "", recurring: "none" });
const receipt = ref<MediaFile[]>([]);
const uploading = ref(false);
const saving = ref(false);
const error = ref("");

const startCreate = () => {
  editing.value = null;
  form.value = {
    typeId: String(types.value[0]?.id ?? ""),
    amount: "",
    spentOn: toLocalDate(new Date()),
    note: "",
    recurring: "none",
  };
  receipt.value = [];
  error.value = "";
  open.value = true;
};

const startEdit = (e: Expense) => {
  editing.value = e;
  form.value = {
    typeId: String(e.type_id),
    amount: String(e.amount),
    spentOn: e.spent_on,
    note: e.note ?? "",
    recurring: e.recurring ?? "none",
  };
  receipt.value = e.receipt ? [e.receipt] : [];
  error.value = "";
  open.value = true;
};

const save = async () => {
  const f = form.value;
  const expense = {
    typeId: Number(f.typeId),
    amount: Number(f.amount) || 0,
    spentOn: f.spentOn,
    note: f.note || null,
    receiptMediaId: receipt.value[0]?.id ?? null,
    recurring: f.recurring === "none" ? null : f.recurring,
  };
  saving.value = true;
  error.value = "";
  try {
    if (editing.value) await call("update_expense", { id: editing.value.id, expense });
    else await call("create_expense", expense);
    open.value = false;
    await load();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};

const isDraftEdit = computed(() => editing.value?.status === "draft" || !!editing.value?.recurring_parent_id);
</script>

<template>
  <div class="space-y-5">
    <div class="flex flex-wrap items-center gap-3">
      <RangePicker v-model="range" :presets="['thisMonth', 'lastMonth', 'last3Months', 'thisYear', 'custom']" />
      <Select v-model="typeFilter" :key="$i18n.locale">
        <SelectTrigger class="w-48"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem value="all">{{ $t("expenses.allTypes") }}</SelectItem>
          <SelectItem v-for="t in types" :key="t.id" :value="String(t.id)">{{ t.name }}</SelectItem>
        </SelectContent>
      </Select>
      <Button v-if="canEdit" size="lg" class="ms-auto" @click="startCreate">
        <Plus /> {{ $t("expenses.add") }}
      </Button>
    </div>
    <p v-if="listError" role="alert" class="text-sm text-destructive">{{ listError }}</p>

    <!-- Recurring drafts waiting for someone to check the amount -->
    <section v-if="drafts.length" class="rounded-xl border border-warning/40 bg-warning/8 p-4">
      <h2 class="mb-3 flex items-center gap-2 font-semibold">
        <Repeat :size="16" class="text-warning" /> {{ $t("expenses.draftsTitle", drafts.length) }}
      </h2>
      <ul class="divide-y rounded-lg border bg-card">
        <li v-for="e in drafts" :key="e.id" class="flex flex-wrap items-center gap-3 px-4 py-2.5">
          <AppIcon :icon="e.type_icon" :size="18" />
          <span class="font-medium">{{ e.type_name }}</span>
          <span class="num text-sm text-muted-foreground" dir="ltr">{{ e.spent_on }}</span>
          <span class="ms-auto font-semibold">{{ formatCurrency(e.amount) }}</span>
          <template v-if="canEdit">
            <Button size="sm" @click="confirm(e)"><Check /> {{ $t("expenses.confirm") }}</Button>
            <Button size="sm" variant="ghost" @click="startEdit(e)"><Pencil /> {{ $t("settings.edit") }}</Button>
            <Button size="icon-sm" variant="ghost" class="text-muted-foreground hover:text-destructive" :aria-label="$t('expenses.skip')" :title="$t('expenses.skip')" @click="remove(e)">
              <Trash2 />
            </Button>
          </template>
        </li>
      </ul>
    </section>

    <div class="grid gap-5 lg:grid-cols-[1fr_2fr]">
      <section class="rounded-xl border bg-card p-5">
        <p class="text-sm text-muted-foreground">{{ $t("expenses.total") }}</p>
        <p class="mt-1 text-3xl font-semibold tracking-tight">{{ formatCurrency(total) }}</p>
        <ul class="mt-5 space-y-3">
          <li v-for="{ e, sum } in byType" :key="e.type_id" class="space-y-1">
            <div class="flex items-center gap-2 text-sm">
              <AppIcon :icon="e.type_icon" :size="16" :style="{ color: categoryInk(e.type_color) }" />
              <span class="flex-1">{{ e.type_name }}</span>
              <span class="num font-medium">{{ formatCurrency(sum) }}</span>
            </div>
            <div class="h-1.5 overflow-hidden rounded-full bg-muted">
              <div class="h-full rounded-full" :style="{ width: `${(sum / total) * 100}%`, background: categoryInk(e.type_color) }" />
            </div>
          </li>
        </ul>
        <p v-if="!byType.length" class="mt-4 text-sm text-muted-foreground">{{ $t("expenses.none") }}</p>
      </section>

      <section class="overflow-hidden rounded-xl border bg-card">
        <ul class="divide-y">
          <li v-if="!confirmed.length" class="px-5 py-14 text-center text-muted-foreground">{{ $t("expenses.none") }}</li>
          <li v-for="e in confirmed" :key="e.id" class="flex items-center gap-3 px-5 py-3">
            <span
              class="grid size-9 shrink-0 place-items-center rounded-lg"
              :style="{ background: categoryTint(e.type_color), color: categoryInk(e.type_color) }"
            >
              <AppIcon :icon="e.type_icon" :size="18" />
            </span>
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2 font-medium">
                {{ e.type_name }}
                <Repeat v-if="e.recurring" :size="14" class="text-muted-foreground" :aria-label="$t(`expenses.repeat.${e.recurring}`)" />
                <Image v-if="e.receipt" :size="14" class="text-muted-foreground" :aria-label="$t('expenses.hasReceipt')" />
              </div>
              <div class="truncate text-xs text-muted-foreground">
                <span class="num" dir="ltr">{{ e.spent_on }}</span>
                <template v-if="e.note"> — {{ e.note }}</template>
              </div>
            </div>
            <span class="num font-semibold">{{ formatCurrency(e.amount) }}</span>
            <Button v-if="canEdit" variant="ghost" size="icon-sm" :aria-label="$t('settings.edit')" @click="startEdit(e)">
              <Pencil />
            </Button>
          </li>
        </ul>
      </section>
    </div>

    <EditSheet v-model:open="open" :title="editing ? $t('expenses.edit') : $t('expenses.add')" @submit="save">
      <div class="space-y-2">
        <Label>{{ $t("expenses.type") }}</Label>
        <Select v-model="form.typeId" :key="$i18n.locale">
          <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem v-for="t in types" :key="t.id" :value="String(t.id)">{{ t.name }}</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label for="x-amount">{{ $t("expenses.amount") }}</Label>
          <Input id="x-amount" v-model="form.amount" type="number" min="0" step="any" required dir="ltr" />
        </div>
        <div class="space-y-2">
          <Label for="x-date">{{ $t("schedule.date") }}</Label>
          <DateInput id="x-date" v-model="form.spentOn" required />
        </div>
      </div>
      <div v-if="!isDraftEdit" class="space-y-2">
        <Label>{{ $t("expenses.repeats") }}</Label>
        <Select v-model="form.recurring" :key="$i18n.locale">
          <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="none">{{ $t("expenses.repeat.none") }}</SelectItem>
            <SelectItem value="weekly">{{ $t("expenses.repeat.weekly") }}</SelectItem>
            <SelectItem value="monthly">{{ $t("expenses.repeat.monthly") }}</SelectItem>
          </SelectContent>
        </Select>
        <p class="text-xs text-muted-foreground">{{ $t("expenses.repeatHint") }}</p>
      </div>
      <div class="space-y-2">
        <Label for="x-note">
          {{ $t("schedule.note") }}
          <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
        </Label>
        <Input id="x-note" v-model="form.note" />
      </div>
      <div class="space-y-2">
        <Label>{{ $t("expenses.receipt") }}</Label>
        <MediaGallery v-model="receipt" v-model:busy="uploading" />
      </div>

      <template #footer>
        <Button type="submit" :disabled="saving || uploading">
          <Loader2 v-if="saving || uploading" class="animate-spin" />
          {{ editing ? $t("common.save") : $t("expenses.add") }}
        </Button>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <Button
          v-if="editing && canEdit"
          type="button"
          variant="ghost"
          class="ms-auto text-muted-foreground hover:text-destructive"
          @click="remove(editing); open = false"
        >
          <Trash2 /> {{ $t("settings.delete") }}
        </Button>
      </template>
    </EditSheet>
  </div>
</template>
