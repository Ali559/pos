<script setup lang="ts">
import {
  ChartColumn,
  LogOut,
  Package,
  ScanBarcode,
  Settings,
} from "lucide-vue-next";
import { useRoute, useRouter } from "vue-router";

const router = useRouter();
const route = useRoute();

const items = [
  { to: "/main/point-of-sale", icon: ScanBarcode, key: "sidebar.pointOfSale" },
  { to: "/main/inventory", icon: Package, key: "sidebar.inventory" },
  { to: "/main/analytics", icon: ChartColumn, key: "sidebar.analytics" },
  { to: "/main/settings", icon: Settings, key: "sidebar.settings" },
];
</script>

<template>
  <nav
    class="flex h-full w-[88px] shrink-0 flex-col items-center bg-sidebar pt-3 pb-4 text-sidebar-foreground"
  >
    <ul class="flex w-full flex-1 flex-col gap-1 px-2">
      <li v-for="item in items" :key="item.to">
        <button
          type="button"
          @click="router.push(item.to)"
          :aria-current="route.path === item.to ? 'page' : undefined"
          class="group relative flex w-full flex-col items-center gap-1 rounded-lg px-1 py-2.5 text-[11px] leading-tight font-medium transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none aria-[current=page]:bg-sidebar-accent aria-[current=page]:text-sidebar-accent-foreground"
        >
          <span
            class="absolute inset-y-2 -start-2 w-1 rounded-e-full bg-sidebar-primary opacity-0 transition-opacity group-aria-[current=page]:opacity-100"
          />
          <component
            :is="item.icon"
            :size="22"
            :stroke-width="1.75"
            class="group-aria-[current=page]:text-sidebar-primary"
          />
          <span class="text-center">{{ $t(item.key) }}</span>
        </button>
      </li>
    </ul>

    <div class="flex flex-col items-center gap-3">
      <div
        class="grid size-10 place-items-center rounded-full bg-sidebar-accent text-sm font-semibold text-sidebar-accent-foreground"
        :title="`${$t('sidebar.userInfo.user')}: Admin`"
      >
        AD
      </div>
      <button
        type="button"
        @click="router.push('/')"
        :title="$t('sidebar.logout')"
        :aria-label="$t('sidebar.logout')"
        class="grid size-10 place-items-center rounded-lg transition-colors hover:bg-destructive/20 hover:text-destructive focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none"
      >
        <LogOut :size="19" class="rtl:rotate-180" />
      </button>
    </div>
  </nav>
</template>
