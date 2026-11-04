<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import LanguageSwitcher from "@/components/LanguageSwitcher.vue";
import ShopForm from "@/components/shop/ShopForm.vue";
import { emptyShopProfile } from "@/db/shop";
import type { MediaFile } from "@/lib/media";
import { errorMessage } from "@/lib/errors";
import { useShop } from "@/stores/shop";
import { landingPath, useAuth } from "@/stores/auth";

const router = useRouter();
const shop = useShop();
const auth = useAuth();

// A shop saved by an earlier version (before accounts existed) is reused
const existing = shop.profile.value;
const form = ref(existing ? (({ logo: _, ...p }) => p)(existing) : emptyShopProfile());
const logo = ref<MediaFile | null>(existing?.logo ?? null);

const owner = ref({ fullName: "", username: "", password: "", confirm: "", pin: "" });
const saving = ref(false);
const error = ref("");

const submit = async () => {
  error.value = "";
  if (owner.value.password !== owner.value.confirm) {
    error.value = errorMessage({ code: "password_mismatch", message: "" });
    return;
  }
  saving.value = true;
  try {
    const { confirm: _, pin, ...rest } = owner.value;
    await auth.completeSetup(form.value, { ...rest, pin: pin || null });
    await shop.reload();
    router.replace(landingPath());
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};
</script>

<template>
  <div class="min-h-full bg-background">
    <div class="mx-auto flex max-w-3xl flex-col p-6 sm:p-10">
      <div class="flex justify-end">
        <LanguageSwitcher />
      </div>
      <h1 class="mt-8 text-3xl font-semibold tracking-tight">
        {{ $t("setup.title") }}
      </h1>
      <p class="mt-2 max-w-xl text-muted-foreground">{{ $t("setup.subtitle") }}</p>

      <form class="mt-10 space-y-6" @submit.prevent="submit">
        <section class="rounded-xl border bg-card p-6">
          <h2 class="mb-5 font-semibold">{{ $t("setup.shopSection") }}</h2>
          <ShopForm v-model="form" v-model:logo="logo" compact />
        </section>

        <section class="rounded-xl border bg-card p-6">
          <h2 class="font-semibold">{{ $t("setup.ownerSection") }}</h2>
          <p class="mt-1 mb-5 text-sm text-muted-foreground">{{ $t("setup.ownerHint") }}</p>
          <div class="grid gap-4 md:grid-cols-2">
            <div class="space-y-2">
              <Label for="owner-name">{{ $t("staff.fields.fullName") }}</Label>
              <Input id="owner-name" v-model="owner.fullName" required autocomplete="name" />
            </div>
            <div class="space-y-2">
              <Label for="owner-username">{{ $t("login.username") }}</Label>
              <Input
                id="owner-username"
                v-model="owner.username"
                required
                dir="ltr"
                autocomplete="username"
                autocapitalize="off"
              />
            </div>
            <div class="space-y-2">
              <Label for="owner-password">{{ $t("login.password") }}</Label>
              <Input
                id="owner-password"
                v-model="owner.password"
                type="password"
                required
                minlength="6"
                dir="ltr"
                autocomplete="new-password"
              />
              <p class="text-xs text-muted-foreground">{{ $t("staff.passwordHint") }}</p>
            </div>
            <div class="space-y-2">
              <Label for="owner-confirm">{{ $t("staff.fields.confirmPassword") }}</Label>
              <Input
                id="owner-confirm"
                v-model="owner.confirm"
                type="password"
                required
                dir="ltr"
                autocomplete="new-password"
              />
            </div>
            <div class="space-y-2">
              <Label for="owner-pin">
                {{ $t("staff.fields.pin") }}
                <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
              </Label>
              <Input
                id="owner-pin"
                v-model="owner.pin"
                type="password"
                inputmode="numeric"
                pattern="[0-9]{4,6}"
                maxlength="6"
                dir="ltr"
                autocomplete="off"
              />
              <p class="text-xs text-muted-foreground">{{ $t("staff.pinHint") }}</p>
            </div>
          </div>
        </section>

        <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
        <div class="flex justify-end">
          <Button type="submit" size="lg" :disabled="saving">
            <Loader2 v-if="saving" class="animate-spin" />
            {{ $t("setup.submit") }}
          </Button>
        </div>
      </form>
    </div>
  </div>
</template>
