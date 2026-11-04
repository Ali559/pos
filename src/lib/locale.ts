import { i18n, languages, type Language } from "@/utils/i18n";

// Chromium (and so WebView2 on Windows) ships without Sorani (ckb) locale
// data, and Intl silently falls back to English. Detect that once and fill
// the gap: Arabic-Iraq number/currency rules (same "د.ع" dinar sign), and
// hand-written Kurdish month and weekday names for dates.
const hasCkb = Intl.DateTimeFormat.supportedLocalesOf(["ckb-IQ"]).length > 0;

export const currentLanguage = () => i18n.global.locale.value as Language;

/** Locale to hand to Intl.NumberFormat for a UI language. */
export const numberLocale = (lang: Language = currentLanguage()) =>
  lang === "ku" && !hasCkb ? "ar-IQ" : languages[lang].intl;

// Iraqi Kurdistan uses the Syriac-derived Gregorian month names
const KU_MONTHS = [
  "کانوونی دووەم", "شوبات", "ئازار", "نیسان", "ئایار", "حوزەیران",
  "تەممووز", "ئاب", "ئەیلوول", "تشرینی یەکەم", "تشرینی دووەم", "کانوونی یەکەم",
];
const KU_WEEKDAYS = [
  "یەکشەممە", "دووشەممە", "سێشەممە", "چوارشەممە", "پێنجشەممە", "هەینی", "شەممە",
];

const pad = (n: number) => String(n).padStart(2, "0");

/** Weekday, day, month and time, e.g. for the header clock. */
export const formatDateTime = (date: Date, lang: Language = currentLanguage()) => {
  if (lang === "ku" && !hasCkb) {
    return `${KU_WEEKDAYS[date.getDay()]}، ${date.getDate()}ی ${KU_MONTHS[date.getMonth()]}، ${pad(date.getHours())}:${pad(date.getMinutes())}`;
  }
  return date.toLocaleString(languages[lang].intl, {
    weekday: "short",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    numberingSystem: "latn",
  });
};

/** "18:00" — 24-hour clock with Western digits in every language. */
export const formatTime = (date: Date, lang: Language = currentLanguage()) =>
  date.toLocaleTimeString(numberLocale(lang), {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    numberingSystem: "latn",
  });

/** Short weekday + day, e.g. for schedule column headers. */
export const formatDay = (date: Date, lang: Language = currentLanguage()) => {
  if (lang === "ku" && !hasCkb) return `${KU_WEEKDAYS[date.getDay()]} ${date.getDate()}`;
  return date.toLocaleDateString(languages[lang].intl, {
    weekday: "short",
    day: "numeric",
    numberingSystem: "latn",
  });
};

/** "November 2026" in the UI language (Kurdish names when Intl lacks them). */
export const formatMonthYear = (date: Date, lang: Language = currentLanguage()) => {
  if (lang === "ku" && !hasCkb) return `${KU_MONTHS[date.getMonth()]} ${date.getFullYear()}`;
  return date.toLocaleDateString(languages[lang].intl, { month: "long", year: "numeric", numberingSystem: "latn" });
};

/** "4 November 2026": a full date for display. */
export const formatDate = (date: Date, lang: Language = currentLanguage()) => {
  if (lang === "ku" && !hasCkb) return `${date.getDate()}ی ${KU_MONTHS[date.getMonth()]} ${date.getFullYear()}`;
  return date.toLocaleDateString(languages[lang].intl, { day: "numeric", month: "long", year: "numeric", numberingSystem: "latn" });
};

/** One- or two-letter weekday headers, Saturday first (0 = Sunday). */
const SHORT_WEEKDAYS: Record<Language, string[]> = {
  en: ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"],
  ku: ["ی", "د", "س", "چ", "پ", "ھ", "ش"],
  ar: ["ح", "ن", "ث", "ر", "خ", "ج", "س"],
};
export const shortWeekday = (day: number, lang: Language = currentLanguage()) => SHORT_WEEKDAYS[lang][day];
