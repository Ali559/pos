<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Archive, ArchiveRestore, Loader2, Pencil, Plus } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import EditSheet from "@/components/EditSheet.vue";
import AppIcon from "@/components/icons/AppIcon.vue";
import IconPicker from "@/components/icons/IconPicker.vue";
import { listExpenseTypes, type ExpenseType } from "@/db/expenses";
import { categoryInk, categoryTint } from "@/lib/categoryColor";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { useAuth } from "@/stores/auth";

const auth = useAuth();
const canEdit = computed(() => auth.can("expenses.edit"));
const COLORS = ["#2d5a8f", "#94600c", "#5f4996", "#9a3f26", "#2f6b38", "#0e7490", "#be185d", "#57534e"];

const types = ref<ExpenseType[]>([]);
const load = async () => {
  types.value = await listExpenseTypes(true);
};
onMounted(load);

const open = ref(false);
const editing = ref<ExpenseType | null>(null);
const form = ref({ name: "", iconKey: null as string | null, color: COLORS[0] as string | null });
const saving = ref(false);
const error = ref("");

const startCreate = () => {
  editing.value = null;
  form.value = { name: "", iconKey: null, color: COLORS[types.value.length % COLORS.length] };
  error.value = "";
  open.value = true;
};
const startEdit = (t: ExpenseType) => {
  editing.value = t;
  form.value = { name: t.name, iconKey: t.icon_key, color: t.color };
  error.value = "";
  open.value = true;
};

const save = async () => {
  const f = form.value;
  const type = { name: f.name, iconKey: f.iconKey ?? "", color: f.color };
  saving.value = true;
  error.value = "";
  try {
    if (editing.value) await call("update_expense_type", { id: editing.value.id, type });
    else await call("create_expense_type", type);
    open.value = false;
    await load();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};

const toggle = async (t: ExpenseType) => {
  await call("set_expense_type_active", { id: t.id, active: !t.is_active });
  await load();
};
const suggestText = computed(() => form.value.name);
</script>

<template>
  <div class="space-y-5">
    <div class="flex">
      <Button v-if="canEdit" size="lg" class="ms-auto" @click="startCreate"><Plus /> {{ $t("expenses.addType") }}</Button>
    </div>
    <ul class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      <li v-for="t in types" :key="t.id" class="flex items-center gap-3 rounded-xl border bg-card p-4" :class="!t.is_active && 'opacity-55'">
        <span class="grid size-11 shrink-0 place-items-center rounded-lg" :style="{ background: categoryTint(t.color), color: categoryInk(t.color) }">
          <AppIcon :icon="t.icon_key" :size="22" />
        </span>
        <div class="min-w-0 flex-1">
          <div class="truncate font-semibold">{{ t.name }}</div>
          <div v-if="!t.is_active" class="text-xs text-muted-foreground">{{ $t("catalog.archived") }}</div>
        </div>
        <template v-if="canEdit">
          <Button variant="ghost" size="icon-sm" :aria-label="$t('settings.edit')" @click="startEdit(t)"><Pencil /></Button>
          <Button variant="ghost" size="icon-sm" class="text-muted-foreground" :aria-label="t.is_active ? $t('catalog.archive') : $t('catalog.restore')" @click="toggle(t)">
            <Archive v-if="t.is_active" /><ArchiveRestore v-else />
          </Button>
        </template>
      </li>
    </ul>

    <EditSheet v-model:open="open" :title="editing ? $t('expenses.editType') : $t('expenses.addType')" @submit="save">
      <div class="space-y-2">
        <Label for="t-name">{{ $t("catalog.name") }}</Label>
        <Input id="t-name" v-model="form.name" required dir="auto" />
      </div>
      <div class="space-y-2">
        <Label>{{ $t("catalog.icon") }}</Label>
        <IconPicker v-model="form.iconKey" :suggest-from="suggestText" />
      </div>
      <fieldset class="space-y-2">
        <legend class="text-sm font-medium">{{ $t("catalog.color") }}</legend>
        <div class="flex flex-wrap gap-2">
          <button
            v-for="c in COLORS"
            :key="c"
            type="button"
            class="size-9 rounded-full ring-offset-2 ring-offset-background aria-pressed:ring-2 aria-pressed:ring-foreground"
            :style="{ background: c }"
            :aria-pressed="form.color === c"
            :aria-label="c"
            @click="form.color = c"
          />
        </div>
      </fieldset>
      <template #footer>
        <Button type="submit" :disabled="saving">
          <Loader2 v-if="saving" class="animate-spin" />
          {{ editing ? $t("common.save") : $t("expenses.addType") }}
        </Button>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      </template>
    </EditSheet>
  </div>
</template>
