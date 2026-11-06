<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { inDesktopApp } from "./lib/ipc";
import { getSetting } from "./db/settings";
import LocaleProvider from "./components/providers/LocaleProvider.vue";
import PrintRoot from "./components/sales/PrintRoot.vue";
import StatementPrint from "./components/customers/StatementPrint.vue";
import LabelsPrint from "./components/catalog/LabelsPrint.vue";

// Full screen: on at start when the shop wants a kiosk-style till, F11 any time
const toggleFullscreen = async (e: KeyboardEvent) => {
  if (e.key !== "F11" || !inDesktopApp) return;
  e.preventDefault();
  const win = getCurrentWindow();
  await win.setFullscreen(!(await win.isFullscreen()));
};
onMounted(async () => {
  window.addEventListener("keydown", toggleFullscreen);
  if (inDesktopApp && (await getSetting("fullscreen").catch(() => null)) === "1") {
    await getCurrentWindow().setFullscreen(true);
  }
});
onUnmounted(() => window.removeEventListener("keydown", toggleFullscreen));
</script>

<template>
  <locale-provider>
    <router-view />
    <print-root />
    <statement-print />
    <labels-print />
  </locale-provider>
</template>
