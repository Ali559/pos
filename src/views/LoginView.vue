<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { ReceiptText } from "lucide-vue-next";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import RTLToggle from "@/components/RTLToggle.vue";
import { useTimeManager } from "@/hooks/useTimeManager";

const router = useRouter();
const username = ref("");
const password = ref("");
const { timeOfDay } = useTimeManager();

const handleLogin = () => {
  // Add your login logic here
  router.push("/main/point-of-sale");
};
</script>

<template>
  <div class="grid min-h-screen lg:grid-cols-[1.1fr_1fr]">
    <!-- Brand side -->
    <aside
      class="relative hidden flex-col justify-between overflow-hidden bg-brand p-12 text-brand-foreground lg:flex"
    >
      <div class="grid size-12 place-items-center rounded-xl bg-sun text-sun-foreground">
        <ReceiptText :size="24" />
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
          <div class="flex justify-between"><span>Coca-Cola 330ml ×2</span><span class="num">$1.50</span></div>
          <div class="flex justify-between"><span>Samoon bread (5)</span><span class="num">$0.50</span></div>
          <div class="flex justify-between"><span>Fresh milk 1L</span><span class="num">$1.25</span></div>
          <div class="flex justify-between"><span>Pomegranates 1kg</span><span class="num">$2.00</span></div>
          <div class="flex justify-between"><span>Klicha date cookies</span><span class="num">$2.75</span></div>
          <div class="rule-dashed !my-5" />
          <div class="flex items-baseline justify-between">
            <span class="font-semibold">Total</span>
            <span class="text-4xl font-bold tracking-tight">$8.80</span>
          </div>
        </div>
      </div>

      <div>
        <p class="text-6xl leading-none font-semibold tracking-tight">
          {{ $t("login.title") }}
        </p>
        <p class="mt-4 text-sm opacity-70">{{ timeOfDay }}</p>
      </div>
    </aside>

    <!-- Form side -->
    <main class="flex flex-col p-6 sm:p-10">
      <div class="flex justify-end">
        <RTLToggle />
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
