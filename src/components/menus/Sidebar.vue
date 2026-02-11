<script setup lang="ts">
import {
  BadgeDollarSign,
  ChartPie,
  CirclePile,
  LogOut,
  Settings,
  X,
} from "lucide-vue-next";
import { useRouter } from "vue-router";

defineProps<{ isOpen: boolean }>();
defineEmits<{
  (e: "close"): void;
  (e: "logout"): void;
}>();
const router = useRouter();
</script>

<template>
  <Transition name="slide">
    <div
      v-if="isOpen"
      class="fixed inset-y-0 start-0 h-full w-72 bg-sidebar border-r border-sidebar-border z-50 shadow-xl"
    >
      <div class="p-6 border-b border-sidebar-border">
        <div
          class="flex justify-between items-center mb-6 rtl:flex-row-reverse"
        >
          <h2 class="text-xl font-semibold text-sidebar-foreground">Menu</h2>
          <button
            @click="$emit('close')"
            class="p-2 hover:bg-sidebar-accent rounded-lg transition-colors"
          >
            <X class="text-sidebar-foreground" />
          </button>
        </div>
        <nav class="space-y-1">
          <button
            @click="router.push('/main/point-of-sale')"
            class="w-full flex gap-3 rtl:flex-row-reverse px-4 py-3 rounded-lg font-medium items-center text-sidebar-foreground hover:text-sidebar-primary hover:bg-sidebar-accent transition-colors"
          >
            <BadgeDollarSign :size="20" /> {{ $t("sidebar.pointOfSale") }}
          </button>
          <button
            @click="router.push('/main/inventory')"
            class="w-full flex gap-3 rtl:flex-row-reverse px-4 py-3 rounded-lg font-medium items-center text-sidebar-foreground hover:text-sidebar-primary hover:bg-sidebar-accent transition-colors"
          >
            <CirclePile :size="20" /> {{ $t("sidebar.inventory") }}
          </button>
          <button
            @click="router.push('/main/analytics')"
            class="w-full flex gap-3 rtl:flex-row-reverse px-4 py-3 rounded-lg font-medium items-center text-sidebar-foreground hover:text-sidebar-primary hover:bg-sidebar-accent transition-colors"
          >
            <ChartPie :size="20" /> {{ $t("sidebar.analytics") }}
          </button>
          <button
            @click="router.push('/main/settings')"
            class="w-full flex gap-3 rtl:flex-row-reverse px-4 py-3 rounded-lg font-medium items-center text-sidebar-foreground hover:text-sidebar-primary hover:bg-sidebar-accent transition-colors"
          >
            <Settings :size="20" /> {{ $t("sidebar.settings") }}
          </button>
        </nav>
      </div>
      <div
        class="p-6 border-t border-sidebar-border absolute bottom-0 w-full bg-sidebar"
      >
        <div class="text-sm mb-4 text-sidebar-foreground text-center">
          <div class="font-medium">
            {{ $t("sidebar.userInfo.user") }}: <span id="userName">Admin</span>
          </div>
          <div class="text-muted-foreground">
            {{ $t("sidebar.userInfo.role") }}:
            <span id="userRole">Administrator</span>
          </div>
        </div>
        <button
          @click="$emit('logout')"
          class="w-full flex items-center rtl:flex-row-reverse justify-center gap-2 px-4 py-3 rounded-lg font-medium text-foreground border border-input hover:bg-destructive hover:text-destructive-foreground hover:border-destructive transition-colors"
        >
          <LogOut :size="18" class="rtl:rotate-180" />
          {{ $t("sidebar.logout") }}
        </button>
      </div>
    </div>
  </Transition>
</template>

<style>
.slide-enter-active,
.slide-leave-active {
  transition: transform 0.3s ease;
}

.slide-enter-from,
.slide-leave-to {
  transform: translateX(-100%);
}

[dir="rtl"] .slide-enter-from,
[dir="rtl"] .slide-leave-to {
  transform: translateX(100%);
}
</style>
