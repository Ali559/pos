<script setup lang="ts">
import { computed, onMounted, provide, watchEffect } from "vue";
import {
  DEFAULT_LANGUAGE,
  i18n,
  isLanguage,
  languages,
  type Language,
} from "@/utils/i18n";
import { getSetting, setSetting } from "@/db/settings";

// The `settings` table is the source of truth; localStorage is only a cache
// so the first paint is already in the right language and direction.
const CACHE_KEY = "app-language";

const readCache = (): Language | null => {
  try {
    const v = localStorage.getItem(CACHE_KEY);
    return isLanguage(v) ? v : null;
  } catch {
    return null;
  }
};

const language = computed(() => i18n.global.locale.value as Language);
const direction = computed(() => languages[language.value].dir);

const apply = (lang: Language) => {
  i18n.global.locale.value = lang;
  try {
    localStorage.setItem(CACHE_KEY, lang);
  } catch {}
};

const setLanguage = async (lang: Language) => {
  apply(lang);
  await setSetting("language", lang);
};

apply(readCache() ?? DEFAULT_LANGUAGE);

watchEffect(() => {
  document.documentElement.lang = language.value;
  document.documentElement.dir = direction.value;
});

onMounted(async () => {
  try {
    const saved = await getSetting("language");
    if (isLanguage(saved) && saved !== language.value) apply(saved);
  } catch (e) {
    console.error("[locale] could not read language setting", e);
  }
});

provide("language", language);
provide("direction", direction);
provide("setLanguage", setLanguage);
</script>

<template>
  <div :dir="direction" :lang="language" class="fixed inset-0 overflow-auto">
    <slot />
  </div>
</template>
