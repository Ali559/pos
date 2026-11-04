<script setup lang="ts">
import { computed } from "vue";
import MediaImage from "@/components/media/MediaImage.vue";
import AppIcon from "@/components/icons/AppIcon.vue";
import { categoryInk, categoryTint } from "@/lib/categoryColor";
import type { Product } from "@/db/catalog";

// What a product looks like in lists and on the register, in order of
// preference: cover photo → its own icon → its category's icon → initials.
const props = withDefaults(
  defineProps<{
    product: Pick<Product, "name" | "cover" | "iconKey" | "categoryIcon" | "categoryColor">;
    size?: number;
  }>(),
  { size: 40 },
);

const icon = computed(() => props.product.iconKey ?? props.product.categoryIcon);
const initials = computed(() =>
  props.product.name
    .split(/\s+/)
    .filter((w) => /\p{L}/u.test(w))
    .slice(0, 2)
    .map((w) => [...w][0])
    .join("")
    .toUpperCase(),
);
</script>

<template>
  <div
    class="grid shrink-0 place-items-center overflow-hidden rounded-lg"
    :style="{
      width: `${size}px`,
      height: `${size}px`,
      background: categoryTint(product.categoryColor),
      color: categoryInk(product.categoryColor),
    }"
  >
    <MediaImage v-if="product.cover" :media="product.cover" class="size-full" />
    <AppIcon v-else-if="icon" :icon="icon" :size="Math.round(size * 0.55)" />
    <span v-else class="font-semibold" :style="{ fontSize: `${Math.round(size * 0.36)}px` }">
      {{ initials }}
    </span>
  </div>
</template>
