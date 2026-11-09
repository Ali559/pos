import { ref } from "vue";
import { getSetting, setSetting } from "@/db/settings";

// How the shop takes money: card payments (off unless switched on) and
// loyalty points (on unless switched off). Shared by the register, returns,
// customer screens and Settings.
const cards = ref(false);
const loyalty = ref(true);
/** Money paid for one point */
const earnPer = ref(1000);
/** What one point is worth when spent */
const pointValue = ref(10);
let loading: Promise<void> | null = null;

const load = () =>
  (loading ??= (async () => {
    const [c, l, e, v] = await Promise.all(["card_payments", "loyalty_enabled", "loyalty_earn_per", "loyalty_point_value"].map(getSetting));
    cards.value = c === "1";
    loyalty.value = l !== "0";
    earnPer.value = Number(e) > 0 ? Number(e) : 1000;
    pointValue.value = Number(v) > 0 ? Number(v) : 10;
  })().catch((e) => {
    loading = null;
    throw e;
  }));

const reload = () => {
  loading = null;
  return load();
};

export const usePayments = () => ({
  cards,
  loyalty,
  earnPer,
  pointValue,
  load,
  reload,
  async save(next: { cards: boolean; loyalty: boolean; earnPer: number; pointValue: number }) {
    await setSetting("card_payments", next.cards ? "1" : "0");
    await setSetting("loyalty_enabled", next.loyalty ? "1" : "0");
    await setSetting("loyalty_earn_per", String(next.earnPer));
    await setSetting("loyalty_point_value", String(next.pointValue));
    await reload();
  },
});
