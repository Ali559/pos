import { createMemoryHistory, createRouter } from "vue-router";
import { useShop } from "@/stores/shop";
import { landingPath, useAuth } from "@/stores/auth";

declare module "vue-router" {
  interface RouteMeta {
    titleKey?: string;
    /** Needed to open the screen. A UI guard only; Rust checks every action. */
    permission?: string;
  }
}

// * Router Config
export const router = createRouter({
  history: createMemoryHistory(),
  routes: [
    {
      path: "/setup",
      name: "Setup",
      component: () => import("@/views/SetupView.vue"),
    },
    {
      path: "/",
      name: "Login",
      component: () => import("@/views/LoginView.vue"),
    },
    {
      path: "/main",
      name: "main",
      component: () => import("@/layouts/MainLayout.vue"),
      redirect: () => landingPath(),
      children: [
        {
          path: "point-of-sale",
          name: "Point of Sale",
          meta: { titleKey: "sidebar.pointOfSale", permission: "sales.sell" },
          component: () => import("@/views/SalesView.vue"),
        },
        {
          path: "inventory",
          name: "Inventory",
          meta: { titleKey: "sidebar.inventory", permission: "inventory.view" },
          component: () => import("@/views/InventoryView.vue"),
        },
        {
          path: "analytics",
          name: "Analytics",
          meta: { titleKey: "sidebar.analytics", permission: "analytics.view" },
          component: () => import("@/views/AnalyticsView.vue"),
        },
        {
          path: "staff",
          name: "Staff",
          meta: { titleKey: "sidebar.staff", permission: "staff.view" },
          component: () => import("@/views/StaffView.vue"),
        },
        {
          path: "settings",
          name: "Settings",
          meta: { titleKey: "sidebar.settings" },
          component: () => import("@/views/SettingsView.vue"),
        },
      ],
    },
  ],
});

router.beforeEach(async (to) => {
  const shop = useShop();
  const auth = useAuth();
  try {
    await Promise.all([shop.load(), auth.refresh()]);
  } catch (e) {
    console.error("[router] could not load app state", e);
    return true;
  }

  // First run: shop details and an owner account before anything else
  const needsSetup = auth.needsSetup.value || shop.profile.value === null;
  if (needsSetup) return to.path === "/setup" ? true : "/setup";
  if (to.path === "/setup") return "/";

  const signedIn = auth.session.value !== null;
  if (to.path.startsWith("/main") && !signedIn) return "/";
  if (to.path === "/" && signedIn) return landingPath();
  if (to.meta.permission && !auth.can(to.meta.permission)) return landingPath();
  return true;
});
