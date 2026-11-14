import { computed, ref } from "vue";
import { call } from "@/lib/ipc";

// The US dollar rate (IQD per 100 USD, as the market quotes it), shared by
// the register, the drawer, suppliers and Settings. Rust keeps it; a newer
// rate read from Telegram waits as an `offer` until a manager accepts it.
export interface Rate {
  id: number;
  per100: number;
  source: string;
  city: string | null;
  postedAt: string | null;
  setBy: string | null;
  createdAt: string;
}

export interface RateOffer {
  per100: number;
  city: string;
  source: string;
  postedAt: string;
  fetchedAt: string;
}

export interface RateStatus {
  enabled: boolean;
  current: Rate | null;
  offer: RateOffer | null;
  fetch: boolean;
  city: string;
  lastFetch: string | null;
  lastError: string | null;
  history: Rate[];
}

export const RATE_CITIES = ["erbil", "sulaymaniyah", "duhok", "kirkuk", "baghdad", "mosul", "basra"] as const;

const status = ref<RateStatus | null>(null);
let loading: Promise<void> | null = null;
let timer: ReturnType<typeof setInterval> | null = null;

const set = (s: RateStatus) => {
  status.value = s;
};

const load = () =>
  (loading ??= call<RateStatus>("exchange_rate")
    .then(set)
    .catch((e) => {
      loading = null;
      throw e;
    }));

const reload = () => {
  loading = null;
  return load();
};

/** Dollars at the current rate, in whole dinars (as Rust rounds them). */
export const toIqd = (usd: number, per100: number) => Math.round((usd * per100) / 100);

export const useRate = () => ({
  status,
  enabled: computed(() => !!status.value?.enabled),
  /** IQD per 100 USD in use, or null when none is set */
  per100: computed(() => status.value?.current?.per100 ?? null),
  /** Dollars are on and a rate is set: the till can take them */
  ready: computed(() => !!status.value?.enabled && status.value.current != null),
  offer: computed(() => status.value?.offer ?? null),
  load,
  reload,
  /** Keeps the rate fresh while a screen that shows it is open */
  watch() {
    load().catch(() => {});
    timer ??= setInterval(() => reload().catch(() => {}), 60_000);
  },
  async setRate(per100: number) {
    set(await call<RateStatus>("set_rate", { per100 }));
  },
  async accept(per100: number) {
    set(await call<RateStatus>("accept_rate_offer", { per100 }));
  },
  async dismiss() {
    set(await call<RateStatus>("dismiss_rate_offer"));
  },
  async fetchNow() {
    set(await call<RateStatus>("fetch_rate_now"));
  },
});
