import { ref, watch } from "vue";

export type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "app-theme";
const media = window.matchMedia("(prefers-color-scheme: dark)");

const read = (): Theme => {
  try {
    return (localStorage.getItem(STORAGE_KEY) as Theme) || "system";
  } catch {
    return "system";
  }
};

// Shared across every caller so the select and the document stay in sync
const theme = ref<Theme>(read());

const apply = () => {
  const dark =
    theme.value === "dark" || (theme.value === "system" && media.matches);
  document.documentElement.classList.toggle("dark", dark);
};

watch(theme, (value) => {
  try {
    localStorage.setItem(STORAGE_KEY, value);
  } catch {}
  apply();
});
media.addEventListener("change", apply);

export const initTheme = apply;

export const useTheme = () => ({ theme });
