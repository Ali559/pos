<script setup lang="ts">
import { nextTick, ref } from "vue";
import { useRouter } from "vue-router";
import { Delete, Loader2, ReceiptText } from "lucide-vue-next";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import LanguageSwitcher from "@/components/LanguageSwitcher.vue";
import MediaImage from "@/components/media/MediaImage.vue";
import { useTimeManager } from "@/hooks/useTimeManager";
import { formatCurrency } from "@/lib/format";
import { useShop } from "@/stores/shop";
import { landingPath, useAuth } from "@/stores/auth";
import { errorMessage } from "@/lib/errors";

const router = useRouter();
const username = ref("");
const password = ref("");
const { timeOfDay } = useTimeManager();
const { profile, name } = useShop();

// Decorative receipt on the brand panel, priced in the shop's currency
const sample = [
  { name: "Coca-Cola 330ml ×2", price: 1500 },
  { name: "Samoon bread (5)", price: 1000 },
  { name: "Fresh milk 1L", price: 1750 },
  { name: "Pomegranates 1kg", price: 3000 },
  { name: "Klicha date cookies", price: 3500 },
];
const sampleTotal = sample.reduce((n, l) => n + l.price, 0);

const auth = useAuth();
const mode = ref<"password" | "pin">("password");
const pin = ref("");
const pinInput = ref<HTMLInputElement | null>(null);
const busy = ref(false);
const error = ref("");

const run = async (attempt: () => Promise<unknown>) => {
  busy.value = true;
  error.value = "";
  try {
    await attempt();
    router.replace(landingPath());
  } catch (e) {
    error.value = errorMessage(e);
    password.value = "";
    pin.value = "";
  } finally {
    busy.value = false;
  }
};

const handleLogin = () => run(() => auth.login(username.value, password.value));
const handlePin = () => {
  if (pin.value.length >= 4) run(() => auth.loginPin(pin.value));
};

const pressKey = (key: string) => {
  error.value = "";
  if (key === "back") pin.value = pin.value.slice(0, -1);
  else if (pin.value.length < 6) pin.value += key;
};

const switchMode = (m: "password" | "pin") => {
  mode.value = m;
  error.value = "";
  if (m === "pin") nextTick(() => pinInput.value?.focus());
};
</script>

<template>
  <div class="grid min-h-full lg:grid-cols-[1.1fr_1fr]">
    <!-- Brand side -->
    <aside
      class="relative hidden flex-col justify-between overflow-hidden bg-brand p-12 text-brand-foreground lg:flex"
    >
      <div
        class="grid size-14 place-items-center overflow-hidden rounded-xl bg-sun text-sun-foreground"
      >
        <MediaImage
          v-if="profile?.logo"
          :media="profile.logo"
          fit="contain"
          class="size-full bg-white"
        />
        <ReceiptText v-else :size="26" />
      </div>

      <!-- receipt tape fed from the top edge: the till's own object -->
      <div
        class="receipt-edge absolute end-[12%] top-0 w-[22rem] bg-white px-7 pt-12 pb-10 text-[#17201c] shadow-2xl"
        aria-hidden="true"
      >
        <div class="mb-5 flex justify-between text-sm text-[#5f6a64]">
          <span>#1042</span><span>{{ timeOfDay }}</span>
        </div>
        <div class="space-y-3 text-[15px]">
          <div v-for="line in sample" :key="line.name" class="flex justify-between gap-4">
            <bdi>{{ line.name }}</bdi><span>{{ formatCurrency(line.price) }}</span>
          </div>
          <div class="rule-dashed !my-5" />
          <div class="flex items-baseline justify-between">
            <span class="font-semibold">{{ $t("pos.total") }}</span>
            <span class="text-4xl font-bold tracking-tight">{{
              formatCurrency(sampleTotal)
            }}</span>
          </div>
        </div>
      </div>

      <div>
        <p class="text-6xl leading-tight font-semibold tracking-tight">
          {{ name || $t("login.title") }}
        </p>
        <p class="mt-4 text-sm opacity-70">{{ timeOfDay }}</p>
      </div>
    </aside>

    <!-- Form side -->
    <main class="flex flex-col p-6 sm:p-10">
      <div class="flex justify-end">
        <LanguageSwitcher />
      </div>
      <div class="m-auto w-full max-w-sm">
        <h1 class="text-3xl font-semibold tracking-tight">
          {{ $t("login.subtitle") }}
        </h1>
        <p class="mt-2 text-muted-foreground">{{ $t("login.footer") }}</p>

        <div class="mt-8 grid grid-cols-2 rounded-lg bg-muted p-1" role="tablist">
          <button
            v-for="m in ['password', 'pin'] as const"
            :key="m"
            type="button"
            role="tab"
            :aria-selected="mode === m"
            class="h-9 rounded-md text-sm font-medium text-muted-foreground transition-colors aria-selected:bg-card aria-selected:text-foreground aria-selected:shadow-sm"
            @click="switchMode(m)"
          >
            {{ $t(`login.mode.${m}`) }}
          </button>
        </div>

        <form
          v-if="mode === 'password'"
          class="mt-6 space-y-5"
          @submit.prevent="handleLogin"
        >
          <div class="space-y-2">
            <Label for="username">{{ $t("login.username") }}</Label>
            <Input
              id="username"
              v-model="username"
              type="text"
              dir="ltr"
              autocomplete="username"
              autocapitalize="off"
              required
              :placeholder="$t('login.usernamePlaceholder')"
              class="h-12"
            />
          </div>
          <div class="space-y-2">
            <Label for="password">{{ $t("login.password") }}</Label>
            <Input
              id="password"
              v-model="password"
              type="password"
              dir="ltr"
              autocomplete="current-password"
              required
              :placeholder="$t('login.passwordPlaceholder')"
              class="h-12"
            />
          </div>
          <p v-if="error" role="alert" class="text-sm font-medium text-destructive">
            {{ error }}
          </p>
          <Button type="submit" size="lg" class="h-12 w-full" :disabled="busy">
            <Loader2 v-if="busy" class="animate-spin" />
            {{ $t("login.submit") }}
          </Button>
        </form>

        <form v-else class="mt-6 space-y-5" @submit.prevent="handlePin">
          <label for="pin" class="sr-only">{{ $t("staff.fields.pin") }}</label>
          <input
            id="pin"
            ref="pinInput"
            v-model="pin"
            type="password"
            inputmode="numeric"
            autocomplete="off"
            maxlength="6"
            dir="ltr"
            class="h-14 w-full rounded-lg border border-input bg-card text-center text-3xl tracking-[0.5em] outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            @input="pin = pin.replace(/\D/g, ''); error = ''"
          />
          <!-- Touch keypad; digits read left-to-right in every language -->
          <div class="grid grid-cols-3 gap-2" dir="ltr">
            <button
              v-for="k in ['1', '2', '3', '4', '5', '6', '7', '8', '9']"
              :key="k"
              type="button"
              class="h-14 rounded-lg border bg-card text-xl font-medium hover:bg-accent active:scale-[0.97]"
              @click="pressKey(k)"
            >
              {{ k }}
            </button>
            <span />
            <button
              type="button"
              class="h-14 rounded-lg border bg-card text-xl font-medium hover:bg-accent active:scale-[0.97]"
              @click="pressKey('0')"
            >
              0
            </button>
            <button
              type="button"
              class="grid h-14 place-items-center rounded-lg text-muted-foreground hover:bg-accent"
              :aria-label="$t('login.backspace')"
              @click="pressKey('back')"
            >
              <Delete :size="22" />
            </button>
          </div>
          <p v-if="error" role="alert" class="text-sm font-medium text-destructive">
            {{ error }}
          </p>
          <Button
            type="submit"
            size="lg"
            class="h-12 w-full"
            :disabled="busy || pin.length < 4"
          >
            <Loader2 v-if="busy" class="animate-spin" />
            {{ $t("login.submit") }}
          </Button>
        </form>
      </div>
    </main>
  </div>
</template>
