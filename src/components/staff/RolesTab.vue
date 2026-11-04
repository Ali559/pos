<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Copy, Loader2, Lock, Pencil, Plus, ShieldAlert, Trash2 } from "lucide-vue-next";
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
import {
  listPermissions,
  listRolePermissions,
  listRoles,
  type Permission,
  type Role,
} from "@/db/staff";
import { call } from "@/lib/ipc";
import { errorMessage, isAppError } from "@/lib/errors";
import { useAuth } from "@/stores/auth";
import { localized } from "@/utils/i18n";

const { t } = useI18n();
const auth = useAuth();

const roles = ref<Role[]>([]);
const permissions = ref<Permission[]>([]);
const counts = ref<Record<number, number>>({});

const load = async () => {
  [roles.value, permissions.value] = await Promise.all([listRoles(), listPermissions()]);
  const entries = await Promise.all(
    roles.value.map(async (r) => [r.id, (await listRolePermissions(r.id)).length] as const),
  );
  counts.value = Object.fromEntries(entries);
};
onMounted(load);

const groups = computed(() => {
  const map = new Map<string, Permission[]>();
  for (const p of permissions.value) {
    map.set(p.group_key, [...(map.get(p.group_key) ?? []), p]);
  }
  return [...map.entries()];
});

// Non-owners can't grant what they don't hold (Rust enforces this too)
const mayGrant = (key: string) => auth.can(key);

// ----------------------------------------------------------- edit sheet

const open = ref(false);
const editing = ref<Role | null>(null);
const form = ref({ name_en: "", name_ku: "", name_ar: "", description: "" });
const selected = ref(new Set<string>());
const saving = ref(false);
const error = ref("");
const confirmDelete = ref(false);
const reassignTo = ref("");

const reset = () => {
  error.value = "";
  confirmDelete.value = false;
  reassignTo.value = "";
};

const startCreate = () => {
  editing.value = null;
  form.value = { name_en: "", name_ku: "", name_ar: "", description: "" };
  selected.value = new Set();
  reset();
  open.value = true;
};

const startEdit = async (r: Role, duplicate = false) => {
  editing.value = duplicate ? null : r;
  const copy = (s: string | null) => (duplicate && s ? t("staff.roles.copyOf", { name: s }) : s ?? "");
  form.value = {
    name_en: copy(r.name_en),
    name_ku: copy(r.name_ku),
    name_ar: copy(r.name_ar),
    description: r.description ?? "",
  };
  selected.value = new Set(
    r.is_system ? permissions.value.map((p) => p.key) : await listRolePermissions(r.id),
  );
  reset();
  open.value = true;
};

const toggle = (key: string, on: boolean) => {
  const next = new Set(selected.value);
  if (on) next.add(key);
  else next.delete(key);
  selected.value = next;
};

const groupState = (items: Permission[]) => {
  const n = items.filter((p) => selected.value.has(p.key)).length;
  return n === 0 ? "none" : n === items.length ? "all" : "some";
};

const toggleGroup = (items: Permission[]) => {
  const on = groupState(items) !== "all";
  const next = new Set(selected.value);
  for (const p of items) {
    if (!mayGrant(p.key)) continue;
    if (on) next.add(p.key);
    else next.delete(p.key);
  }
  selected.value = next;
};

const save = async () => {
  const role = { ...form.value, permissions: [...selected.value] };
  saving.value = true;
  error.value = "";
  try {
    if (editing.value) await call("update_role", { id: editing.value.id, role });
    else await call("create_role", role);
    open.value = false;
    await Promise.all([load(), auth.refresh()]);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};

const remove = async () => {
  if (!editing.value) return;
  saving.value = true;
  error.value = "";
  try {
    await call("delete_role", {
      id: editing.value.id,
      reassignTo: reassignTo.value ? Number(reassignTo.value) : null,
    });
    open.value = false;
    await load();
  } catch (e) {
    error.value = isAppError(e) && e.code === "role_in_use" ? "" : errorMessage(e);
  } finally {
    saving.value = false;
  }
};

const otherRoles = computed(() => roles.value.filter((r) => r.id !== editing.value?.id));
const isSystem = computed(() => !!editing.value?.is_system);
</script>

<template>
  <div class="space-y-5">
    <div class="flex items-center">
      <p class="max-w-xl text-sm text-muted-foreground">{{ $t("staff.roles.intro") }}</p>
      <Button size="lg" class="ms-auto" @click="startCreate">
        <Plus />
        {{ $t("settings.roles.addRole") }}
      </Button>
    </div>

    <ul class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
      <li v-for="r in roles" :key="r.id" class="flex flex-col rounded-xl border bg-card p-5">
        <div class="flex items-start gap-2">
          <h3 class="flex-1 font-semibold">{{ localized(r, "name") }}</h3>
          <span
            v-if="r.is_system"
            class="inline-flex items-center gap-1 rounded-md bg-sun/20 px-2 py-0.5 text-xs font-medium"
          >
            <Lock :size="12" /> {{ $t("staff.roles.system") }}
          </span>
        </div>
        <p class="mt-1 line-clamp-2 min-h-10 text-sm text-muted-foreground">
          {{ r.is_system ? $t("staff.roles.ownerHint") : r.description || "—" }}
        </p>
        <dl class="mt-3 flex gap-6 text-sm">
          <div>
            <dt class="text-xs text-muted-foreground">{{ $t("staff.roles.people") }}</dt>
            <dd class="font-medium">{{ r.user_count }}</dd>
          </div>
          <div>
            <dt class="text-xs text-muted-foreground">{{ $t("staff.roles.permissions") }}</dt>
            <dd class="font-medium">
              {{ r.is_system ? $t("staff.roles.allPermissions") : counts[r.id] ?? 0 }}
            </dd>
          </div>
        </dl>
        <div class="mt-4 flex gap-2">
          <Button v-if="!r.is_system" variant="outline" size="sm" @click="startEdit(r)">
            <Pencil /> {{ $t("settings.edit") }}
          </Button>
          <Button variant="ghost" size="sm" @click="startEdit(r, true)">
            <Copy /> {{ $t("staff.roles.duplicate") }}
          </Button>
        </div>
      </li>
    </ul>

    <EditSheet
      v-model:open="open"
      :title="editing ? $t('staff.roles.edit') : $t('settings.roles.addRole')"
      @submit="save"
    >
      <div class="grid gap-4 sm:grid-cols-3">
        <div class="space-y-2">
          <Label for="r-en">{{ $t("staff.roles.nameEn") }}</Label>
          <Input id="r-en" v-model="form.name_en" required lang="en" dir="ltr" :disabled="isSystem" />
        </div>
        <div class="space-y-2">
          <Label for="r-ku">{{ $t("staff.roles.nameKu") }}</Label>
          <Input id="r-ku" v-model="form.name_ku" required lang="ku" dir="rtl" :disabled="isSystem" />
        </div>
        <div class="space-y-2">
          <Label for="r-ar">{{ $t("staff.roles.nameAr") }}</Label>
          <Input id="r-ar" v-model="form.name_ar" lang="ar" dir="rtl" :disabled="isSystem" />
        </div>
      </div>
      <div class="space-y-2">
        <Label for="r-desc">{{ $t("settings.roles.description") }}</Label>
        <Input id="r-desc" v-model="form.description" :disabled="isSystem" />
      </div>

      <div class="space-y-4">
        <h3 class="text-sm font-semibold">{{ $t("staff.roles.permissions") }}</h3>
        <fieldset
          v-for="[group, items] in groups"
          :key="group"
          class="rounded-lg border"
          :disabled="isSystem"
        >
          <legend class="sr-only">{{ $t(`permissionGroups.${group}`) }}</legend>
          <label class="flex cursor-pointer items-center gap-3 border-b bg-muted/50 px-4 py-2.5">
            <input
              type="checkbox"
              class="size-4 accent-primary"
              :checked="groupState(items) === 'all'"
              :indeterminate="groupState(items) === 'some'"
              @change="toggleGroup(items)"
            />
            <span class="text-sm font-semibold">{{ $t(`permissionGroups.${group}`) }}</span>
          </label>
          <ul class="divide-y">
            <li v-for="p in items" :key="p.key">
              <label
                class="flex items-center gap-3 px-4 py-2.5 text-sm"
                :class="mayGrant(p.key) ? 'cursor-pointer' : 'opacity-50'"
              >
                <input
                  type="checkbox"
                  class="size-4 accent-primary"
                  :checked="selected.has(p.key)"
                  :disabled="!mayGrant(p.key)"
                  @change="toggle(p.key, ($event.target as HTMLInputElement).checked)"
                />
                <span class="flex-1">{{ $t(`permissions.${p.key}`) }}</span>
                <span
                  v-if="p.is_dangerous"
                  class="inline-flex items-center gap-1 text-xs font-medium text-warning"
                >
                  <ShieldAlert :size="14" /> {{ $t("staff.roles.sensitive") }}
                </span>
              </label>
            </li>
          </ul>
        </fieldset>
      </div>

      <div v-if="confirmDelete && editing" class="space-y-3 rounded-lg border border-destructive/40 bg-destructive/5 p-4">
        <p class="text-sm">
          {{
            editing.user_count
              ? $t("staff.roles.reassignPrompt", editing.user_count)
              : $t("staff.roles.deletePrompt")
          }}
        </p>
        <Select v-if="editing.user_count" v-model="reassignTo" :key="$i18n.locale">
          <SelectTrigger class="w-full"><SelectValue :placeholder="$t('staff.roles.pickRole')" /></SelectTrigger>
          <SelectContent>
            <SelectItem v-for="r in otherRoles" :key="r.id" :value="String(r.id)">
              {{ localized(r, "name") }}
            </SelectItem>
          </SelectContent>
        </Select>
        <div class="flex gap-2">
          <Button
            type="button"
            variant="destructive"
            :disabled="saving || (editing.user_count > 0 && !reassignTo)"
            @click="remove"
          >
            {{ $t("settings.delete") }}
          </Button>
          <Button type="button" variant="ghost" @click="confirmDelete = false">
            {{ $t("staff.cancel") }}
          </Button>
        </div>
      </div>

      <template #footer>
        <Button v-if="!isSystem" type="submit" :disabled="saving">
          <Loader2 v-if="saving" class="animate-spin" />
          {{ editing ? $t("common.save") : $t("settings.roles.addRole") }}
        </Button>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <Button
          v-if="editing && !isSystem && !confirmDelete"
          type="button"
          variant="ghost"
          class="ms-auto text-muted-foreground hover:text-destructive"
          @click="confirmDelete = true"
        >
          <Trash2 /> {{ $t("settings.delete") }}
        </Button>
      </template>
    </EditSheet>
  </div>
</template>
