import { createMemoryHistory, createRouter } from "vue-router";
import { useShop } from "@/stores/shop";

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
      redirect: "/main/point-of-sale",
      children: [
        {
          path: "point-of-sale",
          meta: { titleKey: "sidebar.pointOfSale" },
          name: "Point of Sale",
          component: () => import("@/views/SalesView.vue"),
        },
        {
          path: "inventory",
          meta: { titleKey: "sidebar.inventory" },
          name: "Inventory",
          component: () => import("@/views/InventoryView.vue"),
        },
        {
          path: "analytics",
          meta: { titleKey: "sidebar.analytics" },
          name: "Analytics",
          component: () => import("@/views/AnalyticsView.vue"),
        },
        {
          path: "settings",
          meta: { titleKey: "sidebar.settings" },
          name: "Settings",
          component: () => import("@/views/SettingsView.vue"),
        },
      ],
    },
  ],
});

// First run: nothing works until the shop has a name
router.beforeEach(async (to) => {
  const shop = useShop();
  try {
    await shop.load();
  } catch (e) {
    console.error("[router] could not load shop profile", e);
    return true;
  }
  const hasShop = shop.profile.value !== null;
  if (!hasShop && to.path !== "/setup") return "/setup";
  if (hasShop && to.path === "/setup") return "/";
  return true;
});
