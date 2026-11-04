import { computed, ref } from "vue";
import { call } from "@/lib/ipc";

// Mirrors auth::AuthStatus / SessionInfo in src-tauri/src/auth.rs
export interface SessionUser {
  id: number;
  fullName: string;
  username: string;
  roleId: number;
  roleName: string;
  roleName_ku: string;
  roleName_ar: string | null;
}

export interface ShiftInfo {
  id: number;
  clockedInAt: string;
  scheduleId: number | null;
  scheduledEnd: string | null;
}

export interface Session {
  user: SessionUser;
  isOwner: boolean;
  permissions: string[];
  shift: ShiftInfo | null;
}

interface AuthStatus {
  needsSetup: boolean;
  session: Session | null;
}

// Shared module state. Rust is the authority; this mirrors it for the UI.
const needsSetup = ref(false);
const session = ref<Session | null>(null);
const loaded = ref(false);

const apply = (s: AuthStatus) => {
  needsSetup.value = s.needsSetup;
  session.value = s.session;
  loaded.value = true;
  return s;
};

export const useAuth = () => ({
  needsSetup,
  session,
  loaded,
  user: computed(() => session.value?.user ?? null),
  /** UI hint only; every command re-checks in Rust. */
  can: (permission: string) =>
    !!session.value &&
    (session.value.isOwner || session.value.permissions.includes(permission)),

  refresh: async () => apply(await call<AuthStatus>("auth_status")),
  login: async (username: string, password: string) =>
    apply(await call<AuthStatus>("login", { username, password })),
  loginPin: async (pin: string) => apply(await call<AuthStatus>("login_pin", { pin })),
  completeSetup: async (shop: unknown, owner: unknown) =>
    apply(await call<AuthStatus>("complete_setup", { shop, owner })),
  logout: async () => {
    await call("logout");
    session.value = null;
  },
  endShift: async () => {
    await call("end_shift");
    session.value = null;
  },
});

/** Signed-in screens in nav order, each with the permission that opens it. */
export const MAIN_ROUTES = [
  { path: "/main/point-of-sale", permission: "sales.sell" },
  { path: "/main/inventory", permission: "inventory.view" },
  { path: "/main/analytics", permission: "analytics.view" },
  { path: "/main/expenses", permission: "expenses.view" },
  { path: "/main/staff", permission: "staff.view" },
  { path: "/main/settings", permission: null },
] as const;

/** Where to land after sign-in: the first screen this user may open. */
export const landingPath = () => {
  const { can } = useAuth();
  return (
    MAIN_ROUTES.find((r) => r.permission === null || can(r.permission))?.path ??
    "/main/settings"
  );
};
