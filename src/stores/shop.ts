import { computed, ref } from "vue";
import {
  getShopProfile,
  saveShopProfile,
  type ShopProfile,
  type ShopProfileWithLogo,
} from "@/db/shop";
import { getSetting, setSetting } from "@/db/settings";
import { localized } from "@/utils/i18n";

// Module-level state shared by every caller (header, register, settings)
const profile = ref<ShopProfileWithLogo | null>(null);
const taxRate = ref(0);
const loaded = ref(false);
let loading: Promise<void> | null = null;

const load = () =>
  (loading ??= (async () => {
    const [p, tax] = await Promise.all([getShopProfile(), getSetting("tax_rate")]);
    profile.value = p;
    taxRate.value = Number(tax) || 0;
    loaded.value = true;
  })());

const reload = () => {
  loading = null;
  return load();
};

export const useShop = () => ({
  profile,
  taxRate,
  loaded,
  load,
  reload,
  /** Shop name in the current UI language, falling back to English. */
  name: computed(() => (profile.value ? localized(profile.value, "name") : "")),
  currency: computed(() => profile.value?.currency ?? "IQD"),
  async save(next: ShopProfile) {
    await saveShopProfile(next);
    await reload();
  },
  async saveTaxRate(rate: number) {
    await setSetting("tax_rate", String(rate));
    taxRate.value = rate;
  },
});
