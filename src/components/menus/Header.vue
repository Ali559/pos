<script setup lang="ts">
import { Store } from "lucide-vue-next";
import { useTimeManager } from "@/hooks/useTimeManager";
import { useShop } from "@/stores/shop";
import { useAuth } from "@/stores/auth";
import { useShiftStatus } from "@/hooks/useShiftStatus";
import MediaImage from "@/components/media/MediaImage.vue";
import LanguageSwitcher from "../LanguageSwitcher.vue";

defineProps<{ pageTitle: string }>();

const { timeOfDay } = useTimeManager();
const { profile, name } = useShop();
const { user } = useAuth();
const shift = useShiftStatus();
</script>

<template>
  <header
    class="flex h-16 shrink-0 items-center gap-4 border-b bg-background px-6"
  >
    <div class="flex min-w-0 items-center gap-3">
      <div
        class="grid size-9 shrink-0 place-items-center overflow-hidden rounded-lg border bg-card text-muted-foreground"
      >
        <MediaImage
          v-if="profile?.logo"
          :media="profile.logo"
          :alt="name"
          fit="contain"
          class="size-full"
        />
        <Store v-else :size="18" :stroke-width="1.75" />
      </div>
      <span class="truncate font-semibold">{{ name }}</span>
    </div>
    <span class="h-5 w-px shrink-0 bg-border" aria-hidden="true" />
    <h1 class="truncate text-muted-foreground">{{ pageTitle }}</h1>
    <div class="ms-auto flex shrink-0 items-center gap-5">
      <div v-if="user" class="text-end leading-tight">
        <div class="text-sm font-medium">{{ user.fullName }}</div>
        <div
          v-if="shift"
          class="text-xs"
          :class="shift.tone === 'warn' ? 'font-medium text-warning' : 'text-muted-foreground'"
        >
          {{ shift.text }}
        </div>
      </div>
      <span class="h-8 w-px bg-border" aria-hidden="true" />
      <span class="text-sm text-muted-foreground">{{ timeOfDay }}</span>
      <LanguageSwitcher />
    </div>
  </header>
</template>
