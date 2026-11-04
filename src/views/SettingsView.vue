<template>
  <div class="p-6">
    <div class="max-w-5xl mx-auto">
      <Tabs v-model="activeTab" class="w-full">
        <TabsList class="mb-6">
          <TabsTrigger value="general" class="">
            {{ $t("settings.tabs.general") }}
          </TabsTrigger>
          <TabsTrigger value="roles" class="">
            {{ $t("settings.tabs.roles") }}
          </TabsTrigger>
          <TabsTrigger value="users" class="">
            {{ $t("settings.tabs.users") }}
          </TabsTrigger>
        </TabsList>

        <!-- General Tab -->
        <TabsContent value="general">
          <Card>
            <CardHeader>
              <CardTitle class="">
                {{ $t("settings.general.title") }}
              </CardTitle>
            </CardHeader>
            <CardContent class="space-y-6">
              <div class="space-y-4">
                <h3 class="font-semibold">
                  {{ $t("settings.general.appearance") }}
                </h3>
                <div class="space-y-2">
                  <Select v-model="theme" :key="$i18n.locale">
                    <SelectTrigger class="w-full max-w-xs">
                      <SelectValue
                        :placeholder="$t('settings.general.selectTheme')"
                      />
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
                </div>
              </div>

              <Separator />

              <div class="space-y-4">
                <h3 class="font-semibold">
                  {{ $t("settings.general.storeInfo") }}
                </h3>
                <div class="grid md:grid-cols-2 gap-4">
                  <div class="space-y-2">
                    <Label for="storeName">
                      {{ $t("settings.general.storeName") }}
                    </Label>
                    <Input
                      id="storeName"
                      v-model="storeName"
                      :placeholder="$t('settings.general.storeNamePlaceholder')"
                      class=""
                    />
                  </div>
                  <div class="space-y-2">
                    <Label for="taxRate">
                      {{ $t("settings.general.taxRate") }}
                    </Label>
                    <div class="relative">
                      <Input
                        id="taxRate"
                        v-model="taxRate"
                        type="number"
                        class="pe-12"
                      />
                      <div
                        class="absolute end-3 top-1/2 -translate-y-1/2 text-muted-foreground"
                      >
                        %
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <Button @click="saveSettings">
                {{ $t("settings.save") }}
              </Button>
            </CardContent>
          </Card>
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
import { ref } from "vue";
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
import { Separator } from "@/components/ui/separator";
import { Badge } from "@/components/ui/badge";
import { Pencil, Plus, Trash2 } from "lucide-vue-next";
import { useTheme } from "@/hooks/useTheme";

const activeTab = ref("general");
const { theme } = useTheme();
const storeName = ref("My Store");
const taxRate = ref(10);

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

const saveSettings = () => {
  console.log("Settings saved:", {
    theme: theme.value,
    storeName: storeName.value,
    taxRate: taxRate.value,
  });
};

const openRoleModal = () => {
  console.log("Open role modal");
};

const openUserModal = () => {
  console.log("Open user modal");
};
</script>
