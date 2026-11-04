<script setup lang="ts">
import { inject, type ComputedRef } from "vue";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Check, ChevronDown, Languages } from "lucide-vue-next";
import { languages, type Language } from "@/utils/i18n";

const language = inject<ComputedRef<Language>>("language")!;
const setLanguage = inject<(lang: Language) => Promise<void>>("setLanguage")!;

// Each language is listed in its own script so anyone can find theirs
const options = Object.entries(languages) as [
  Language,
  (typeof languages)[Language],
][];
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button variant="outline" class="gap-2 px-3">
        <Languages class="size-4" />
        <span :lang="language">{{ languages[language].label }}</span>
        <ChevronDown class="size-4 opacity-50" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="w-44">
      <DropdownMenuItem
        v-for="[id, meta] in options"
        :key="id"
        :lang="id"
        :dir="meta.dir"
        class="flex cursor-pointer items-center justify-between"
        @click="setLanguage(id)"
      >
        <span>{{ meta.label }}</span>
        <Check v-if="language === id" class="size-4" />
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
