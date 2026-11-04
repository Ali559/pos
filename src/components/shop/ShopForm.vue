<script setup lang="ts">
import { computed } from "vue";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import LogoField from "./LogoField.vue";
import type { ShopProfile } from "@/db/shop";
import type { MediaFile } from "@/lib/media";

// `compact` is the first-run version: identity and currency only
defineProps<{ compact?: boolean }>();

const form = defineModel<ShopProfile>({ required: true });
const logo = defineModel<MediaFile | null>("logo", { required: true });

const CURRENCIES = ["IQD", "USD"] as const;

const logoModel = computed({
  get: () => logo.value,
  set: (m) => {
    logo.value = m;
    form.value.logo_media_id = m?.id ?? null;
  },
});

// Inputs for text that's always in one script get that script's direction,
// whatever the UI language is
const fields = {
  names: [
    { key: "name_en", label: "shop.nameEn", lang: "en", required: true },
    { key: "name_ku", label: "shop.nameKu", lang: "ku", required: true },
    { key: "name_ar", label: "shop.nameAr", lang: "ar", required: false },
  ],
  addresses: [
    { key: "address_en", label: "shop.addressEn", lang: "en" },
    { key: "address_ku", label: "shop.addressKu", lang: "ku" },
    { key: "address_ar", label: "shop.addressAr", lang: "ar" },
  ],
  footers: [
    { key: "receipt_footer_en", label: "shop.footerEn", lang: "en" },
    { key: "receipt_footer_ku", label: "shop.footerKu", lang: "ku" },
    { key: "receipt_footer_ar", label: "shop.footerAr", lang: "ar" },
  ],
} as const;
const dirOf = (lang: string) => (lang === "en" ? "ltr" : "rtl");
</script>

<template>
  <div class="space-y-8">
    <section class="space-y-5">
      <h3 v-if="!compact" class="font-semibold">{{ $t("shop.identity") }}</h3>
      <div class="space-y-2">
        <Label>{{ $t("shop.logo") }}</Label>
        <LogoField v-model="logoModel" />
      </div>
      <div class="grid items-end gap-4 md:grid-cols-3">
        <div v-for="f in fields.names" :key="f.key" class="space-y-2">
          <Label :for="f.key">
            {{ $t(f.label) }}
            <span v-if="!f.required" class="font-normal text-muted-foreground"
              >({{ $t("common.optional") }})</span
            >
          </Label>
          <Input
            :id="f.key"
            v-model="form[f.key] as string"
            :lang="f.lang"
            :dir="dirOf(f.lang)"
            :required="f.required"
          />
        </div>
      </div>
      <div class="grid gap-4 md:grid-cols-3">
        <div class="space-y-2">
          <Label for="phone">{{ $t("shop.phone") }}</Label>
          <Input id="phone" v-model="form.phone as string" type="tel" dir="ltr" />
        </div>
        <div class="space-y-2">
          <Label>{{ $t("shop.currency") }}</Label>
          <Select v-model="form.currency" :key="$i18n.locale">
            <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem v-for="c in CURRENCIES" :key="c" :value="c">
                {{ $t(`shop.currencies.${c}`) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
    </section>

    <template v-if="!compact">
      <section class="space-y-5">
        <h3 class="font-semibold">{{ $t("shop.contact") }}</h3>
        <div class="grid gap-4 md:grid-cols-3">
          <div class="space-y-2">
            <Label for="phone_alt">{{ $t("shop.phoneAlt") }}</Label>
            <Input id="phone_alt" v-model="form.phone_alt as string" type="tel" dir="ltr" />
          </div>
          <div class="space-y-2">
            <Label for="tax_number">{{ $t("shop.taxNumber") }}</Label>
            <Input id="tax_number" v-model="form.tax_number as string" dir="ltr" />
          </div>
        </div>
        <div class="grid gap-4 md:grid-cols-3">
          <div v-for="f in fields.addresses" :key="f.key" class="space-y-2">
            <Label :for="f.key">{{ $t(f.label) }}</Label>
            <Input
              :id="f.key"
              v-model="form[f.key] as string"
              :lang="f.lang"
              :dir="dirOf(f.lang)"
            />
          </div>
        </div>
      </section>

      <section class="space-y-5">
        <h3 class="font-semibold">{{ $t("shop.receipt") }}</h3>
        <div class="grid gap-4 md:grid-cols-3">
          <div v-for="f in fields.footers" :key="f.key" class="space-y-2">
            <Label :for="f.key">{{ $t(f.label) }}</Label>
            <textarea
              :id="f.key"
              v-model="form[f.key] as string"
              :lang="f.lang"
              :dir="dirOf(f.lang)"
              rows="3"
              class="w-full resize-none rounded-md border border-input bg-card px-3 py-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            />
          </div>
        </div>
      </section>
    </template>
  </div>
</template>
