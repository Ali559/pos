<template>
  <div class="p-6">
    <div class="max-w-5xl mx-auto">
      <Tabs v-model="activeTab" class="w-full">
        <TabsList class="mb-6">
          <TabsTrigger value="general" class="">
            {{ $t("settings.tabs.general") }}
          </TabsTrigger>
          <TabsTrigger value="shop">
            {{ $t("settings.tabs.shop") }}
          </TabsTrigger>
          <TabsTrigger value="roles" class="">
            {{ $t("settings.tabs.roles") }}
          </TabsTrigger>
          <TabsTrigger value="users" class="">
            {{ $t("settings.tabs.users") }}
          </TabsTrigger>
        </TabsList>

        <!-- General Tab -->
        <TabsContent value="general" class="space-y-5">
          <Card>
            <CardHeader>
              <CardTitle>{{ $t("settings.general.appearance") }}</CardTitle>
            </CardHeader>
            <CardContent class="space-y-2">
              <Label>{{ $t("settings.general.theme") }}</Label>
              <Select v-model="theme" :key="$i18n.locale">
                <SelectTrigger class="w-full max-w-xs">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="light">
                    {{ $t("settings.general.light") }}
                  </SelectItem>
                  <SelectItem value="dark">
                    {{ $t("settings.general.dark") }}
                  </SelectItem>
                  <SelectItem value="system">
                    {{ $t("settings.general.system") }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>{{ $t("settings.general.sales") }}</CardTitle>
            </CardHeader>
            <CardContent>
              <form class="flex items-end gap-3" @submit.prevent="saveTax">
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
                    <span
                      class="absolute end-3 top-1/2 -translate-y-1/2 text-muted-foreground"
                      >%</span
                    >
                  </div>
                </div>
                <Button type="submit" :disabled="taxDraft === shop.taxRate.value">
                  {{ $t("common.save") }}
                </Button>
                <span v-if="taxSaved" class="pb-2.5 text-sm text-success">
                  {{ $t("common.saved") }}
                </span>
              </form>
            </CardContent>
          </Card>

          <Card v-if="showSampleData">
            <CardHeader>
              <CardTitle>{{ $t("settings.general.sampleData") }}</CardTitle>
            </CardHeader>
            <CardContent class="flex items-center gap-4">
              <p class="flex-1 text-sm text-muted-foreground">
                {{ $t("settings.general.sampleDataHint") }}
              </p>
              <Button variant="outline" @click="loadSampleData">
                {{ $t("settings.general.loadSampleData") }}
              </Button>
            </CardContent>
            <CardContent v-if="sampleMessage" class="-mt-2 text-sm" role="status">
              {{ sampleMessage }}
            </CardContent>
          </Card>
        </TabsContent>

        <!-- Shop Tab -->
        <TabsContent value="shop">
          <form v-if="shopForm" @submit.prevent="saveShop">
            <Card>
              <CardContent>
                <ShopForm v-model="shopForm" v-model:logo="shopLogo" />
              </CardContent>
              <CardContent class="flex items-center gap-3 border-t pt-5">
                <Button type="submit" :disabled="shopSaving">
                  {{ shopSaving ? $t("common.saving") : $t("common.save") }}
                </Button>
                <span v-if="shopSaved" class="text-sm text-success">
                  {{ $t("common.saved") }}
                </span>
                <span v-if="shopError" role="alert" class="text-sm text-destructive">
                  {{ shopError }}
                </span>
              </CardContent>
            </Card>
          </form>
        </TabsContent>

        <!-- Roles Tab -->
        <TabsContent value="roles">
          <Card>
            <CardHeader
              class="flex flex-row items-center justify-between"
            >
              <CardTitle class="">
                {{ $t("settings.roles.title") }}
              </CardTitle>
              <Button @click="openRoleModal">
                <Plus />
                {{ $t("settings.roles.addRole") }}
              </Button>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>
                      {{ $t("settings.roles.role") }}
                    </TableHead>
                    <TableHead class="">
                      {{ $t("settings.roles.description") }}
                    </TableHead>
                    <TableHead class="text-end">
                      {{ $t("settings.roles.users") }}
                    </TableHead>
                    <TableHead class="text-center">
                      {{ $t("settings.actions") }}
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow v-for="role in roles" :key="role.id">
                    <TableCell class="font-medium">
                      {{ role.name }}
                    </TableCell>
                    <TableCell class="">
                      {{ role.description }}
                    </TableCell>
                    <TableCell class="text-end">
                      {{ role.userCount }}
                    </TableCell>
                    <TableCell class="text-center">
                      <div class="flex justify-center gap-2">
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          :aria-label="$t('settings.edit')"
                        >
                          <Pencil />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          class="text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                          :aria-label="$t('settings.delete')"
                        >
                          <Trash2 />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        </TabsContent>

        <!-- Users Tab -->
        <TabsContent value="users">
          <Card>
            <CardHeader class="flex flex-row items-center justify-between">
              <CardTitle class="">
                {{ $t("settings.users.title") }}
              </CardTitle>
              <Button @click="openUserModal">
                <Plus />
                {{ $t("settings.users.addUser") }}
              </Button>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead class="">
                      {{ $t("settings.users.name") }}
                    </TableHead>
                    <TableHead class="">
                      {{ $t("settings.users.username") }}
                    </TableHead>
                    <TableHead class="">
                      {{ $t("settings.users.role") }}
                    </TableHead>
                    <TableHead class="text-center">
                      {{ $t("settings.users.status") }}
                    </TableHead>
                    <TableHead class="text-center">
                      {{ $t("settings.actions") }}
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow v-for="user in users" :key="user.id">
                    <TableCell class="font-medium">
                      {{ user.name }}
                    </TableCell>
                    <TableCell class="">
                      {{ user.username }}
                    </TableCell>
                    <TableCell class="">
                      <Badge variant="secondary">
                        {{ user.role }}
                      </Badge>
                    </TableCell>
                    <TableCell class="text-center">
                      <Badge
                        :variant="
                          user.status === 'active' ? 'default' : 'outline'
                        "
                        :class="
                          user.status === 'active'
                            ? 'bg-success/12 text-success'
                            : 'text-muted-foreground'
                        "
                      >
                        {{
                          user.status === "active"
                            ? $t("settings.users.active")
                            : $t("settings.users.inactive")
                        }}
                      </Badge>
                    </TableCell>
                    <TableCell class="text-center">
                      <div class="flex justify-center gap-2">
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          :aria-label="$t('settings.edit')"
                        >
                          <Pencil />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          class="text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                          :aria-label="$t('settings.delete')"
                        >
                          <Trash2 />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { Pencil, Plus, Trash2 } from "lucide-vue-next";
import { useTheme } from "@/hooks/useTheme";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import ShopForm from "@/components/shop/ShopForm.vue";
import { useShop } from "@/stores/shop";
import { inDesktopApp } from "@/db/client";
import type { ShopProfile } from "@/db/shop";
import type { MediaFile } from "@/lib/media";

const { t } = useI18n();

const activeTab = ref("general");
const { theme } = useTheme();
const shop = useShop();

// Tax rate
const taxDraft = ref(0);
const taxSaved = ref(false);
const saveTax = async () => {
  await shop.saveTaxRate(Math.min(100, Math.max(0, Number(taxDraft.value) || 0)));
  taxDraft.value = shop.taxRate.value;
  taxSaved.value = true;
  setTimeout(() => (taxSaved.value = false), 2000);
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
    shopSaved.value = true;
    setTimeout(() => (shopSaved.value = false), 2000);
  } catch (e) {
    shopError.value = String(e);
  } finally {
    shopSaving.value = false;
  }
};

// Sample data: dev builds of the desktop app only (the browser preview seeds itself)
const showSampleData = import.meta.env.DEV && inDesktopApp;
const sampleMessage = ref("");
const loadSampleData = async () => {
  try {
    await invoke("seed_demo_data");
    sampleMessage.value = t("settings.general.sampleDataLoaded");
  } catch (e) {
    sampleMessage.value = String(e);
  }
};

onMounted(async () => {
  await shop.load();
  taxDraft.value = shop.taxRate.value;
  resetShopForm();
});

const roles = ref([
  {
    id: 1,
    name: "Administrator",
    description: "Full system access",
    userCount: 2,
  },
  {
    id: 2,
    name: "Manager",
    description: "Manage products and view reports",
    userCount: 3,
  },
  { id: 3, name: "Cashier", description: "Process sales only", userCount: 5 },
]);

const users = ref([
  {
    id: 1,
    name: "John Doe",
    username: "john",
    role: "Administrator",
    status: "active",
  },
  {
    id: 2,
    name: "Jane Smith",
    username: "jane",
    role: "Manager",
    status: "active",
  },
  {
    id: 3,
    name: "Bob Johnson",
    username: "bob",
    role: "Cashier",
    status: "inactive",
  },
]);


const openRoleModal = () => {
  console.log("Open role modal");
};

const openUserModal = () => {
  console.log("Open user modal");
};
</script>
