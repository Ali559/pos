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
import { getSetting, setSetting } from "@/db/settings";

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

// Alerts
const warnDays = ref(7);
const osAlerts = ref(true);
const alertsSaved = ref(false);
const alertsError = ref("");
const saveAlerts = async () => {
  alertsError.value = "";
  try {
    await setSetting("expiry_warn_days", String(Math.max(0, Math.min(90, Math.round(warnDays.value)))));
    await setSetting("os_notifications", osAlerts.value ? "1" : "0");
    await call("refresh_notifications");
    flash(alertsSaved);
  } catch (e) {
    alertsError.value = errorMessage(e);
  }
};

// Register: receipts, drawer, barcode lookup
const register = ref({ receiptWidth: "80", autoPrint: false, openingFloat: 0, barcodeLookup: true });
const registerSaved = ref(false);
const registerError = ref("");
const saveRegister = async () => {
  registerError.value = "";
  try {
    const r = register.value;
    await setSetting("receipt_width", r.receiptWidth);
    await setSetting("auto_print", r.autoPrint ? "1" : "0");
    await setSetting("opening_float", String(Math.max(0, Number(r.openingFloat) || 0)));
    await setSetting("barcode_lookup", r.barcodeLookup ? "1" : "0");
    flash(registerSaved);
  } catch (e) {
    registerError.value = errorMessage(e);
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
  warnDays.value = Number(await getSetting("expiry_warn_days")) || 7;
  osAlerts.value = (await getSetting("os_notifications")) !== "0";
  const [width, auto, float, lookup] = await Promise.all(
    ["receipt_width", "auto_print", "opening_float", "barcode_lookup"].map(getSetting),
  );
  register.value = {
    receiptWidth: width === "58" ? "58" : "80",
    autoPrint: auto === "1",
    openingFloat: Number(float) || 0,
    barcodeLookup: lookup !== "0",
  };
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

        <Card v-if="canSystem">
          <CardHeader>
            <CardTitle>{{ $t("settings.register.title") }}</CardTitle>
          </CardHeader>
          <CardContent>
            <form class="space-y-4" @submit.prevent="saveRegister">
              <div class="flex flex-wrap gap-6">
                <div class="space-y-2">
                  <Label>{{ $t("settings.register.receiptWidth") }}</Label>
                  <Select v-model="register.receiptWidth" :key="$i18n.locale">
                    <SelectTrigger class="w-40"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="80">80 mm</SelectItem>
                      <SelectItem value="58">58 mm</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div class="space-y-2">
                  <Label for="openingFloat">{{ $t("settings.register.openingFloat") }}</Label>
                  <Input id="openingFloat" v-model.number="register.openingFloat" type="number" min="0" step="any" dir="ltr" class="w-40" />
                </div>
              </div>
              <p class="-mt-2 text-xs text-muted-foreground">{{ $t("settings.register.openingFloatHint") }}</p>
              <label class="flex items-center gap-3 text-sm">
                <input v-model="register.autoPrint" type="checkbox" class="size-4 accent-primary" />
                <span>
                  <span class="font-medium">{{ $t("settings.register.autoPrint") }}</span>
                  <span class="block text-muted-foreground">{{ $t("settings.register.autoPrintHint") }}</span>
                </span>
              </label>
              <label class="flex items-center gap-3 text-sm">
                <input v-model="register.barcodeLookup" type="checkbox" class="size-4 accent-primary" />
                <span>
                  <span class="font-medium">{{ $t("settings.register.barcodeLookup") }}</span>
                  <span class="block text-muted-foreground">{{ $t("settings.register.barcodeLookupHint") }}</span>
                </span>
              </label>
              <div class="flex items-center gap-3">
                <Button type="submit">{{ $t("common.save") }}</Button>
                <span v-if="registerSaved" class="text-sm text-success">{{ $t("common.saved") }}</span>
                <span v-if="registerError" role="alert" class="text-sm text-destructive">{{ registerError }}</span>
              </div>
            </form>
          </CardContent>
        </Card>

        <Card v-if="canSystem">
          <CardHeader>
            <CardTitle>{{ $t("settings.alerts.title") }}</CardTitle>
          </CardHeader>
          <CardContent>
            <form class="space-y-4" @submit.prevent="saveAlerts">
              <div class="space-y-2">
                <Label for="warnDays">{{ $t("settings.alerts.warnDays") }}</Label>
                <Input id="warnDays" v-model.number="warnDays" type="number" min="0" max="90" dir="ltr" class="w-28" />
                <p class="text-xs text-muted-foreground">{{ $t("settings.alerts.warnDaysHint") }}</p>
              </div>
              <label class="flex items-center gap-3 text-sm">
                <input v-model="osAlerts" type="checkbox" class="size-4 accent-primary" />
                <span>
                  <span class="font-medium">{{ $t("settings.alerts.os") }}</span>
                  <span class="block text-muted-foreground">{{ $t("settings.alerts.osHint") }}</span>
                </span>
              </label>
              <div class="flex items-center gap-3">
                <Button type="submit">{{ $t("common.save") }}</Button>
                <span v-if="alertsSaved" class="text-sm text-success">{{ $t("common.saved") }}</span>
                <span v-if="alertsError" role="alert" class="text-sm text-destructive">{{ alertsError }}</span>
              </div>
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
