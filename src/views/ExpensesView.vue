<script setup lang="ts">
import { ref } from "vue";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import ExpensesTab from "@/components/expenses/ExpensesTab.vue";
import ExpenseTypesTab from "@/components/expenses/ExpenseTypesTab.vue";

const tab = ref("expenses");
</script>

<template>
  <div class="mx-auto max-w-7xl p-6">
    <Tabs v-model="tab">
      <TabsList class="mb-6">
        <TabsTrigger value="expenses">{{ $t("expenses.tabs.expenses") }}</TabsTrigger>
        <TabsTrigger value="types">{{ $t("expenses.tabs.types") }}</TabsTrigger>
      </TabsList>
      <TabsContent value="expenses"><ExpensesTab /></TabsContent>
      <TabsContent value="types"><ExpenseTypesTab /></TabsContent>
    </Tabs>
  </div>
</template>
