<script setup lang="ts">
import LocaleProvider from "./components/providers/LocaleProvider.vue";
import PrintRoot from "./components/sales/PrintRoot.vue";
</script>

<template>
  <locale-provider>
    <router-view />
    <print-root />
  </locale-provider>
</template>
