<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { Loader2, Monitor, Search } from "lucide-vue-next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import RangePicker, { resolveRange, type RangeModel } from "@/components/reports/RangePicker.vue";
import { ACTIVITY_GROUPS, listActivity, type AuditEntry } from "@/db/audit";
import { select } from "@/db/client";
import { describe } from "@/lib/activity";
import { errorMessage } from "@/lib/errors";
import { formatDate, formatTime } from "@/lib/locale";

// Settings → Activity: everything people did, newest first, in plain words.
const PAGE = 100;
const range = ref<RangeModel>({ preset: "thisWeek" });
const userId = ref("all");
const group = ref("all");
const search = ref("");
const entries = ref<AuditEntry[]>([]);
const users = ref<{ id: number; full_name: string }[]>([]);
const names = ref(new Map<number, string>());
const loading = ref(true);
const more = ref(false);
const error = ref("");

const load = async (append = false) => {
  loading.value = true;
  error.value = "";
  try {
    const rows = await listActivity({
      range: resolveRange(range.value),
      userId: userId.value === "all" ? null : Number(userId.value),
      group: group.value === "all" ? null : group.value,
      search: search.value,
      limit: PAGE + 1,
      offset: append ? entries.value.length : 0,
    });
    more.value = rows.length > PAGE;
    const page = rows.slice(0, PAGE);
    entries.value = append ? [...entries.value, ...page] : page;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loading.value = false;
  }
};

let timer: ReturnType<typeof setTimeout> | undefined;
watch([range, userId, group], () => load(), { deep: true });
watch(search, () => {
  clearTimeout(timer);
  timer = setTimeout(() => load(), 300);
});

onMounted(async () => {
  users.value = await select<{ id: number; full_name: string }>("SELECT id, full_name FROM users ORDER BY full_name COLLATE NOCASE").catch(() => []);
  names.value = new Map(users.value.map((u) => [u.id, u.full_name]));
  await load();
});

// A date line between days
const dayOf = (e: AuditEntry) => formatDate(new Date(e.created_at));
const newDay = (i: number) => i === 0 || dayOf(entries.value[i - 1]!) !== dayOf(entries.value[i]!);
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle>{{ $t("activity.title") }}</CardTitle>
      <p class="text-sm text-muted-foreground">{{ $t("activity.hint") }}</p>
    </CardHeader>
    <CardContent class="space-y-4">
      <div class="flex flex-wrap items-center gap-2">
        <RangePicker v-model="range" />
        <Select v-model="userId" :key="`u${$i18n.locale}`">
          <SelectTrigger class="w-44"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{{ $t("analytics.filters.allStaff") }}</SelectItem>
            <SelectItem v-for="u in users" :key="u.id" :value="String(u.id)">{{ u.full_name }}</SelectItem>
          </SelectContent>
        </Select>
        <Select v-model="group" :key="`g${$i18n.locale}`">
          <SelectTrigger class="w-44"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{{ $t("activity.groups.all") }}</SelectItem>
            <SelectItem v-for="g in Object.keys(ACTIVITY_GROUPS)" :key="g" :value="g">{{ $t(`activity.groups.${g}`) }}</SelectItem>
          </SelectContent>
        </Select>
        <div class="relative min-w-48 flex-1">
          <Search :size="16" class="absolute start-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <Input v-model="search" dir="auto" class="ps-9" :placeholder="$t('activity.search')" :aria-label="$t('activity.search')" />
        </div>
        <Loader2 v-if="loading" class="size-4 animate-spin text-muted-foreground" />
      </div>

      <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
      <p v-else-if="!loading && !entries.length" class="py-10 text-center text-sm text-muted-foreground">{{ $t("activity.empty") }}</p>

      <ol v-else class="space-y-0.5">
        <template v-for="(e, i) in entries" :key="e.id">
          <li v-if="newDay(i)" class="pt-3 pb-1 text-xs font-semibold text-muted-foreground first:pt-0">{{ dayOf(e) }}</li>
          <li class="flex gap-3 rounded-lg px-2 py-2 hover:bg-muted/60">
            <span class="num w-12 shrink-0 pt-px text-sm text-muted-foreground">{{ formatTime(new Date(e.created_at)) }}</span>
            <div class="min-w-0 flex-1">
              <p class="text-sm">{{ describe(e, names) }}</p>
              <p class="flex flex-wrap items-center gap-x-2 text-xs text-muted-foreground">
                <bdi>{{ e.user_name ?? $t("activity.system") }}</bdi>
                <span v-if="e.till_name" class="flex items-center gap-1"><Monitor :size="12" /><bdi>{{ e.till_name }}</bdi></span>
              </p>
            </div>
          </li>
        </template>
      </ol>
      <div v-if="more" class="flex justify-center">
        <Button variant="outline" :disabled="loading" @click="load(true)">{{ $t("activity.more") }}</Button>
      </div>
    </CardContent>
  </Card>
</template>
