import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
import { i18n } from "./utils/i18n";
import { initTheme } from "./hooks/useTheme";

initTheme();

createApp(App).use(router).use(i18n).mount("#app");
