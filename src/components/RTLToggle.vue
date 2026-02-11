<script setup lang="ts">
import { ref, inject, computed, type Ref } from "vue";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Languages, ChevronDown, Check } from "lucide-vue-next";

// Injected state - cast as Ref to ensure .value works in script
const direction = inject<Ref<string>>("direction", ref("ltr"));

// Local state for language selection
const currentLang = ref("en");

const languages = [
  { id: "en", ltr: "English", rtl: "ئینگلیزی" },
  { id: "ku", ltr: "Kurdish", rtl: "کوردی" },
];

const currentLabel = computed(() => {
  return direction.value === "rtl" ? "زمان" : "Language";
});

const setDirection = inject<(dir: "ltr" | "rtl") => void>("setDirection")!;

// Helper to handle the selection
const selectLanguage = (langId: string, dir: "ltr" | "rtl") => {
  currentLang.value = langId;
  setDirection(dir);
};
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button
        variant="outline"
        class="flex items-center gap-2 px-3 transition-all"
        :dir="direction"
      >
        <Languages class="h-4 w-4" />
        <span class="text-sm font-medium">{{ currentLabel }}</span>
        <ChevronDown
          class="h-4 w-4 opacity-50 transition-transform duration-200 ms-1"
        />
      </Button>
    </DropdownMenuTrigger>

    <DropdownMenuContent
      :align="direction === 'rtl' ? 'end' : 'start'"
      class="w-40"
    >
      <DropdownMenuItem
        v-for="lang in languages"
        :key="lang.id"
        @click="
          () => {
            selectLanguage(lang.id, lang.id === 'en' ? 'ltr' : 'rtl');
          }
        "
        class="flex items-center justify-between cursor-pointer"
        :dir="direction"
      >
        <span>{{ direction === "rtl" ? lang.rtl : lang.ltr }}</span>
        <Check v-if="currentLang === lang.id" class="h-4 w-4" />
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
