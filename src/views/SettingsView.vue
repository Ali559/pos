<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
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
import BackupsTab from "@/components/backup/BackupsTab.vue";
import { useTheme } from "@/hooks/useTheme";
import { useShop } from "@/stores/shop";
import { useAuth } from "@/stores/auth";
import { call } from "@/lib/ipc";
import { errorMessage } from "@/lib/errors";
import type { ShopProfile } from "@/db/shop";
import type { MediaFile } from "@/lib/media";
import { getSetting, setSetting } from "@/db/settings";
import { printTestPage } from "@/lib/print";

const { t } = useI18n();
const { theme } = useTheme();
const shop = useShop();
const auth = useAuth();

// The theme is per device, so anyone may change it; the rest is gated
const canSystem = computed(() => auth.can("settings.system"));
const canShop = computed(() => auth.can("settings.shop"));
// ?tab=backups opens a tab directly (e.g. from a backup alert)
const route = useRoute();
const activeTab = ref(typeof route.query.tab === "string" ? route.query.tab : "general");

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
const register = ref({ receiptWidth: "80", autoPrint: false, openingFloat: 0, barcodeLookup: true, blockExpired: false, fullscreen: false });

// Receipt printer: the system dialog, an installed printer, or a network one
const printerMode = ref<"dialog" | "system" | "network">("dialog");
const systemPrinter = ref("");
const networkPrinter = ref("");
const printers = ref<{ systemName: string; name: string }[]>([]);
const printerTarget = computed(() =>
  printerMode.value === "system" && systemPrinter.value
    ? `system:${systemPrinter.value}`
    : printerMode.value === "network" && networkPrinter.value.trim()
      ? `tcp:${networkPrinter.value.trim().includes(":") ? networkPrinter.value.trim() : `${networkPrinter.value.trim()}:9100`}`
      : "",
);
const testMessage = ref("");
const testPrint = async () => {
  testMessage.value = "";
  try {
    await printTestPage(printerTarget.value);
    testMessage.value = t("settings.register.testSent");
  } catch (e) {
    testMessage.value = errorMessage(e);
  }
};
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
    await setSetting("block_expired", r.blockExpired ? "1" : "0");
    await setSetting("fullscreen", r.fullscreen ? "1" : "0");
    await setSetting("receipt_printer", printerTarget.value);
    flash(registerSaved);
  } catch (e) {
    registerError.value = errorMessage(e);
  }
};

// Scale labels: prefix + item code + weight or price + check digit
const scale = ref({ prefix: "20", codeLength: "5", mode: "weight", decimals: "3" });
const scaleSaved = ref(false);
const scaleError = ref("");
const scaleExample = computed(() => {
  const p = scale.value.prefix.trim();
  const code = Number(scale.value.codeLength) || 0;
  const rest = 12 - p.length - code;
  if (!p || rest < 4) return "";
  return `${p} ${"C".repeat(code)} ${"V".repeat(rest)} ✓`;
});
const saveScale = async () => {
  scaleError.value = "";
  try {
    const sc = scale.value;
    // Off first, so the layout is never checked half-changed
    await setSetting("scale_prefix", "");
    await setSetting("scale_code_length", sc.codeLength);
    await setSetting("scale_mode", sc.mode);
    await setSetting("scale_decimals", sc.decimals);
    await setSetting("scale_prefix", sc.prefix.trim());
    flash(scaleSaved);
  } catch (e) {
    scaleError.value = errorMessage(e);
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
  const [width, auto, float, lookup, blockExpired, printer, fullscreen] = await Promise.all(
    ["receipt_width", "auto_print", "opening_float", "barcode_lookup", "block_expired", "receipt_printer", "fullscreen"].map(getSetting),
  );
  register.value = {
    receiptWidth: width === "58" ? "58" : "80",
    autoPrint: auto === "1",
    openingFloat: Number(float) || 0,
    barcodeLookup: lookup !== "0",
    blockExpired: blockExpired === "1",
    fullscreen: fullscreen === "1",
  };
  if (printer?.startsWith("system:")) {
    printerMode.value = "system";
    systemPrinter.value = printer.slice(7);
  } else if (printer?.startsWith("tcp:")) {
    printerMode.value = "network";
    networkPrinter.value = printer.slice(4);
  }
  const [sp, sl, sm, sd] = await Promise.all(["scale_prefix", "scale_code_length", "scale_mode", "scale_decimals"].map(getSetting));
  scale.value = { prefix: sp ?? "20", codeLength: sl ?? "5", mode: sm === "price" ? "price" : "weight", decimals: sd ?? "3" };
  if (canSystem.value) printers.value = await call<typeof printers.value>("list_printers").catch(() => []);
});
</script>

<template>
  <div class="mx-auto max-w-5xl p-6">
    <Tabs v-model="activeTab" class="w-full">
      <TabsList class="mb-6">
        <TabsTrigger value="general">{{ $t("settings.tabs.general") }}</TabsTrigger>
        <TabsTrigger v-if="canShop" value="shop">{{ $t("settings.tabs.shop") }}</TabsTrigger>
        <TabsTrigger v-if="canSystem" value="backups">{{ $t("settings.tabs.backups") }}</TabsTrigger>
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
              <fieldset class="space-y-2">
                <legend class="text-sm font-medium">{{ $t("settings.register.printer") }}</legend>
                <div class="flex flex-wrap items-center gap-2">
                  <Select v-model="printerMode" :key="$i18n.locale">
                    <SelectTrigger class="w-56"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="dialog">{{ $t("settings.register.printerDialog") }}</SelectItem>
                      <SelectItem value="system">{{ $t("settings.register.printerSystem") }}</SelectItem>
                      <SelectItem value="network">{{ $t("settings.register.printerNetwork") }}</SelectItem>
                    </SelectContent>
                  </Select>
                  <Select v-if="printerMode === 'system'" v-model="systemPrinter">
                    <SelectTrigger class="w-64"><SelectValue :placeholder="$t('settings.register.pickPrinter')" /></SelectTrigger>
                    <SelectContent>
                      <SelectItem v-for="p in printers" :key="p.systemName" :value="p.systemName">{{ p.name }}</SelectItem>
                    </SelectContent>
                  </Select>
                  <Input
                    v-if="printerMode === 'network'"
                    v-model="networkPrinter"
                    dir="ltr"
                    class="w-56"
                    placeholder="192.168.1.50:9100"
                    :aria-label="$t('settings.register.printerAddress')"
                  />
                  <Button v-if="printerTarget" type="button" variant="outline" @click="testPrint">{{ $t("settings.register.testPrint") }}</Button>
                </div>
                <p class="text-xs text-muted-foreground">
                  {{ printerMode === "dialog" ? $t("settings.register.printerDialogHint") : $t("settings.register.printerDirectHint") }}
                </p>
                <p v-if="printerMode === 'system' && !printers.length" class="text-xs text-muted-foreground">{{ $t("settings.register.noPrinters") }}</p>
                <p v-if="testMessage" role="status" class="text-sm">{{ testMessage }}</p>
              </fieldset>
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
              <label class="flex items-center gap-3 text-sm">
                <input v-model="register.fullscreen" type="checkbox" class="size-4 accent-primary" />
                <span>
                  <span class="font-medium">{{ $t("settings.register.fullscreen") }}</span>
                  <span class="block text-muted-foreground">{{ $t("settings.register.fullscreenHint") }}</span>
                </span>
              </label>
              <label class="flex items-center gap-3 text-sm">
                <input v-model="register.blockExpired" type="checkbox" class="size-4 accent-primary" />
                <span>
                  <span class="font-medium">{{ $t("settings.register.blockExpired") }}</span>
                  <span class="block text-muted-foreground">{{ $t("settings.register.blockExpiredHint") }}</span>
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
            <CardTitle>{{ $t("settings.scale.title") }}</CardTitle>
          </CardHeader>
          <CardContent>
            <form class="space-y-4" @submit.prevent="saveScale">
              <p class="text-sm text-muted-foreground">{{ $t("settings.scale.hint") }}</p>
              <div class="flex flex-wrap gap-4">
                <div class="space-y-2">
                  <Label for="scalePrefix">{{ $t("settings.scale.prefix") }}</Label>
                  <Input id="scalePrefix" v-model="scale.prefix" dir="ltr" inputmode="numeric" maxlength="3" class="w-24" />
                </div>
                <div class="space-y-2">
                  <Label for="scaleCode">{{ $t("settings.scale.codeLength") }}</Label>
                  <Input id="scaleCode" v-model="scale.codeLength" type="number" min="3" max="6" dir="ltr" class="w-24" />
                </div>
                <div class="space-y-2">
                  <Label>{{ $t("settings.scale.mode") }}</Label>
                  <Select v-model="scale.mode" :key="$i18n.locale">
                    <SelectTrigger class="w-44"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="weight">{{ $t("settings.scale.modeWeight") }}</SelectItem>
                      <SelectItem value="price">{{ $t("settings.scale.modePrice") }}</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div class="space-y-2">
                  <Label for="scaleDec">{{ $t("settings.scale.decimals") }}</Label>
                  <Input id="scaleDec" v-model="scale.decimals" type="number" min="0" max="3" dir="ltr" class="w-24" />
                </div>
              </div>
              <p class="text-xs text-muted-foreground">
                {{ scale.prefix.trim() ? $t("settings.scale.decimalsHint") : $t("settings.scale.off") }}
                <span v-if="scaleExample" dir="ltr" class="num ms-1 rounded bg-muted px-1.5 py-0.5 font-mono">{{ scaleExample }}</span>
              </p>
              <div class="flex items-center gap-3">
                <Button type="submit">{{ $t("common.save") }}</Button>
                <span v-if="scaleSaved" class="text-sm text-success">{{ $t("common.saved") }}</span>
                <span v-if="scaleError" role="alert" class="text-sm text-destructive">{{ scaleError }}</span>
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

      <TabsContent v-if="canSystem" value="backups">
        <BackupsTab />
      </TabsContent>
    </Tabs>
  </div>
</template>
