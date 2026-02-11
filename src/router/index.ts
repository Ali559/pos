import { createMemoryHistory, createRouter } from "vue-router";

// * Router Config
export const router = createRouter({
  history: createMemoryHistory(),
  routes: [
    {
      path: "/",
      name: "Login",
      component: () => import("@/views/LoginView.vue"),
    },
    {
      path: "/main",
      name: "main",
      component: () => import("@/layouts/MainLayout.vue"),
      children: [
        {
          path: "point-of-sale",
          name: "Point of Sale",
          component: () => import("@/views/SalesView.vue"),
        },
        {
          path: "inventory",
          name: "Inventory",
          component: () => import("@/views/InventoryView.vue"),
        },
        {
          path: "analytics",
          name: "Analytics",
          component: () => import("@/views/AnalyticsView.vue"),
        },
        {
          path: "settings",
          name: "Settings",
          component: () => import("@/views/SettingsView.vue"),
        },
      ],
    },
  ],
});
