<script setup lang="ts">
import { computed, ref } from "vue";
import {
  BookUser,
  ChartColumn,
  Clock,
  LogOut,
  Package,
  ScanBarcode,
  Settings,
  UsersRound,
  Wallet,
} from "lucide-vue-next";
import { useRoute, useRouter } from "vue-router";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import EndShiftDialog from "@/components/sales/EndShiftDialog.vue";
import { MAIN_ROUTES, useAuth } from "@/stores/auth";

const router = useRouter();
const route = useRoute();
const auth = useAuth();

const meta = {
  "/main/point-of-sale": { icon: ScanBarcode, key: "sidebar.pointOfSale" },
  "/main/inventory": { icon: Package, key: "sidebar.inventory" },
  "/main/customers": { icon: BookUser, key: "sidebar.customers" },
  "/main/analytics": { icon: ChartColumn, key: "sidebar.analytics" },
  "/main/expenses": { icon: Wallet, key: "sidebar.expenses" },
  "/main/staff": { icon: UsersRound, key: "sidebar.staff" },
  "/main/settings": { icon: Settings, key: "sidebar.settings" },
} as const;

// Only screens this user may open
const items = computed(() =>
  MAIN_ROUTES.filter((r) => r.permission === null || auth.can(r.permission)).map(
    (r) => ({ to: r.path, ...meta[r.path] }),
  ),
);

const initials = computed(() =>
  (auth.user.value?.fullName ?? "")
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0])
    .join("")
    .toUpperCase(),
);

const logout = async () => {
  await auth.logout();
  router.replace("/");
};
const endingShift = ref(false);
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
          :aria-current="route.path === item.to || route.path.startsWith(item.to + '/') ? 'page' : undefined"
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

    <DropdownMenu v-if="auth.user.value">
      <DropdownMenuTrigger as-child>
        <button
          type="button"
          class="grid size-11 place-items-center rounded-full bg-sidebar-accent text-sm font-semibold text-sidebar-accent-foreground ring-sidebar-ring outline-none hover:ring-2 focus-visible:ring-2"
          :aria-label="auth.user.value.fullName"
        >
          {{ initials }}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent side="right" align="end" class="w-56">
        <DropdownMenuLabel class="font-normal">
          <div class="font-semibold">{{ auth.user.value.fullName }}</div>
          <div class="text-xs text-muted-foreground">
            {{ auth.user.value.roleName }}
          </div>
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem class="cursor-pointer gap-2" @click="logout">
          <LogOut class="size-4 rtl:rotate-180" />
          {{ $t("sidebar.switchUser") }}
        </DropdownMenuItem>
        <DropdownMenuItem
          class="cursor-pointer gap-2 text-destructive focus:text-destructive"
          @click="endingShift = true"
        >
          <Clock class="size-4" />
          {{ $t("sidebar.endShift") }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
    <EndShiftDialog v-model:open="endingShift" @ended="router.replace('/')" />
  </nav>
</template>
