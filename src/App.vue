<script setup lang="ts">
import LocaleProvider from "./components/providers/LocaleProvider.vue";
</script>

<template>
  <locale-provider>
    <router-view />
  </locale-provider>
</template>
