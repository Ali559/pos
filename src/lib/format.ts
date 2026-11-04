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
