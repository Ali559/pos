<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { DollarSign, RefreshCw } from "lucide-vue-next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { getSetting, setSetting } from "@/db/settings";
import { errorMessage } from "@/lib/errors";
import { formatCurrency, formatRate, formatUsd } from "@/lib/format";
import { formatDateTime } from "@/lib/locale";
import { useAuth } from "@/stores/auth";
import { RATE_CITIES, toIqd, useRate } from "@/stores/rate";

// Settings → Payments → US dollars. Switching dollars on and the drawer's
// dollar float need system settings; the rate itself, its source and city
// need `settings.rates` (managers).
const auth = useAuth();
const rate = useRate();
const canSystem = computed(() => auth.can("settings.system"));
const canRates = computed(() => auth.can("settings.rates"));

const typed = ref("");
const floatUsd = ref("");
const busy = ref(false);
const error = ref("");
const saved = ref(false);

onMounted(async () => {
  await rate.reload().catch((e) => (error.value = errorMessage(e)));
  floatUsd.value = await getSetting("opening_float_usd").then((v) => v ?? "0").catch(() => "0");
});

const run = async (f: () => Promise<unknown>) => {
  busy.value = true;
  error.value = "";
  try {
    await f();
    await rate.reload();
    saved.value = true;
    setTimeout(() => (saved.value = false), 2000);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
};

const status = computed(() => rate.status.value);
const sourceName = (source: string) => (source === "manual" ? "" : source.replace(/^telegram:/, "@"));
const when = (iso: string | null) => (iso ? formatDateTime(new Date(iso)) : "");
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="flex items-center gap-2"><DollarSign :size="18" /> {{ $t("rates.title") }}</CardTitle>
    </CardHeader>
    <CardContent v-if="status" class="space-y-5">
      <label class="flex items-start gap-3" :class="!canSystem && 'opacity-60'">
        <input
          type="checkbox"
          class="mt-0.5 size-4 accent-primary"
          :checked="status.enabled"
          :disabled="!canSystem || busy"
          @change="run(() => setSetting('usd_enabled', ($event.target as HTMLInputElement).checked ? '1' : '0'))"
        />
        <span>
          <span class="block text-sm font-medium">{{ $t("rates.enable") }}</span>
          <span class="block text-xs text-muted-foreground">{{ $t("rates.enableHint") }}</span>
        </span>
      </label>

      <template v-if="status.enabled">
        <div class="flex flex-wrap items-end gap-x-8 gap-y-3 rounded-lg bg-muted/60 p-4">
          <div>
            <p class="text-xs text-muted-foreground">{{ $t("rates.current") }}</p>
            <p v-if="status.current" class="num text-3xl font-bold" dir="ltr">{{ formatRate(status.current.per100) }}</p>
            <p v-else class="text-sm font-medium text-destructive">{{ $t("rates.noRateYet") }}</p>
            <p v-if="status.current" class="text-xs text-muted-foreground">
              {{ $t("rates.per100") }} · {{ $t("rates.oneDollar", { amount: formatCurrency(toIqd(1, status.current.per100)) }) }}
            </p>
          </div>
          <p v-if="status.current" class="text-xs text-muted-foreground">
            {{ status.current.city ? $t(`rates.cities.${status.current.city}`) + " · " : "" }}{{ sourceName(status.current.source) || $t("rates.typedBy", { name: status.current.setBy ?? "" }) }}
            <br />{{ when(status.current.postedAt ?? status.current.createdAt) }}
          </p>
        </div>

        <div v-if="status.offer && canRates" class="flex flex-wrap items-center gap-3 rounded-lg border border-sun/50 bg-sun/12 p-3 text-sm">
          <span class="flex-1">
            <span class="font-medium">{{ $t("rates.offer", { rate: formatRate(status.offer.per100) }) }}</span>
            <span class="block text-xs text-muted-foreground">{{ $t("rates.offerFrom", { city: $t(`rates.cities.${status.offer.city}`), time: when(status.offer.postedAt) }) }} · {{ sourceName(status.offer.source) }}</span>
          </span>
          <Button size="sm" :disabled="busy" @click="run(() => rate.accept(status!.offer!.per100))">{{ $t("rates.useIt") }}</Button>
          <Button size="sm" variant="ghost" :disabled="busy" @click="run(rate.dismiss)">{{ $t("rates.notNow") }}</Button>
        </div>

        <form v-if="canRates" class="flex flex-wrap items-end gap-3" @submit.prevent="run(() => rate.setRate(Number(typed))).then(() => (typed = ''))">
          <div class="space-y-2">
            <Label for="rate-typed">{{ $t("rates.typeLabel") }}</Label>
            <Input id="rate-typed" v-model="typed" type="number" min="1" step="any" dir="ltr" placeholder="169550" class="w-40" />
          </div>
          <Button type="submit" variant="outline" :disabled="busy || !(Number(typed) > 0)">{{ $t("rates.setRate") }}</Button>
          <p v-if="Number(typed) > 0" class="num pb-2 text-xs text-muted-foreground">{{ $t("rates.example", { usd: formatUsd(20), amount: formatCurrency(toIqd(20, Number(typed))) }) }}</p>
        </form>

        <div v-if="canRates" class="space-y-3 border-t pt-4">
          <label class="flex items-start gap-3">
            <input
              type="checkbox"
              class="mt-0.5 size-4 accent-primary"
              :checked="status.fetch"
              :disabled="busy"
              @change="run(() => setSetting('rate_fetch', ($event.target as HTMLInputElement).checked ? '1' : '0'))"
            />
            <span>
              <span class="block text-sm font-medium">{{ $t("rates.fetch") }}</span>
              <span class="block text-xs text-muted-foreground">{{ $t("rates.fetchHint") }}</span>
            </span>
          </label>
          <div v-if="status.fetch" class="flex flex-wrap items-end gap-3">
            <div class="space-y-2">
              <Label for="rate-city">{{ $t("rates.city") }}</Label>
              <select
                id="rate-city"
                :value="status.city"
                :disabled="busy"
                class="h-9 rounded-md border border-input bg-card px-2 text-sm"
                @change="run(() => setSetting('rate_city', ($event.target as HTMLSelectElement).value))"
              >
                <option v-for="c in RATE_CITIES" :key="c" :value="c">{{ $t(`rates.cities.${c}`) }}</option>
              </select>
            </div>
            <Button type="button" variant="outline" size="sm" :disabled="busy" @click="run(rate.fetchNow)"><RefreshCw :class="busy && 'animate-spin'" /> {{ $t("rates.checkNow") }}</Button>
            <p class="pb-1.5 text-xs text-muted-foreground">
              <template v-if="status.lastFetch">{{ $t("rates.lastCheck", { time: when(status.lastFetch) }) }}</template>
              <span v-if="status.lastError" class="block text-destructive" dir="auto">{{ $t("rates.fetchFailed") }}</span>
            </p>
          </div>
        </div>

        <div v-if="canSystem" class="flex flex-wrap items-end gap-3 border-t pt-4">
          <div class="space-y-2">
            <Label for="float-usd">{{ $t("rates.floatUsd") }}</Label>
            <Input id="float-usd" v-model="floatUsd" type="number" min="0" step="any" dir="ltr" class="w-32" />
          </div>
          <Button type="button" variant="outline" :disabled="busy" @click="run(() => setSetting('opening_float_usd', String(Math.max(0, Number(floatUsd) || 0))))">{{ $t("common.save") }}</Button>
        </div>

        <details v-if="status.history.length > 1" class="text-sm">
          <summary class="cursor-pointer text-muted-foreground">{{ $t("rates.history") }}</summary>
          <ul class="mt-2 divide-y text-xs">
            <li v-for="r in status.history" :key="r.id" class="flex justify-between gap-3 py-1.5">
              <span class="num font-medium" dir="ltr">{{ formatRate(r.per100) }}</span>
              <span class="text-muted-foreground">{{ r.city ? $t(`rates.cities.${r.city}`) + " · " : "" }}{{ sourceName(r.source) || r.setBy }}</span>
              <span class="text-muted-foreground">{{ when(r.createdAt) }}</span>
            </li>
          </ul>
        </details>
      </template>
      <p v-if="saved" class="text-sm text-success">{{ $t("common.saved") }}</p>
      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
    </CardContent>
  </Card>
</template>
