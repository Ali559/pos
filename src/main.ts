import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
import { i18n } from "./utils/i18n";
import { initTheme } from "./hooks/useTheme";
import { loadIcons } from "./lib/icons";
import { isDisplayWindow } from "./lib/customerDisplay";
import CustomerDisplay from "./components/display/CustomerDisplay.vue";

initTheme();

// Icon drawings load in the background; icons appear once ready
loadIcons();

// The customer display is the same app in its own window, showing only
// what the till sends it (src/lib/customerDisplay.ts)
if (isDisplayWindow()) {
  createApp(CustomerDisplay).use(i18n).mount("#app");
} else {
  createApp(App).use(router).use(i18n).mount("#app");
}
