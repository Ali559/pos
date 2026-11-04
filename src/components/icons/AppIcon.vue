<script setup lang="ts">
import { Icon } from "@iconify/vue/offline";
import { iconsReady, loadIcons } from "@/lib/icons";

// Renders a stored icon key ("tabler:bottle"). Keeps its box while the
// icon set is still loading so layouts don't jump.
withDefaults(defineProps<{ icon: string; size?: number }>(), { size: 20 });
loadIcons();
</script>

<template>
  <Icon
    v-if="iconsReady"
    :icon="icon"
    :width="size"
    :height="size"
    aria-hidden="true"
    class="shrink-0"
  />
  <span
    v-else
    class="inline-block shrink-0"
    :style="{ width: `${size}px`, height: `${size}px` }"
    aria-hidden="true"
  />
</template>
