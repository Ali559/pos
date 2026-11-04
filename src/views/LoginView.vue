<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { ReceiptText } from "lucide-vue-next";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import LanguageSwitcher from "@/components/LanguageSwitcher.vue";
import MediaImage from "@/components/media/MediaImage.vue";
import { useTimeManager } from "@/hooks/useTimeManager";
import { formatCurrency } from "@/lib/format";
import { useShop } from "@/stores/shop";

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

const handleLogin = () => {
  // Add your login logic here
  router.push("/main/point-of-sale");
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

        <form class="mt-8 space-y-5" @submit.prevent="handleLogin">
          <div class="space-y-2">
            <Label for="username">{{ $t("login.username") }}</Label>
            <Input
              id="username"
              v-model="username"
              type="text"
              autocomplete="username"
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
              autocomplete="current-password"
              required
              :placeholder="$t('login.passwordPlaceholder')"
              class="h-12"
            />
          </div>
          <Button type="submit" size="lg" class="h-12 w-full">
            {{ $t("login.submit") }}
          </Button>
        </form>
      </div>
    </main>
  </div>
</template>
