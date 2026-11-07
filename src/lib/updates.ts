// Updates: the app asks GitHub for a newer signed release (the URL and the
// public key are in tauri.conf.json → plugins.updater). Nothing installs on
// its own: a banner offers it, and installing restarts the app, so the
// cashier picks a moment between customers.
import { ref } from "vue";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { inDesktopApp } from "./ipc";

export const available = ref<Update | null>(null);
/** 0–1 while downloading */
export const progress = ref<number | null>(null);

export const currentVersion = () => (inDesktopApp ? getVersion() : Promise.resolve(import.meta.env.DEV ? "dev" : ""));

/** Looks for a newer release. Quiet on failure (offline is normal). */
export const checkForUpdate = async () => {
  if (!inDesktopApp) return null;
  try {
    available.value = await check();
  } catch (e) {
    console.warn("[updates] check failed", e);
    available.value = null;
  }
  return available.value;
};

/** Downloads, installs and restarts. */
export const installUpdate = async () => {
  const update = available.value;
  if (!update) return;
  let total = 0;
  let done = 0;
  progress.value = 0;
  await update.downloadAndInstall((e) => {
    if (e.event === "Started") total = e.data.contentLength ?? 0;
    else if (e.event === "Progress") {
      done += e.data.chunkLength;
      progress.value = total ? done / total : null;
    } else if (e.event === "Finished") progress.value = 1;
  });
  await relaunch();
};

let timer: ReturnType<typeof setInterval> | null = null;
/** At startup, then every six hours while the app stays open */
export const startUpdateChecks = () => {
  if (!inDesktopApp || timer) return;
  checkForUpdate();
  timer = setInterval(checkForUpdate, 6 * 60 * 60 * 1000);
};
