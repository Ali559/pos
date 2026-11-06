// The customer display: a second window (ideally on a second monitor
// facing the customer) that shows the cart, the total and the change.
// The till sends it the whole picture on every change; the display only
// listens, it never reads the database.
import { emitTo, listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { availableMonitors, currentMonitor } from "@tauri-apps/api/window";
import { inDesktopApp } from "./ipc";

export const DISPLAY_LABEL = "customer";

export interface DisplayLine {
  name: string;
  qty: string;
  total: number;
  /** A promotion or discount on the line */
  saved: number;
}

export interface DisplayState {
  locale: string;
  dir: "ltr" | "rtl";
  shopName: string;
  logo: string | null;
  currency: string;
  /** idle: welcome screen; cart: items being rung up; done: thank you */
  mode: "idle" | "cart" | "done";
  lines: DisplayLine[];
  subtotal: number;
  discount: number;
  tax: number;
  total: number;
  /** After the sale */
  paid: number | null;
  change: number | null;
  onAccount: number | null;
}

// In a plain browser tab (dev preview) the display is another tab, and a
// BroadcastChannel stands in for Tauri's events
const channel = !inDesktopApp && typeof BroadcastChannel !== "undefined" ? new BroadcastChannel("pos-display") : null;
let browserTab: Window | null = null;

export const isDisplayWindow = () => new URLSearchParams(location.search).get("display") === DISPLAY_LABEL;

export const displayOpen = async () =>
  inDesktopApp ? !!(await WebviewWindow.getByLabel(DISPLAY_LABEL)) : !!browserTab && !browserTab.closed;

/** Opens the display, full screen on another monitor when there is one. */
export const openDisplay = async () => {
  if (!inDesktopApp) {
    browserTab = window.open(`/?display=${DISPLAY_LABEL}`, DISPLAY_LABEL);
    return;
  }
  const existing = await WebviewWindow.getByLabel(DISPLAY_LABEL);
  if (existing) {
    await existing.setFocus();
    return;
  }
  const win = new WebviewWindow(DISPLAY_LABEL, {
    url: `/?display=${DISPLAY_LABEL}`,
    title: "POS — Customer",
    width: 1024,
    height: 640,
  });
  await new Promise<void>((resolve, reject) => {
    win.once("tauri://created", () => resolve());
    win.once("tauri://error", (e) => reject(e.payload));
  });
  // Put it on a screen other than the till's, full screen
  const [monitors, here] = await Promise.all([availableMonitors(), currentMonitor()]);
  const other = monitors.find((m) => m.name !== here?.name);
  if (other) {
    await win.setPosition(other.position);
    await win.setFullscreen(true);
  }
};

export const closeDisplay = async () => {
  if (!inDesktopApp) {
    browserTab?.close();
    browserTab = null;
    return;
  }
  const win = await WebviewWindow.getByLabel(DISPLAY_LABEL);
  await win?.destroy();
};

/** Sends the current picture to the display (if it's open). */
export const publish = (state: DisplayState) => {
  if (!inDesktopApp) return channel?.postMessage({ type: "display:state", state: JSON.parse(JSON.stringify(state)) });
  emitTo(DISPLAY_LABEL, "display:state", state).catch(() => {});
};

/** The till hears when a display opens, to send it what's on screen now. */
export const onDisplayReady = (fn: () => void): Promise<() => void> => {
  if (inDesktopApp) return listen("display:ready", fn);
  const handler = (e: MessageEvent) => e.data?.type === "display:ready" && fn();
  channel?.addEventListener("message", handler);
  return Promise.resolve(() => channel?.removeEventListener("message", handler));
};

/** For the display itself: hear the till, and say it's ready */
export const subscribe = async (fn: (state: DisplayState) => void) => {
  if (inDesktopApp) {
    await listen<DisplayState>("display:state", (e) => fn(e.payload));
    await emitTo("main", "display:ready");
    return;
  }
  channel?.addEventListener("message", (e) => e.data?.type === "display:state" && fn(e.data.state));
  channel?.postMessage({ type: "display:ready" });
};
