<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { Bell, CalendarClock, PackageX, Receipt, TriangleAlert, X } from "lucide-vue-next";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Button } from "@/components/ui/button";
import { listNotifications, type AppNotification, type NotificationKind } from "@/db/notifications";
import { call } from "@/lib/ipc";
import { formatCurrency } from "@/lib/format";
import { useAuth } from "@/stores/auth";
import { localized } from "@/utils/i18n";

const { t } = useI18n();
const router = useRouter();
const auth = useAuth();

// Each person only sees alerts for areas they can act on
const kinds = computed<NotificationKind[]>(() => [
  ...(auth.can("inventory.view") ? (["stock_low", "stock_out", "expiring", "expired"] as const) : []),
  ...(auth.can("expenses.view") ? (["expense_due"] as const) : []),
]);

const items = ref<AppNotification[]>([]);
const unread = computed(() => items.value.filter((n) => !n.read_at).length);
const open = ref(false);

const load = async () => {
  items.value = await listNotifications(kinds.value);
};

// Rust keeps the table current after every stock change and hourly; this
// only re-reads it
let timer: ReturnType<typeof setInterval>;
onMounted(() => {
  load();
  timer = setInterval(load, 60_000);
});
onUnmounted(() => clearInterval(timer));

const onOpenChange = async (isOpen: boolean) => {
  open.value = isOpen;
  if (!isOpen) return;
  await call("refresh_notifications");
  await load();
  if (unread.value) {
    await call("mark_notifications_read", { ids: null });
  }
};

const dismiss = async (n: AppNotification) => {
  await call("dismiss_notification", { id: n.id });
  items.value = items.value.filter((x) => x.id !== n.id);
};

const icon = (k: NotificationKind) =>
  ({ stock_out: PackageX, stock_low: TriangleAlert, expired: CalendarClock, expiring: CalendarClock, expense_due: Receipt })[k];
const tone = (k: NotificationKind) =>
  k === "stock_out" || k === "expired" ? "text-destructive bg-destructive/10" : "text-warning bg-warning/12";

// Inserted values (an English product name, a date) keep their own
// direction inside a Kurdish/Arabic sentence: wrap them in Unicode
// first-strong isolates (FSI … PDI), the plain-text form of <bdi>
const iso = (v: unknown) => `\u2068${v ?? ""}\u2069`;

const text = (n: AppNotification) => {
  const p = n.payload as Record<string, never>;
  switch (n.kind) {
    case "stock_out":
      return t("alerts.stockOut", { name: iso(p.name) });
    case "stock_low":
      return t("alerts.stockLow", { name: iso(p.name), stock: iso(p.stock) });
    case "expiring":
      return t("alerts.expiring", { name: iso(p.name), quantity: iso(p.quantity), date: iso(p.expiry) });
    case "expired":
      return t("alerts.expired", { name: iso(p.name), quantity: iso(p.quantity), date: iso(p.expiry) });
    case "expense_due":
      return t("alerts.expenseDue", { name: iso(localized(p, "name")), amount: iso(formatCurrency(Number(p.amount))), date: iso(p.date) });
  }
};

const go = (n: AppNotification) => {
  open.value = false;
  if (n.kind === "expense_due") router.push("/main/expenses");
  else router.push({ path: "/main/inventory", query: { q: String(n.payload.name ?? "") } });
};
</script>

<template>
  <DropdownMenu v-if="kinds.length" :open="open" @update:open="onOpenChange">
    <DropdownMenuTrigger as-child>
      <Button
        variant="ghost"
        size="icon"
        class="relative"
        :aria-label="unread ? $t('alerts.titleUnread', { n: unread }) : $t('alerts.title')"
      >
        <Bell />
        <span
          v-if="unread"
          class="num absolute -end-0.5 -top-0.5 grid h-5 min-w-5 place-items-center rounded-full bg-destructive px-1 text-[11px] font-semibold text-white"
        >
          {{ unread > 9 ? "9+" : unread }}
        </span>
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="w-96 p-0">
      <div class="border-b px-4 py-3 font-semibold">{{ $t("alerts.title") }}</div>
      <p v-if="!items.length" class="px-4 py-10 text-center text-sm text-muted-foreground">
        {{ $t("alerts.none") }}
      </p>
      <ul v-else class="max-h-[60vh] divide-y overflow-y-auto">
        <li v-for="n in items" :key="n.id" class="group flex items-start gap-3 px-4 py-3 hover:bg-muted/60">
          <span class="mt-0.5 grid size-8 shrink-0 place-items-center rounded-full" :class="tone(n.kind)">
            <component :is="icon(n.kind)" :size="16" />
          </span>
          <button type="button" class="min-w-0 flex-1 text-start text-sm" @click="go(n)">
            {{ text(n) }}
          </button>
          <button
            type="button"
            class="rounded p-1 text-muted-foreground opacity-0 group-hover:opacity-100 hover:bg-muted hover:text-foreground focus-visible:opacity-100"
            :aria-label="$t('alerts.dismiss')"
            :title="$t('alerts.dismissHint')"
            @click="dismiss(n)"
          >
            <X :size="14" />
          </button>
        </li>
      </ul>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
