<script setup lang="ts">
import { inject, type ComputedRef } from "vue";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";

// Side panel for create/edit forms; slides in from the reading end
const open = defineModel<boolean>("open", { required: true });
defineProps<{ title: string; description?: string }>();
defineEmits<{ submit: [] }>();

const direction = inject<ComputedRef<"ltr" | "rtl">>("direction");
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent
      :side="direction === 'rtl' ? 'left' : 'right'"
      class="w-full gap-0 sm:max-w-lg"
      :dir="direction"
    >
      <SheetHeader class="border-b p-5">
        <SheetTitle class="text-lg">{{ title }}</SheetTitle>
        <SheetDescription v-if="description">{{ description }}</SheetDescription>
      </SheetHeader>
      <form class="flex min-h-0 flex-1 flex-col" @submit.prevent="$emit('submit')">
        <div class="min-h-0 flex-1 space-y-5 overflow-y-auto p-5">
          <slot />
        </div>
        <div class="flex items-center gap-3 border-t p-5">
          <slot name="footer" />
        </div>
      </form>
    </SheetContent>
  </Sheet>
</template>
