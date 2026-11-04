<script setup lang="ts">
import Header from "@/components/menus/Header.vue";
import Sidebar from "@/components/menus/Sidebar.vue";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

const route = useRoute();
const { t } = useI18n();

const title = computed(() =>
  route.meta.titleKey ? t(route.meta.titleKey as string) : "",
);
</script>

<template>
  <div class="flex h-full overflow-hidden">
    <Sidebar />
    <div class="flex min-w-0 flex-1 flex-col">
      <Header :page-title="title" />
      <main class="min-h-0 flex-1 overflow-y-auto">
        <router-view />
      </main>
    </div>
  </div>
</template>
