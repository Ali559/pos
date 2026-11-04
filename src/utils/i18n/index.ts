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
