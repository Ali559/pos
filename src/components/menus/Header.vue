<script setup lang="ts">
import { useTimeManager } from "@/hooks/useTimeManager";
import { Menu } from "lucide-vue-next";
import RTLToggle from "../RTLToggle.vue";

defineProps<{ pageTitle: string }>();
defineEmits<{
  (e: "toggle-menu"): void;
}>();

const { timeOfDay } = useTimeManager();
</script>

<template>
  <header
    class="bg-card/50 backdrop-blur-sm border-b sticky top-0 z-30 shadow-lg"
  >
    <div class="px-6 py-4 flex items-center">
      <div class="flex gap-4 items-center">
        <button
          @click="$emit('toggle-menu')"
          class="p-2 hover:bg-accent rounded-lg transition-colors"
        >
          <Menu class="text-foreground" />
        </button>
        <h1 class="text-xl font-semibold text-foreground">{{ pageTitle }}</h1>
      </div>
      <div class="ms-auto flex items-center gap-4">
        <RTLToggle class="mr-4" />
        <div class="text-sm font-medium text-muted-foreground">
          {{ timeOfDay }}
        </div>
      </div>
    </div>
  </header>
</template>
