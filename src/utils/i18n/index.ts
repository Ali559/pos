import { createI18n } from "vue-i18n";
import en from "./locales/en";
import ku from "./locales/ku";
import ar from "./locales/ar";

export type Language = "ku" | "ar" | "en";

/**
 * Language is the setting; direction follows from it. `intl` is the locale
 * passed to Intl formatters (Western digits are forced separately in
 * src/lib/format.ts).
 */
export const languages: Record<
  Language,
  { label: string; dir: "ltr" | "rtl"; intl: string }
> = {
  ku: { label: "کوردی", dir: "rtl", intl: "ckb-IQ" },
  ar: { label: "العربية", dir: "rtl", intl: "ar-IQ" },
  en: { label: "English", dir: "ltr", intl: "en-US" },
};

export const DEFAULT_LANGUAGE: Language = "ku";

export const isLanguage = (v: unknown): v is Language =>
  typeof v === "string" && v in languages;

export const i18n = createI18n({
  legacy: false,
  locale: DEFAULT_LANGUAGE,
  fallbackLocale: "en",
  globalInjection: true,
  messages: { en, ku, ar },
});

/**
 * Picks the column for the current language from a row with per-language
 * columns. English may be stored as `name` (categories) or `name_en`
 * (shop_profile); empty translations fall back to English.
 */
export const localized = <T extends object>(
  row: T,
  field: string,
  lang: Language = i18n.global.locale.value as Language,
): string => {
  const r = row as Record<string, unknown>;
  for (const key of [`${field}_${lang}`, field, `${field}_en`]) {
    const v = r[key];
    if (typeof v === "string" && v.trim()) return v;
  }
  return "";
};
