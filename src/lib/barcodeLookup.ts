import { call } from "./ipc";
import { keywordsFor } from "./iconSearch";
import type { MediaFile } from "./media";
import type { Category } from "@/db/catalog";

/** Mirrors lookup::Lookup in src-tauri/src/lookup.rs. */
export interface Lookup {
  existingProductId: number | null;
  name: string | null;
  brand: string | null;
  quantity: string | null;
  categoryTags: string[];
  image: MediaFile | null;
  source: "local" | "openfoodfacts" | "none";
  offline: boolean;
}

export const lookupBarcode = (barcode: string) => call<Lookup>("lookup_barcode", { barcode });

/**
 * The shop category that best matches Open Food Facts tags such as
 * "en:carbonated-drinks". Both sides go through the icon keyword map, so
 * Kurdish and Arabic category names match English tags. Subcategories win
 * ties because they're more specific.
 */
export const guessCategory = (tags: string[], categories: Category[]): Category | null => {
  const tagWords = new Set(keywordsFor(tags.map((t) => t.replace(/^\w\w:/, "").replace(/-/g, " ")).join(" ")));
  if (!tagWords.size) return null;
  let best: { c: Category; score: number } | null = null;
  for (const c of categories) {
    if (!c.is_active) continue;
    const score = keywordsFor(c.name).filter((k) => tagWords.has(k)).length + (c.parent_id ? 0.5 : 0);
    if (score >= 1 && (!best || score > best.score)) best = { c, score };
  }
  return best?.c ?? null;
};
