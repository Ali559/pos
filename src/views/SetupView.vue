<script setup lang="ts">
import { computed, inject, ref, watch, type ComputedRef } from "vue";
import { useRouter } from "vue-router";
import { Check, Loader2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import ShopForm from "@/components/shop/ShopForm.vue";
import PairForm from "@/components/settings/PairForm.vue";
import { MonitorSmartphone } from "lucide-vue-next";
import AppIcon from "@/components/icons/AppIcon.vue";
import { emptyShopProfile, type ShopType } from "@/db/shop";
import { pickName, starterTemplates } from "@/data/starterCategories";
import type { MediaFile } from "@/lib/media";
import { errorMessage } from "@/lib/errors";
import { categoryInk, categoryTint } from "@/lib/categoryColor";
import { useShop } from "@/stores/shop";
import { landingPath, useAuth } from "@/stores/auth";
import { languages, type Language } from "@/utils/i18n";

const router = useRouter();
const shop = useShop();
const auth = useAuth();

// Choosing here switches the UI right away as a preview. It's fixed once
// setup finishes: every name in the shop is written in this language.
const language = inject<ComputedRef<Language>>("language")!;
const setLanguage = inject<(lang: Language) => Promise<void>>("setLanguage")!;
const LANG_ORDER: Language[] = ["ku", "ar", "en"];

// A shop saved by an earlier version (before accounts existed) is reused
const existing = shop.profile.value;
const form = ref(existing ? (({ logo: _, ...p }) => p)(existing) : emptyShopProfile());
const logo = ref<MediaFile | null>(existing?.logo ?? null);

const SHOP_TYPES: ShopType[] = ["mini_market", "supermarket", "other"];
const templates = computed(() => starterTemplates(form.value.shop_type));
const picked = ref(new Set<string>());
// Everything ticked by default whenever the shop type changes
watch(() => form.value.shop_type, () => {
  picked.value = new Set(templates.value.map((t) => t.key));
}, { immediate: true });
const toggle = (key: string) => {
  const next = new Set(picked.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  picked.value = next;
};

const owner = ref({ fullName: "", username: "", password: "", confirm: "", pin: "" });
const saving = ref(false);
const error = ref("");

const submit = async () => {
  error.value = "";
  if (owner.value.password !== owner.value.confirm) {
    error.value = errorMessage({ code: "password_mismatch", message: "" });
    return;
  }
  const lang = language.value;
  const categories = templates.value
    .filter((t) => picked.value.has(t.key))
    .map((t) => ({
      name: pickName(t.names, lang),
      iconKey: t.icon,
      color: t.color ?? null,
      children: (t.children ?? []).map((c) => ({ name: pickName(c.names, lang), iconKey: c.icon, color: null, children: [] })),
    }));
  saving.value = true;
  try {
    const { confirm: _, pin, ...rest } = owner.value;
    await call(lang, categories, { ...rest, pin: pin || null });
    await shop.reload();
    router.replace(landingPath());
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
};

const call = (lang: Language, categories: unknown[], ownerArgs: unknown) =>
  auth.completeSetup({ language: lang, shop: form.value, owner: ownerArgs, categories });
</script>

<template>
  <div class="min-h-full bg-background">
    <div class="mx-auto flex max-w-3xl flex-col p-6 sm:p-10">
      <h1 class="mt-4 text-3xl font-semibold tracking-tight">{{ $t("setup.title") }}</h1>
      <p class="mt-2 max-w-xl text-muted-foreground">{{ $t("setup.subtitle") }}</p>

      <!-- A second till for a shop that's already set up on another computer -->
      <details class="group mt-6 rounded-xl border bg-card p-4">
        <summary class="flex cursor-pointer list-none items-center gap-2 font-medium">
          <MonitorSmartphone :size="18" class="text-muted-foreground" />
          {{ $t("tills.setupQuestion") }}
        </summary>
        <p class="mt-2 mb-4 text-sm text-muted-foreground">{{ $t("tills.setupHint") }}</p>
        <PairForm />
      </details>

      <form class="mt-10 space-y-6" @submit.prevent="submit">
        <!-- 1. Language: one per shop, fixed after setup -->
        <section class="rounded-xl border bg-card p-6">
          <h2 class="font-semibold">{{ $t("setup.languageSection") }}</h2>
          <p class="mt-1 mb-5 text-sm text-muted-foreground">{{ $t("setup.languageHint") }}</p>
          <div class="grid gap-3 sm:grid-cols-3" role="radiogroup" :aria-label="$t('setup.languageSection')">
            <button
              v-for="id in LANG_ORDER"
              :key="id"
              type="button"
              role="radio"
              :aria-checked="language === id"
              :lang="id"
              :dir="languages[id].dir"
              class="relative flex h-20 items-center justify-center rounded-xl border-2 text-xl font-semibold transition-colors hover:border-foreground/30 aria-checked:border-primary aria-checked:bg-primary/5"
              @click="setLanguage(id)"
            >
              {{ languages[id].label }}
              <span v-if="language === id" class="absolute end-2 top-2 grid size-5 place-items-center rounded-full bg-primary text-primary-foreground">
                <Check :size="13" />
              </span>
            </button>
          </div>
        </section>

        <!-- 2. Shop -->
        <section class="rounded-xl border bg-card p-6">
          <h2 class="mb-5 font-semibold">{{ $t("setup.shopSection") }}</h2>
          <ShopForm v-model="form" v-model:logo="logo" compact />
        </section>

        <!-- 3. Shop type and starter categories -->
        <section class="rounded-xl border bg-card p-6">
          <h2 class="font-semibold">{{ $t("setup.typeSection") }}</h2>
          <p class="mt-1 mb-5 text-sm text-muted-foreground">{{ $t("setup.typeHint") }}</p>
          <div class="grid gap-3 sm:grid-cols-3" role="radiogroup" :aria-label="$t('setup.typeSection')">
            <button
              v-for="t in SHOP_TYPES"
              :key="t"
              type="button"
              role="radio"
              :aria-checked="form.shop_type === t"
              class="rounded-xl border-2 p-4 text-start transition-colors hover:border-foreground/30 aria-checked:border-primary aria-checked:bg-primary/5"
              @click="form.shop_type = t"
            >
              <span class="block font-semibold">{{ $t(`shop.types.${t}`) }}</span>
              <span class="mt-1 block text-xs text-muted-foreground">{{ $t(`shop.typeHints.${t}`) }}</span>
            </button>
          </div>

          <div v-if="templates.length" class="mt-6">
            <p class="mb-3 text-sm font-medium">
              {{ $t("setup.categoriesPicked", { n: picked.size, total: templates.length }) }}
            </p>
            <ul class="grid gap-2 sm:grid-cols-2">
              <li v-for="t in templates" :key="t.key">
                <label
                  class="flex cursor-pointer items-start gap-3 rounded-lg border p-3 transition-colors has-checked:border-primary/50 has-checked:bg-primary/5"
                >
                  <input type="checkbox" class="mt-2.5 size-4 accent-primary" :checked="picked.has(t.key)" @change="toggle(t.key)" />
                  <span
                    class="grid size-9 shrink-0 place-items-center rounded-lg"
                    :style="{ background: categoryTint(t.color ?? null), color: categoryInk(t.color ?? null) }"
                  >
                    <AppIcon :icon="t.icon" :size="20" />
                  </span>
                  <span class="min-w-0">
                    <span class="block font-medium">{{ pickName(t.names, language) }}</span>
                    <span v-if="t.children?.length" class="block text-xs text-muted-foreground">
                      {{ t.children.map((c) => pickName(c.names, language)).join("، ") }}
                    </span>
                  </span>
                </label>
              </li>
            </ul>
            <p class="mt-3 text-xs text-muted-foreground">{{ $t("setup.categoriesHint") }}</p>
          </div>
        </section>

        <!-- 4. Owner -->
        <section class="rounded-xl border bg-card p-6">
          <h2 class="font-semibold">{{ $t("setup.ownerSection") }}</h2>
          <p class="mt-1 mb-5 text-sm text-muted-foreground">{{ $t("setup.ownerHint") }}</p>
          <div class="grid gap-4 md:grid-cols-2">
            <div class="space-y-2">
              <Label for="owner-name">{{ $t("staff.fields.fullName") }}</Label>
              <Input id="owner-name" v-model="owner.fullName" required autocomplete="name" dir="auto" />
            </div>
            <div class="space-y-2">
              <Label for="owner-username">{{ $t("login.username") }}</Label>
              <Input id="owner-username" v-model="owner.username" required dir="ltr" autocomplete="username" autocapitalize="off" />
            </div>
            <div class="space-y-2">
              <Label for="owner-password">{{ $t("login.password") }}</Label>
              <Input id="owner-password" v-model="owner.password" type="password" required minlength="6" dir="ltr" autocomplete="new-password" />
              <p class="text-xs text-muted-foreground">{{ $t("staff.passwordHint") }}</p>
            </div>
            <div class="space-y-2">
              <Label for="owner-confirm">{{ $t("staff.fields.confirmPassword") }}</Label>
              <Input id="owner-confirm" v-model="owner.confirm" type="password" required dir="ltr" autocomplete="new-password" />
            </div>
            <div class="space-y-2">
              <Label for="owner-pin">
                {{ $t("staff.fields.pin") }}
                <span class="font-normal text-muted-foreground">({{ $t("common.optional") }})</span>
              </Label>
              <Input id="owner-pin" v-model="owner.pin" type="password" inputmode="numeric" pattern="[0-9]{4,6}" maxlength="6" dir="ltr" autocomplete="off" />
              <p class="text-xs text-muted-foreground">{{ $t("staff.pinHint") }}</p>
            </div>
          </div>
        </section>

        <p v-if="error" role="alert" class="text-sm font-medium text-destructive">{{ error }}</p>
        <div class="flex items-center justify-end gap-4">
          <p class="text-sm text-muted-foreground">{{ $t("setup.languageFinal", { language: languages[language].label }) }}</p>
          <Button type="submit" size="lg" :disabled="saving">
            <Loader2 v-if="saving" class="animate-spin" />
            {{ $t("setup.submit") }}
          </Button>
        </div>
      </form>
    </div>
  </div>
</template>
