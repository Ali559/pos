<script setup lang="ts">
import { ref, onMounted, provide } from "vue";
import { i18n } from "@/utils/i18n";
type Direction = "ltr" | "rtl";
type Language = "en" | "ku";

const direction = ref<Direction>("ltr");
const language = ref<Language>("en");

const applyLanguage = (lang: Language) => {
  language.value = lang;
  i18n.global.locale.value = lang; // 👈 THIS is the missing link
};

const toggleDirection = () => {
  direction.value = direction.value === "ltr" ? "rtl" : "ltr";
  applyLanguage(direction.value === "rtl" ? "ku" : "en");
  updateDocument();
  saveToStorage();
};

const setDirection = (dir: Direction) => {
  direction.value = dir;
  applyLanguage(dir === "rtl" ? "ku" : "en");
  updateDocument();
  saveToStorage();
};

const updateDocument = () => {
  document.documentElement.dir = direction.value;
  document.documentElement.lang = language.value;
};

const saveToStorage = () => {
  localStorage.setItem("app-direction", direction.value);
  localStorage.setItem("app-language", language.value);
};

const loadFromStorage = () => {
  const savedDirection = localStorage.getItem("app-direction") as Direction;
  const savedLanguage = localStorage.getItem("app-language") as Language;
  if (savedDirection) {
    direction.value = savedDirection;
    const lang = savedLanguage || (savedDirection === "rtl" ? "ku" : "en");
    applyLanguage(lang);
  } else {
    // Detect from browser
    const rtlLanguages = ["ku", "he", "fa", "ur"];
    const browserLang = navigator.language.split("-")[0];
    if (rtlLanguages.includes(browserLang)) {
      direction.value = "rtl";
      language.value = "ku";
    }
  }
};

// Provide to child components
provide("direction", direction);
provide("language", language);
provide("toggleDirection", toggleDirection);
provide("setDirection", setDirection);

onMounted(() => {
  loadFromStorage();
  updateDocument();
});
</script>

<template>
  <div :dir="direction" :lang="language">
    <slot />
  </div>
</template>
