<script setup lang="ts">
import Header from "@/components/menus/Header.vue";
import Sidebar from "@/components/menus/Sidebar.vue";
import { ref } from "vue";

const menuOpen = ref<boolean>(false);
</script>

<template>
  <Header page-title="" @toggle-menu="menuOpen = !menuOpen" />
  <!-- Overlay -->
  <div
    v-if="menuOpen"
    class="absolute top-0 h-full w-full bg-gray-100/10 backdrop-blur-sm z-40"
    @click="menuOpen = false"
  />
  <Sidebar :is-open="menuOpen" @close="menuOpen = false" />
  <main>
    <slot></slot>
  </main>
  <router-view></router-view>
</template>
