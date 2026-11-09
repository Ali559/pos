import { selectOne } from "./client";
import { call } from "@/lib/ipc";
import { DEVICE_KEYS, localSetting, setLocalSetting } from "@/lib/device";

/** A shop setting, or one of this computer's own (receipt printer, paper,
 *  auto-print, full screen), which stay on each till. */
export const getSetting = async (key: string) => {
  if (DEVICE_KEYS.includes(key)) return localSetting(key);
  return (await selectOne<{ value: string | null }>(
    "SELECT value FROM settings WHERE key = $1",
    [key],
  ))?.value ?? null;
};

/** Rust decides who may change which key (src-tauri/src/settings.rs). */
export const setSetting = (key: string, value: string) =>
  DEVICE_KEYS.includes(key) ? setLocalSetting(key, value) : call("set_setting", { key, value });
