<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import ShopForm from "@/components/shop/ShopForm.vue";
import { useTheme } from "@/hooks/useTheme";
import { useShop } from "@/stores/shop";
import { useAuth } from "@/stores/auth";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import type { ShopProfile } from "@/db/shop";
import type { MediaFile } from "@/lib/media";

const { t } = useI18n();
const { theme } = useTheme();
const shop = useShop();
const auth = useAuth();

// The theme is per device, so anyone may change it; the rest is gated
const canSystem = computed(() => auth.can("settings.system"));
const canShop = computed(() => auth.can("settings.shop"));
const activeTab = ref("general");

const flash = (r: { value: boolean }) => {
  r.value = true;
  setTimeout(() => (r.value = false), 2000);
};

// Tax rate
const taxDraft = ref(0);
const taxSaved = ref(false);
const taxError = ref("");
const saveTax = async () => {
  taxError.value = "";
  try {
    await shop.saveTaxRate(Math.min(100, Math.max(0, Number(taxDraft.value) || 0)));
    taxDraft.value = shop.taxRate.value;
    flash(taxSaved);
  } catch (e) {
    taxError.value = errorMessage(e);
  }
};

// Shop profile
const shopForm = ref<ShopProfile | null>(null);
const shopLogo = ref<MediaFile | null>(null);
const shopSaving = ref(false);
const shopSaved = ref(false);
const shopError = ref("");

const resetShopForm = () => {
  const p = shop.profile.value;
  if (!p) return;
  const { logo, ...fields } = p;
  shopForm.value = { ...fields };
  shopLogo.value = logo;
};

const saveShop = async () => {
  if (!shopForm.value) return;
  shopSaving.value = true;
  shopError.value = "";
  try {
    await shop.save(shopForm.value);
    resetShopForm();
    flash(shopSaved);
  } catch (e) {
    shopError.value = errorMessage(e);
  } finally {
    shopSaving.value = false;
  }
};

// Sample data: dev builds only (Rust refuses in release builds too)
const showSampleData = computed(() => import.meta.env.DEV && canSystem.value);
const sampleMessage = ref("");
const loadSampleData = async () => {
  try {
    await call("seed_demo_data");
    sampleMessage.value = t("settings.general.sampleDataLoaded");
  } catch (e) {
    sampleMessage.value = errorMessage(e);
  }
};

onMounted(async () => {
  await shop.load();
  taxDraft.value = shop.taxRate.value;
  resetShopForm();
});
</script>

<template>
  <div class="mx-auto max-w-5xl p-6">
    <Tabs v-model="activeTab" class="w-full">
      <TabsList class="mb-6">
        <TabsTrigger value="general">{{ $t("settings.tabs.general") }}</TabsTrigger>
        <TabsTrigger v-if="canShop" value="shop">{{ $t("settings.tabs.shop") }}</TabsTrigger>
      </TabsList>

      <TabsContent value="general" class="space-y-5">
        <Card>
          <CardHeader>
            <CardTitle>{{ $t("settings.general.appearance") }}</CardTitle>
          </CardHeader>
          <CardContent class="space-y-2">
            <Label>{{ $t("settings.general.theme") }}</Label>
            <Select v-model="theme" :key="$i18n.locale">
              <SelectTrigger class="w-full max-w-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="light">{{ $t("settings.general.light") }}</SelectItem>
                <SelectItem value="dark">{{ $t("settings.general.dark") }}</SelectItem>
                <SelectItem value="system">{{ $t("settings.general.system") }}</SelectItem>
              </SelectContent>
            </Select>
          </CardContent>
        </Card>

        <Card v-if="canSystem">
          <CardHeader>
            <CardTitle>{{ $t("settings.general.sales") }}</CardTitle>
          </CardHeader>
          <CardContent>
            <form class="flex flex-wrap items-end gap-3" @submit.prevent="saveTax">
              <div class="space-y-2">
                <Label for="taxRate">{{ $t("settings.general.taxRate") }}</Label>
                <div class="relative w-40">
                  <Input
                    id="taxRate"
                    v-model.number="taxDraft"
                    type="number"
                    min="0"
                    max="100"
                    step="0.5"
                    dir="ltr"
                    class="pe-9"
                  />
                  <span class="absolute end-3 top-1/2 -translate-y-1/2 text-muted-foreground">%</span>
                </div>
              </div>
              <Button type="submit" :disabled="taxDraft === shop.taxRate.value">
                {{ $t("common.save") }}
              </Button>
              <span v-if="taxSaved" class="pb-2.5 text-sm text-success">{{ $t("common.saved") }}</span>
              <span v-if="taxError" role="alert" class="pb-2.5 text-sm text-destructive">{{ taxError }}</span>
            </form>
          </CardContent>
        </Card>

        <Card v-if="showSampleData">
          <CardHeader>
            <CardTitle>{{ $t("settings.general.sampleData") }}</CardTitle>
          </CardHeader>
          <CardContent class="flex items-center gap-4">
            <p class="flex-1 text-sm text-muted-foreground">{{ $t("settings.general.sampleDataHint") }}</p>
            <Button variant="outline" @click="loadSampleData">
              {{ $t("settings.general.loadSampleData") }}
            </Button>
          </CardContent>
          <CardContent v-if="sampleMessage" class="-mt-2 text-sm" role="status">
            {{ sampleMessage }}
          </CardContent>
        </Card>
      </TabsContent>

      <TabsContent v-if="canShop" value="shop">
        <form v-if="shopForm" @submit.prevent="saveShop">
          <Card>
            <CardContent>
              <ShopForm v-model="shopForm" v-model:logo="shopLogo" />
            </CardContent>
            <CardContent class="flex items-center gap-3 border-t pt-5">
              <Button type="submit" :disabled="shopSaving">
                {{ shopSaving ? $t("common.saving") : $t("common.save") }}
              </Button>
              <span v-if="shopSaved" class="text-sm text-success">{{ $t("common.saved") }}</span>
              <span v-if="shopError" role="alert" class="text-sm text-destructive">{{ shopError }}</span>
            </CardContent>
          </Card>
        </form>
      </TabsContent>
    </Tabs>
  </div>
</template>
