<script setup lang="ts">
import LocaleProvider from "./components/providers/LocaleProvider.vue";
import PrintRoot from "./components/sales/PrintRoot.vue";
import StatementPrint from "./components/customers/StatementPrint.vue";
import LabelsPrint from "./components/catalog/LabelsPrint.vue";
</script>

<template>
  <locale-provider>
    <router-view />
    <print-root />
    <statement-print />
    <labels-print />
  </locale-provider>
</template>
