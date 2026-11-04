// Icon search and name-based suggestions over the Tabler index
// (src/data/icon-index.json, built by scripts/build-icon-index.mjs).
// Pure functions, no Vue, so they can be tested with `bun`.
import { ICON_SYNONYMS } from "@/data/iconSynonyms";

export interface IconEntry {
  key: string; // "tabler:bottle"
  name: string;
  category: string;
  words: string[]; // name split on "-"
  tags: string[];
}

export const toEntries = (raw: [string, string, string][]): IconEntry[] =>
  raw.map(([name, category, tags]) => ({
    key: `tabler:${name}`,
    name,
    category,
    words: name.split("-"),
    tags: tags ? tags.split(" ") : [],
  }));

/**
 * Lowercase, strip Arabic diacritics and tatweel, and fold letter variants
 * Kurdish and Arabic spell differently (ي/ى→ی, ك→ک, أإآ→ا, ة→ە, ه at word
 * end is left alone).
 */
export const normalize = (s: string) =>
  s
    .toLowerCase()
    .replace(/[ً-ْـ]/g, "")
    .replace(/[يى]/g, "ی")
    .replace(/ك/g, "ک")
    .replace(/[أإآ]/g, "ا")
    .replace(/ة/g, "ە");

// Sizes and counts carry no meaning for an icon: "330ml", "1.5L", "x6", "5"
const NOISE = /^(\d+([.,]\d+)?(ml|l|g|gr|kg|cl|oz|pcs|x)?|x\d+|\d+x)$/;

export const tokenize = (text: string) =>
  normalize(text)
    .split(/[^\p{L}\p{N}]+/u)
    .map((t) => t.replace(/^ال(?=\p{L}{2})/u, "")) // Arabic definite article
    .filter((t) => t.length >= 2 && !NOISE.test(t));

const synonyms = ICON_SYNONYMS.flatMap(([en, words]) =>
  words.split("|").map((w) => ({ word: normalize(w), keywords: en.split(" ") })),
);

/** English keywords for a product or category name in any of the three languages. */
export const keywordsFor = (text: string): string[] => {
  const tokens = tokenize(text);
  const phrase = tokens.join(" ");
  const out = new Set<string>();
  for (const t of tokens) {
    if (/^[a-z]+$/.test(t)) {
      out.add(t);
      if (t.length > 3 && t.endsWith("s")) out.add(t.slice(0, -1));
    }
  }
  for (const { word, keywords } of synonyms) {
    const hit = word.includes(" ")
      ? phrase.includes(word)
      : tokens.some((t) => t === word || (word.length >= 3 && t.startsWith(word)));
    if (hit) keywords.forEach((k) => out.add(k));
  }
  return [...out];
};

const score = (icon: IconEntry, keywords: string[]) => {
  let s = 0;
  for (const k of keywords) {
    if (icon.name === k) s += 10;
    else if (icon.words.includes(k)) s += 6;
    else if (icon.tags.includes(k)) s += 4;
    else if (k.length >= 4 && (icon.name.startsWith(k) || icon.tags.some((t) => t.startsWith(k)))) s += 1;
  }
  return s;
};

/**
 * Best icons for a name. `boostCategory` (a Tabler category such as "Food",
 * taken from the parent category's icon) breaks ties toward related icons.
 */
export const suggest = (
  index: IconEntry[],
  text: string,
  { limit = 8, boostCategory }: { limit?: number; boostCategory?: string } = {},
) => {
  const keywords = keywordsFor(text);
  if (!keywords.length) return [];
  const ranked = index
    .map((icon) => {
      const s = score(icon, keywords);
      return { icon, s: s && icon.category === boostCategory ? s + 1 : s };
    })
    .filter((r) => r.s > 0)
    .sort((a, b) => b.s - a.s || a.icon.name.length - b.icon.name.length);
  // Loose prefix-only matches are noise once there are a few real ones
  const solid = ranked.filter((r) => r.s >= 4);
  return (solid.length >= 3 ? solid : ranked).slice(0, limit).map((r) => r.icon);
};

/** Picker search: every word must match a name part or a tag prefix. */
export const search = (index: IconEntry[], query: string, limit = 160) => {
  const words = normalize(query).split(/\s+/).filter(Boolean);
  if (!words.length) return index.slice(0, limit);
  // A Kurdish/Arabic query searches by its English keywords instead
  const terms = words.every((w) => /^[a-z0-9-]+$/.test(w)) ? words : keywordsFor(query);
  if (!terms.length) return [];
  const matches = (icon: IconEntry, w: string) =>
    icon.name.includes(w) || icon.tags.some((t) => t.startsWith(w));
  const any = !words.every((w) => /^[a-z0-9-]+$/.test(w));
  return index
    .filter((icon) => (any ? terms.some((w) => matches(icon, w)) : terms.every((w) => matches(icon, w))))
    .slice(0, limit);
};
