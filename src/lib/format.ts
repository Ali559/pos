import { numberLocale } from "@/lib/locale";
import { useShop } from "@/stores/shop";

const { currency } = useShop();
const cache = new Map<string, Intl.NumberFormat>();

/**
 * Formats money in the shop's currency and the UI language. Reads reactive
 * state, so templates re-render when either changes. Digits stay Western
 * (latn) in every language for now, matching barcodes and price labels.
 */
export const formatCurrency = (amount: number) => {
  const locale = numberLocale();
  const key = `${locale}|${currency.value}`;
  let fmt = cache.get(key);
  if (!fmt) {
    fmt = new Intl.NumberFormat(locale, {
      style: "currency",
      currency: currency.value,
      numberingSystem: "latn",
      // Dinar prices have no fractional part in practice
      ...(currency.value === "IQD" && { maximumFractionDigits: 0 }),
    });
    cache.set(key, fmt);
  }
  return fmt.format(amount);
};

const usdFormats = new Map<string, Intl.NumberFormat>();

/** US dollars, with cents only when there are some ($100, $12.50). */
export const formatUsd = (amount: number) => {
  const locale = numberLocale();
  const whole = Number.isInteger(Math.round(amount * 100) / 100);
  const key = `${locale}|${whole}`;
  let fmt = usdFormats.get(key);
  if (!fmt) {
    fmt = new Intl.NumberFormat(locale, {
      style: "currency",
      currency: "USD",
      numberingSystem: "latn",
      minimumFractionDigits: whole ? 0 : 2,
      maximumFractionDigits: 2,
    });
    usdFormats.set(key, fmt);
  }
  return fmt.format(amount);
};

/** A market rate the way people say it: "169,550" (IQD per 100 USD). */
export const formatRate = (per100: number) => Math.round(per100).toLocaleString("en");

/** An amount in the currency it's kept in: a dollar supplier's account is in USD. */
export const formatIn = (amount: number, currency: string | null | undefined) =>
  currency === "USD" ? formatUsd(amount) : formatCurrency(amount);
