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

// `compact` is the first-run version: identity and currency only (setup
// asks for the shop type separately, next to the category checklist)
defineProps<{ compact?: boolean }>();

const form = defineModel<ShopProfile>({ required: true });
const logo = defineModel<MediaFile | null>("logo", { required: true });

const CURRENCIES = ["IQD", "USD"] as const;
const SHOP_TYPES = ["mini_market", "supermarket", "other"] as const;

const logoModel = computed({
  get: () => logo.value,
  set: (m) => {
    logo.value = m;
    form.value.logo_media_id = m?.id ?? null;
  },
});
</script>

<template>
  <div class="space-y-8">
    <section class="space-y-5">
      <h3 v-if="!compact" class="font-semibold">{{ $t("shop.identity") }}</h3>
      <div class="space-y-2">
        <Label>{{ $t("shop.logo") }}</Label>
        <LogoField v-model="logoModel" />
      </div>
      <div class="grid gap-4 md:grid-cols-2">
        <div class="space-y-2">
          <Label for="shop-name">{{ $t("shop.name") }}</Label>
          <Input id="shop-name" v-model="form.name" required dir="auto" />
        </div>
        <div v-if="!compact" class="space-y-2">
          <Label>{{ $t("shop.type") }}</Label>
          <Select v-model="form.shop_type" :key="$i18n.locale">
            <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
            <SelectContent>
              <SelectItem v-for="t in SHOP_TYPES" :key="t" :value="t">{{ $t(`shop.types.${t}`) }}</SelectItem>
            </SelectContent>
          </Select>
        </div>
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
        <div class="grid gap-4 md:grid-cols-2">
          <div class="space-y-2">
            <Label for="phone_alt">{{ $t("shop.phoneAlt") }}</Label>
            <Input id="phone_alt" v-model="form.phone_alt as string" type="tel" dir="ltr" />
          </div>
          <div class="space-y-2">
            <Label for="tax_number">{{ $t("shop.taxNumber") }}</Label>
            <Input id="tax_number" v-model="form.tax_number as string" dir="ltr" />
          </div>
          <div class="space-y-2 md:col-span-2">
            <Label for="address">{{ $t("shop.address") }}</Label>
            <Input id="address" v-model="form.address as string" dir="auto" />
          </div>
        </div>
      </section>

      <section class="space-y-5">
        <h3 class="font-semibold">{{ $t("shop.receipt") }}</h3>
        <div class="space-y-2">
          <Label for="receipt_footer">{{ $t("shop.footer") }}</Label>
          <textarea
            id="receipt_footer"
            v-model="form.receipt_footer as string"
            dir="auto"
            rows="3"
            class="w-full resize-none rounded-md border border-input bg-card px-3 py-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
          />
          <p class="text-xs text-muted-foreground">{{ $t("shop.footerHint") }}</p>
        </div>
      </section>
    </template>
  </div>
</template>
