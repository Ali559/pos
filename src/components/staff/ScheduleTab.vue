<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronLeft, ChevronRight, CopyPlus, Loader2, Plus, Trash2 } from "lucide-vue-next";
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
import EditSheet from "./EditSheet.vue";
import { listEmployees, listSchedules, type Employee, type Schedule } from "@/db/staff";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import { formatDay, formatTime } from "@/lib/locale";
import { useAuth } from "@/stores/auth";

const { t } = useI18n();
const auth = useAuth();
const canEdit = computed(() => auth.can("staff.shifts"));

const DAY_MS = 86_400_000;
// Iraqi work weeks start on Saturday
const WEEK_START_DAY = 6;

const startOfWeek = (d: Date) => {
  const x = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  x.setDate(x.getDate() - ((x.getDay() - WEEK_START_DAY + 7) % 7));
  return x;
};
const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
const sameDay = (a: Date, b: Date) => a.toDateString() === b.toDateString();
const pad = (n: number) => String(n).padStart(2, "0");
const dateInput = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const timeInput = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;

const weekStart = ref(startOfWeek(new Date()));
const days = computed(() => Array.from({ length: 7 }, (_, i) => addDays(weekStart.value, i)));
const employees = ref<Employee[]>([]);
const schedules = ref<Schedule[]>([]);
const notice = ref("");

const load = async () => {
  const from = weekStart.value.toISOString();
  const to = addDays(weekStart.value, 7).toISOString();
  [employees.value, schedules.value] = await Promise.all([
    listEmployees().then((list) => list.filter((e) => e.is_active)),
    listSchedules(from, to),
  ]);
};
onMounted(load);
watch(weekStart, () => {
  notice.value = "";
  load();
});

const shiftsFor = (userId: number, day: Date) =>
  schedules.value.filter((s) => s.user_id === userId && sameDay(new Date(s.starts_at), day));

const label = (s: Schedule) =>
  `${formatTime(new Date(s.starts_at))}–${formatTime(new Date(s.ends_at))}`;

const hoursFor = (userId: number) =>
  schedules.value
    .filter((s) => s.user_id === userId)
    .reduce((h, s) => h + (Date.parse(s.ends_at) - Date.parse(s.starts_at)) / 3_600_000, 0);

const weekLabel = computed(
  () => `${formatDay(days.value[0])} – ${formatDay(days.value[6])}`,
);

const copyPreviousWeek = async () => {
  notice.value = "";
  try {
    const r = await call<{ copied: number; skipped: number }>("copy_schedule_week", {
      fromStart: addDays(weekStart.value, -7).toISOString(),
      toStart: weekStart.value.toISOString(),
    });
    notice.value = t("schedule.copied", { copied: r.copied, skipped: r.skipped });
    await load();
  } catch (e) {
    notice.value = errorMessage(e);
  }
};

// ----------------------------------------------------------- edit sheet

const open = ref(false);
const editing = ref<Schedule | null>(null);
const form = ref({ userId: "", date: "", start: "09:00", end: "17:00", note: "" });
const saving = ref(false);
const error = ref("");

const startCreate = (userId: number, day: Date) => {
  editing.value = null;
  form.value = { userId: String(userId), date: dateInput(day), start: "09:00", end: "17:00", note: "" };
  error.value = "";
  open.value = true;
};

const startEdit = (s: Schedule) => {
  editing.value = s;
  const start = new Date(s.starts_at);
  form.value = {
    userId: String(s.user_id),
    date: dateInput(start),
    start: timeInput(start),
    end: timeInput(new Date(s.ends_at)),
    note: s.note ?? "",
  };
  error.value = "";
  open.value = true;
};

// An end time at or before the start means the shift runs past midnight
const overnight = computed(() => form.value.end <= form.value.start);

const save = async () => {
  const f = form.value;
  const start = new Date(`${f.date}T${f.start}`);
  let end = new Date(`${f.date}T${f.end}`);
  if (end <= start) end = new Date(end.getTime() + DAY_MS);
  saving.value = true;
  error.value = "";
  try {
    await call("save_schedule", {
      id: editing.value?.id ?? null,
      userId: Number(f.userId),
      startsAt: start.toISOString(),
      endsAt: end.toISOString(),
      note: f.note || null,
    });
    open.value = false;
    await load();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};

const remove = async () => {
  if (!editing.value) return;
  try {
    await call("delete_schedule", { id: editing.value.id });
    open.value = false;
    await load();
  } catch (e) {
    error.value = errorMessage(e);
  }
};
</script>

<template>
  <div class="space-y-5">
    <div class="flex flex-wrap items-center gap-3">
      <div class="flex items-center gap-1">
        <Button variant="outline" size="icon" :aria-label="$t('schedule.previousWeek')" @click="weekStart = addDays(weekStart, -7)">
          <ChevronLeft class="rtl:rotate-180" />
        </Button>
        <Button variant="outline" size="icon" :aria-label="$t('schedule.nextWeek')" @click="weekStart = addDays(weekStart, 7)">
          <ChevronRight class="rtl:rotate-180" />
        </Button>
      </div>
      <h2 class="font-semibold">{{ weekLabel }}</h2>
      <Button variant="ghost" size="sm" @click="weekStart = startOfWeek(new Date())">
        {{ $t("schedule.thisWeek") }}
      </Button>
      <Button v-if="canEdit" variant="outline" class="ms-auto" @click="copyPreviousWeek">
        <CopyPlus /> {{ $t("schedule.copyPrevious") }}
      </Button>
    </div>
    <p v-if="notice" role="status" class="text-sm text-muted-foreground">{{ notice }}</p>

    <div class="overflow-x-auto rounded-xl border bg-card">
      <div class="grid min-w-[880px] grid-cols-[180px_repeat(7,minmax(0,1fr))]">
        <div class="border-b p-3 text-xs font-medium text-muted-foreground">
          {{ $t("staff.fields.fullName") }}
        </div>
        <div
          v-for="d in days"
          :key="d.toISOString()"
          class="border-s border-b p-3 text-xs font-medium"
          :class="sameDay(d, new Date()) ? 'bg-sun/15 text-foreground' : 'text-muted-foreground'"
        >
          {{ formatDay(d) }}
        </div>

        <template v-for="e in employees" :key="e.id">
          <div class="border-b p-3">
            <div class="truncate text-sm font-medium">{{ e.full_name }}</div>
            <div class="text-xs text-muted-foreground">
              {{ $t("schedule.hours", { h: Math.round(hoursFor(e.id) * 10) / 10 }) }}
            </div>
          </div>
          <div
            v-for="d in days"
            :key="d.toISOString()"
            class="group flex min-h-16 flex-col gap-1 border-s border-b p-1.5"
            :class="sameDay(d, new Date()) && 'bg-sun/5'"
          >
            <button
              v-for="s in shiftsFor(e.id, d)"
              :key="s.id"
              type="button"
              class="num rounded-md bg-primary/10 px-2 py-1 text-start text-xs font-medium text-primary hover:bg-primary/20 disabled:cursor-default"
              :disabled="!canEdit"
              :title="s.note ?? undefined"
              dir="ltr"
              @click="startEdit(s)"
            >
              {{ label(s) }}
            </button>
            <button
              v-if="canEdit"
              type="button"
              class="grid flex-1 place-items-center rounded-md text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:bg-muted focus-visible:opacity-100"
              :aria-label="$t('schedule.addShift')"
              @click="startCreate(e.id, d)"
            >
              <Plus :size="16" />
            </button>
          </div>
        </template>

        <p v-if="employees.length === 0" class="col-span-8 p-10 text-center text-muted-foreground">
          {{ $t("staff.noResults") }}
        </p>
      </div>
    </div>

    <EditSheet
      v-model:open="open"
      :title="editing ? $t('schedule.editShift') : $t('schedule.addShift')"
      @submit="save"
    >
      <div class="space-y-2">
        <Label>{{ $t("staff.employee") }}</Label>
        <Select v-model="form.userId" :key="$i18n.locale">
          <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem v-for="e in employees" :key="e.id" :value="String(e.id)">
              {{ e.full_name }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div class="grid gap-4 sm:grid-cols-3">
        <div class="space-y-2">
          <Label for="s-date">{{ $t("schedule.date") }}</Label>
          <Input id="s-date" v-model="form.date" type="date" required dir="ltr" />
        </div>
        <div class="space-y-2">
          <Label for="s-start">{{ $t("schedule.start") }}</Label>
          <Input id="s-start" v-model="form.start" type="time" required dir="ltr" />
        </div>
        <div class="space-y-2">
          <Label for="s-end">{{ $t("schedule.end") }}</Label>
          <Input id="s-end" v-model="form.end" type="time" required dir="ltr" />
        </div>
      </div>
      <p v-if="overnight" class="text-sm text-muted-foreground">{{ $t("schedule.overnight") }}</p>
      <div class="space-y-2">
        <Label for="s-note">
          {{ $t("schedule.note") }}
          <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
        </Label>
        <Input id="s-note" v-model="form.note" />
      </div>

      <template #footer>
        <Button type="submit" :disabled="saving">
          <Loader2 v-if="saving" class="animate-spin" />
          {{ $t("common.save") }}
        </Button>
        <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
        <Button
          v-if="editing"
          type="button"
          variant="ghost"
          class="ms-auto text-muted-foreground hover:text-destructive"
          @click="remove"
        >
          <Trash2 /> {{ $t("settings.delete") }}
        </Button>
      </template>
    </EditSheet>
  </div>
</template>
