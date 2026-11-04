import { ref } from "vue";
import { addCollection } from "@iconify/vue/offline";
import { search, suggest, toEntries, type IconEntry } from "./iconSearch";

// The drawing data (~2 MB) and the search index (~450 KB) are separate
// chunks, loaded once in the background so startup isn't blocked. The
// offline build of @iconify/vue never fetches from the Iconify API.

export const iconsReady = ref(false);
let drawing: Promise<void> | null = null;
let indexLoad: Promise<IconEntry[]> | null = null;

export const loadIcons = () =>
  (drawing ??= import("@iconify-json/tabler/icons.json").then((m) => {
    addCollection(m.default);
    iconsReady.value = true;
  }));

export const loadIconIndex = () =>
  (indexLoad ??= import("@/data/icon-index.json").then((m) =>
    toEntries(m.default as [string, string, string][]),
  ));

export const suggestIcons = async (text: string, boostFrom?: string | null) => {
  const index = await loadIconIndex();
  const boostCategory = boostFrom
    ? index.find((i) => i.key === boostFrom)?.category
    : undefined;
  return suggest(index, text, { boostCategory });
};

export const searchIcons = async (query: string) => search(await loadIconIndex(), query);

// Recently picked icons, per device
const RECENT_KEY = "recent-icons";
export const recentIcons = (): string[] => {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
  } catch {
    return [];
  }
};
export const rememberIcon = (key: string) => {
  try {
    const next = [key, ...recentIcons().filter((k) => k !== key)].slice(0, 16);
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {}
};
