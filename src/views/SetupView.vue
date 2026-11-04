<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import LanguageSwitcher from "@/components/LanguageSwitcher.vue";
import ShopForm from "@/components/shop/ShopForm.vue";
import { emptyShopProfile } from "@/db/shop";
import type { MediaFile } from "@/lib/media";
import { useShop } from "@/stores/shop";

const router = useRouter();
const shop = useShop();

const form = ref(emptyShopProfile());
const logo = ref<MediaFile | null>(null);
const saving = ref(false);
const error = ref("");

const submit = async () => {
  saving.value = true;
  error.value = "";
  try {
    await shop.save(form.value);
    router.replace("/");
  } catch (e) {
    error.value = String(e);
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

      <form class="mt-10 rounded-xl border bg-card p-6" @submit.prevent="submit">
        <ShopForm v-model="form" v-model:logo="logo" compact />
        <p v-if="error" role="alert" class="mt-6 text-sm text-destructive">{{ error }}</p>
        <div class="mt-8 flex justify-end">
          <Button type="submit" size="lg" :disabled="saving">
            <Loader2 v-if="saving" class="animate-spin" />
            {{ $t("setup.submit") }}
          </Button>
        </div>
      </form>
    </div>
  </div>
</template>
