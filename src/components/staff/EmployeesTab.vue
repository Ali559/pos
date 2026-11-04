<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { KeyRound, Loader2, Pencil, Plus, Search } from "lucide-vue-next";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import EditSheet from "./EditSheet.vue";
import { listEmployees, listRoles, type Employee, type Role } from "@/db/staff";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatDateTime } from "@/lib/locale";
import { useAuth } from "@/stores/auth";
import { localized } from "@/utils/i18n";

const { t } = useI18n();
const auth = useAuth();
const canEdit = computed(() => auth.can("staff.edit"));

const employees = ref<Employee[]>([]);
const roles = ref<Role[]>([]);
const search = ref("");
const roleFilter = ref("all");

const load = async () => {
  [employees.value, roles.value] = await Promise.all([listEmployees(), listRoles()]);
};
onMounted(load);

const visible = computed(() => {
  const q = search.value.trim().toLowerCase();
  return employees.value.filter(
    (e) =>
      (roleFilter.value === "all" || e.role_id === Number(roleFilter.value)) &&
      (!q ||
        e.full_name.toLowerCase().includes(q) ||
        e.username.toLowerCase().includes(q) ||
        e.phone?.includes(q)),
  );
});

// Only an owner may hand out the Owner role (Rust enforces this too)
const assignableRoles = computed(() =>
  roles.value.filter((r) => !r.is_system || auth.session.value?.isOwner),
);

// ----------------------------------------------------------- edit sheet

interface Form {
  fullName: string;
  username: string;
  roleId: string;
  phone: string;
  hireDate: string;
  wageAmount: string;
  wagePeriod: "hour" | "month";
  isActive: boolean;
  password: string;
  pinMode: "keep" | "set" | "remove";
  pin: string;
}

const open = ref(false);
const editing = ref<Employee | null>(null);
const form = ref<Form>(blank());
const saving = ref(false);
const error = ref("");

function blank(): Form {
  return {
    fullName: "", username: "", roleId: "", phone: "", hireDate: "",
    wageAmount: "", wagePeriod: "month", isActive: true, password: "",
    pinMode: "set", pin: "",
  };
}

const startCreate = () => {
  editing.value = null;
  form.value = blank();
  form.value.roleId = String(assignableRoles.value[assignableRoles.value.length - 1]?.id ?? "");
  error.value = "";
  open.value = true;
};

const startEdit = (e: Employee) => {
  editing.value = e;
  form.value = {
    fullName: e.full_name,
    username: e.username,
    roleId: String(e.role_id),
    phone: e.phone ?? "",
    hireDate: e.hire_date ?? "",
    wageAmount: e.wage_amount != null ? String(e.wage_amount) : "",
    wagePeriod: e.wage_period ?? "month",
    isActive: !!e.is_active,
    password: "",
    pinMode: "keep",
    pin: "",
  };
  error.value = "";
  open.value = true;
};

const save = async () => {
  const f = form.value;
  const user = {
    fullName: f.fullName,
    username: f.username,
    roleId: Number(f.roleId),
    phone: f.phone || null,
    photoMediaId: editing.value?.photo_media_id ?? null,
    hireDate: f.hireDate || null,
    wageAmount: f.wageAmount === "" ? null : Number(f.wageAmount),
    wagePeriod: f.wageAmount === "" ? null : f.wagePeriod,
    isActive: f.isActive,
    password: f.password || null,
    // null keeps the current PIN, "" removes it; an empty field never removes
    pin: f.pinMode === "remove" ? "" : f.pinMode === "set" && f.pin ? f.pin : null,
  };
  saving.value = true;
  error.value = "";
  try {
    if (editing.value) await call("update_user", { id: editing.value.id, user });
    else await call("create_user", user);
    open.value = false;
    await load();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};

const lastLogin = (iso: string | null) => (iso ? formatDateTime(new Date(iso)) : t("staff.never"));
</script>

<template>
  <div class="space-y-5">
    <div class="flex flex-wrap items-center gap-3">
      <div class="relative min-w-64 flex-1">
        <Search
          :size="18"
          class="pointer-events-none absolute start-3 top-1/2 -translate-y-1/2 text-muted-foreground"
        />
        <input
          v-model="search"
          type="search"
          :placeholder="$t('staff.searchPlaceholder')"
          :aria-label="$t('staff.searchPlaceholder')"
          class="h-10 w-full rounded-md border border-input bg-card ps-10 pe-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        />
      </div>
      <Select v-model="roleFilter" :key="$i18n.locale">
        <SelectTrigger class="w-48"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem value="all">{{ $t("staff.allRoles") }}</SelectItem>
          <SelectItem v-for="r in roles" :key="r.id" :value="String(r.id)">
            {{ localized(r, "name") }}
          </SelectItem>
        </SelectContent>
      </Select>
      <Button v-if="canEdit" size="lg" class="ms-auto" @click="startCreate">
        <Plus />
        {{ $t("staff.addEmployee") }}
      </Button>
    </div>

    <div class="overflow-hidden rounded-xl border bg-card">
      <Table>
        <TableHeader>
          <TableRow class="hover:bg-transparent">
            <TableHead class="ps-5">{{ $t("staff.fields.fullName") }}</TableHead>
            <TableHead>{{ $t("staff.fields.role") }}</TableHead>
            <TableHead>{{ $t("staff.fields.phone") }}</TableHead>
            <TableHead>{{ $t("staff.lastLogin") }}</TableHead>
            <TableHead>{{ $t("staff.fields.status") }}</TableHead>
            <TableHead class="w-14 pe-5"><span class="sr-only">{{ $t("settings.actions") }}</span></TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-if="visible.length === 0" class="hover:bg-transparent">
            <TableCell colspan="6" class="py-14 text-center text-muted-foreground">
              {{ $t("staff.noResults") }}
            </TableCell>
          </TableRow>
          <TableRow v-for="e in visible" :key="e.id" :class="!e.is_active && 'opacity-60'">
            <TableCell class="ps-5">
              <div class="font-medium">{{ e.full_name }}</div>
              <div class="flex items-center gap-1.5 text-xs text-muted-foreground" dir="ltr">
                <span>@{{ e.username }}</span>
                <KeyRound v-if="e.has_pin" :size="12" :aria-label="$t('staff.hasPin')" />
              </div>
            </TableCell>
            <TableCell>{{ localized(e, "role_name") }}</TableCell>
            <TableCell class="num" dir="ltr">{{ e.phone ?? "—" }}</TableCell>
            <TableCell class="text-muted-foreground">{{ lastLogin(e.last_login) }}</TableCell>
            <TableCell>
              <span
                class="inline-flex h-6 items-center rounded-md px-2 text-xs font-semibold"
                :class="e.is_active ? 'bg-success/12 text-success' : 'bg-muted text-muted-foreground'"
              >
                {{ e.is_active ? $t("settings.users.active") : $t("settings.users.inactive") }}
              </span>
            </TableCell>
            <TableCell class="pe-5">
              <Button
                v-if="canEdit"
                variant="ghost"
                size="icon-sm"
                :aria-label="$t('staff.editEmployee')"
                @click="startEdit(e)"
              >
                <Pencil />
              </Button>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>

    <EditSheet
      v-model:open="open"
      :title="editing ? $t('staff.editEmployee') : $t('staff.addEmployee')"
      @submit="save"
    >
      <div class="space-y-2">
        <Label for="e-name">{{ $t("staff.fields.fullName") }}</Label>
        <Input id="e-name" v-model="form.fullName" required />
      </div>
      <div class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label for="e-username">{{ $t("login.username") }}</Label>
          <Input id="e-username" v-model="form.username" required dir="ltr" autocapitalize="off" />
        </div>
        <div class="space-y-2">
          <Label>{{ $t("staff.fields.role") }}</Label>
          <Select v-model="form.roleId" :key="$i18n.locale">
            <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem v-for="r in assignableRoles" :key="r.id" :value="String(r.id)">
                {{ localized(r, "name") }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="space-y-2">
          <Label for="e-phone">{{ $t("staff.fields.phone") }}</Label>
          <Input id="e-phone" v-model="form.phone" type="tel" dir="ltr" />
        </div>
        <div class="space-y-2">
          <Label for="e-hire">{{ $t("staff.fields.hireDate") }}</Label>
          <Input id="e-hire" v-model="form.hireDate" type="date" dir="ltr" />
        </div>
        <div class="space-y-2">
          <Label for="e-wage">
            {{ $t("staff.fields.wage") }}
            <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
          </Label>
          <Input id="e-wage" v-model="form.wageAmount" type="number" min="0" step="any" dir="ltr" />
        </div>
        <div class="space-y-2">
          <Label>{{ $t("staff.fields.wagePeriod") }}</Label>
          <Select v-model="form.wagePeriod" :key="$i18n.locale" :disabled="form.wageAmount === ''">
            <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem value="hour">{{ $t("staff.wagePeriods.hour") }}</SelectItem>
              <SelectItem value="month">{{ $t("staff.wagePeriods.month") }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <fieldset class="space-y-4 rounded-lg border p-4">
        <legend class="px-1 text-sm font-semibold">{{ $t("staff.signIn") }}</legend>
        <div class="space-y-2">
          <Label for="e-password">
            {{ editing ? $t("staff.fields.newPassword") : $t("login.password") }}
          </Label>
          <Input
            id="e-password"
            v-model="form.password"
            type="password"
            dir="ltr"
            autocomplete="new-password"
            :required="!editing"
            :placeholder="editing ? $t('staff.keepPassword') : ''"
          />
          <p class="text-xs text-muted-foreground">{{ $t("staff.passwordHint") }}</p>
        </div>
        <div class="space-y-2">
          <Label for="e-pin">{{ $t("staff.fields.pin") }}</Label>
          <div v-if="editing && editing.has_pin && form.pinMode !== 'set'" class="flex items-center gap-2 text-sm">
            <span class="flex-1 text-muted-foreground">
              {{ form.pinMode === "remove" ? $t("staff.pinWillBeRemoved") : $t("staff.pinIsSet") }}
            </span>
            <Button type="button" variant="outline" size="sm" @click="form.pinMode = 'set'">
              {{ $t("common.change") }}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              class="text-muted-foreground"
              @click="form.pinMode = form.pinMode === 'remove' ? 'keep' : 'remove'"
            >
              {{ form.pinMode === "remove" ? $t("staff.undo") : $t("common.remove") }}
            </Button>
          </div>
          <template v-else>
            <Input
              id="e-pin"
              v-model="form.pin"
              type="password"
              inputmode="numeric"
              pattern="[0-9]{4,6}"
              maxlength="6"
              dir="ltr"
              autocomplete="off"
              @focus="form.pinMode = 'set'"
            />
            <p class="text-xs text-muted-foreground">{{ $t("staff.pinHint") }}</p>
          </template>
        </div>
      </fieldset>

      <label class="flex items-center gap-3 text-sm">
        <input v-model="form.isActive" type="checkbox" class="size-4 accent-primary" />
        <span>
          <span class="font-medium">{{ $t("settings.users.active") }}</span>
          <span class="block text-muted-foreground">{{ $t("staff.activeHint") }}</span>
        </span>
      </label>

      <template #footer>
        <Button type="submit" :disabled="saving">
          <Loader2 v-if="saving" class="animate-spin" />
          {{ editing ? $t("common.save") : $t("staff.addEmployee") }}
        </Button>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      </template>
    </EditSheet>
  </div>
</template>
