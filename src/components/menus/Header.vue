<script setup lang="ts">
import { useTimeManager } from "@/hooks/useTimeManager";
import RTLToggle from "../RTLToggle.vue";

defineProps<{ pageTitle: string }>();

const { timeOfDay } = useTimeManager();
</script>

<template>
  <header
    class="flex h-16 shrink-0 items-center gap-4 border-b bg-background px-6"
  >
    <h1 class="text-lg font-semibold tracking-tight">{{ pageTitle }}</h1>
    <div class="ms-auto flex items-center gap-5">
      <span class="text-sm text-muted-foreground">{{ timeOfDay }}</span>
      <RTLToggle />
    </div>
  </header>
</template>
