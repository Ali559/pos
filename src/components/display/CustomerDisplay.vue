<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { subscribe, type DisplayState } from "@/lib/customerDisplay";

// What the customer sees, on its own window. Everything comes from the
// till as `display:state` events; this screen holds no data of its own.
const { t, locale } = useI18n();
const state = ref<DisplayState | null>(null);
const list = ref<HTMLElement | null>(null);

const money = (n: number) => {
  if (!state.value) return String(n);
  const lang = state.value.locale === "ku" ? "ar-IQ" : state.value.locale === "ar" ? "ar-IQ" : "en-US";
  return new Intl.NumberFormat(lang, {
    style: "currency",
    currency: state.value.currency,
    maximumFractionDigits: state.value.currency === "IQD" ? 0 : 2,
    numberingSystem: "latn",
  } as Intl.NumberFormatOptions).format(n);
};

onMounted(async () => {
  // Hears every change from the till, after asking for what's on screen now
  await subscribe((s) => {
    state.value = s;
    locale.value = s.locale;
    document.documentElement.dir = s.dir;
    document.documentElement.lang = s.locale;
  });
});

// Keep the newest line in view
watch(
  () => state.value?.lines.length,
  () => nextTick(() => list.value?.scrollTo({ top: list.value.scrollHeight, behavior: "smooth" })),
);

const saved = computed(() => state.value?.discount ?? 0);
</script>

<template>
  <div class="fixed inset-0 flex bg-background text-foreground select-none" :dir="state?.dir ?? 'ltr'">
    <!-- Nothing yet / between customers -->
    <div v-if="!state || state.mode === 'idle'" class="m-auto flex flex-col items-center gap-6 text-center">
      <img v-if="state?.logo" :src="state.logo" alt="" class="max-h-40 max-w-80 object-contain" />
      <p class="text-6xl font-bold">{{ state?.shopName }}</p>
      <p class="text-3xl text-muted-foreground">{{ t("display.welcome") }}</p>
    </div>

    <!-- Thank you, with the change -->
    <div v-else-if="state.mode === 'done'" class="m-auto flex flex-col items-center gap-6 text-center">
      <p class="text-5xl font-bold">{{ t("display.thanks") }}</p>
      <p class="text-3xl text-muted-foreground">{{ t("display.total") }} <span class="num font-semibold text-foreground">{{ money(state.total) }}</span></p>
      <div v-if="state.change" class="rounded-3xl bg-success/12 px-16 py-8 text-success">
        <p class="text-3xl">{{ t("display.change") }}</p>
        <p class="num text-8xl font-bold">{{ money(state.change) }}</p>
      </div>
      <p v-if="state.onAccount" class="text-3xl">{{ t("display.onAccount") }} <span class="num font-semibold">{{ money(state.onAccount) }}</span></p>
    </div>

    <!-- Items being rung up -->
    <template v-else>
      <section class="flex min-w-0 flex-1 flex-col p-10">
        <header class="mb-6 flex items-center gap-4">
          <img v-if="state.logo" :src="state.logo" alt="" class="h-14 max-w-40 object-contain" />
          <p class="text-3xl font-bold">{{ state.shopName }}</p>
        </header>
        <ol ref="list" class="min-h-0 flex-1 divide-y overflow-y-auto text-3xl">
          <li v-for="(l, i) in state.lines" :key="i" class="flex items-baseline gap-6 py-4">
            <bdi class="min-w-0 flex-1 truncate font-medium">{{ l.name }}</bdi>
            <span class="num text-2xl text-muted-foreground"><bdi>{{ l.qty }}</bdi></span>
            <span class="num w-56 text-end font-semibold">{{ money(l.total) }}</span>
          </li>
        </ol>
      </section>
      <aside class="flex w-[40%] max-w-[34rem] shrink-0 flex-col justify-end gap-4 bg-secondary p-10 text-2xl">
        <p class="flex justify-between"><span class="text-muted-foreground">{{ t("display.subtotal") }}</span><span class="num">{{ money(state.subtotal) }}</span></p>
        <p v-if="saved" class="flex justify-between text-success"><span>{{ t("display.saved") }}</span><span class="num">−{{ money(saved) }}</span></p>
        <p v-if="state.tax" class="flex justify-between"><span class="text-muted-foreground">{{ t("display.tax") }}</span><span class="num">{{ money(state.tax) }}</span></p>
        <div class="mt-4 border-t pt-6">
          <p class="text-3xl text-muted-foreground">{{ t("display.total") }}</p>
          <p class="num text-[clamp(3rem,7vw,6rem)] leading-tight font-bold">{{ money(state.total) }}</p>
        </div>
      </aside>
    </template>
  </div>
</template>
