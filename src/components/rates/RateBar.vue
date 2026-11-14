<script setup lang="ts">
import { computed, ref } from "vue";
import { DollarSign } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { errorMessage } from "@/lib/errors";
import { formatRate } from "@/lib/format";
import { formatDateTime } from "@/lib/locale";
import { useAuth } from "@/stores/auth";
import { useRate } from "@/stores/rate";

// For people who set the rate: a newer market rate read from Telegram
// (use it or not), or a reminder that dollars are on but no rate is set.
// Nobody else sees anything here.
const auth = useAuth();
const rate = useRate();
const busy = ref(false);
const error = ref("");
const typing = ref(false);
const typed = ref("");

const canSet = computed(() => auth.can("settings.rates"));
const show = computed(() => canSet.value && rate.enabled.value && (rate.offer.value || !rate.per100.value));

const run = async (f: () => Promise<void>) => {
  busy.value = true;
  error.value = "";
  try {
    await f();
    typing.value = false;
  } catch (e) {
    error.value = errorMessage(e);
    await rate.reload().catch(() => {});
  } finally {
    busy.value = false;
  }
};
</script>

<template>
  <div v-if="show" role="status" class="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border border-sun/50 bg-sun/12 px-4 py-3 text-sm">
    <DollarSign :size="18" class="shrink-0 text-amber-700 dark:text-amber-400" />
    <template v-if="rate.offer.value && !typing">
      <span class="min-w-0 flex-1">
        <span class="font-medium">{{ $t("rates.offer", { rate: formatRate(rate.offer.value.per100) }) }}</span>
        <span class="block text-xs text-muted-foreground">
          {{ $t("rates.offerFrom", { city: $t(`rates.cities.${rate.offer.value.city}`), time: formatDateTime(new Date(rate.offer.value.postedAt)) }) }}
          <template v-if="rate.per100.value"> · {{ $t("rates.inUse", { rate: formatRate(rate.per100.value) }) }}</template>
        </span>
      </span>
      <Button size="sm" :disabled="busy" @click="run(() => rate.accept(rate.offer.value!.per100))">{{ $t("rates.useIt") }}</Button>
      <Button size="sm" variant="outline" :disabled="busy" @click="typing = true">{{ $t("rates.typeOne") }}</Button>
      <Button size="sm" variant="ghost" :disabled="busy" @click="run(rate.dismiss)">{{ $t("rates.notNow") }}</Button>
    </template>
    <form v-else class="flex min-w-0 flex-1 flex-wrap items-center gap-2" @submit.prevent="run(() => rate.setRate(Number(typed)))">
      <label for="rate-bar-input" class="font-medium">{{ rate.per100.value ? $t("rates.typeLabel") : $t("rates.noRate") }}</label>
      <input
        id="rate-bar-input"
        v-model="typed"
        type="number"
        min="1"
        step="any"
        dir="ltr"
        placeholder="169550"
        class="num h-9 w-32 rounded-md border border-input bg-card px-2 outline-none focus:border-primary"
      />
      <span class="text-xs text-muted-foreground">{{ $t("rates.per100") }}</span>
      <Button type="submit" size="sm" :disabled="busy || !(Number(typed) > 0)">{{ $t("common.save") }}</Button>
      <Button v-if="typing" type="button" size="sm" variant="ghost" @click="typing = false">{{ $t("common.cancel") }}</Button>
    </form>
    <p v-if="error" role="alert" class="w-full text-destructive">{{ error }}</p>
  </div>
</template>
