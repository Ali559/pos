<script setup lang="ts">
import { computed, ref } from "vue";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import EmployeesTab from "@/components/staff/EmployeesTab.vue";
import ScheduleTab from "@/components/staff/ScheduleTab.vue";
import RolesTab from "@/components/staff/RolesTab.vue";
import { useAuth } from "@/stores/auth";

const auth = useAuth();
const canManageRoles = computed(() => auth.can("roles.manage"));
const tab = ref("employees");
</script>

<template>
  <div class="mx-auto max-w-7xl p-6">
    <Tabs v-model="tab">
      <TabsList class="mb-6">
        <TabsTrigger value="employees">{{ $t("staff.tabs.employees") }}</TabsTrigger>
        <TabsTrigger value="schedule">{{ $t("staff.tabs.schedule") }}</TabsTrigger>
        <TabsTrigger v-if="canManageRoles" value="roles">{{ $t("staff.tabs.roles") }}</TabsTrigger>
      </TabsList>
      <TabsContent value="employees"><EmployeesTab /></TabsContent>
      <TabsContent value="schedule"><ScheduleTab /></TabsContent>
      <TabsContent v-if="canManageRoles" value="roles"><RolesTab /></TabsContent>
    </Tabs>
  </div>
</template>
