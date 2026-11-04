import { selectOne } from "./client";
import { call } from "@/lib/ipc";

export const getSetting = async (key: string) =>
  (await selectOne<{ value: string | null }>(
    "SELECT value FROM settings WHERE key = $1",
    [key],
  ))?.value ?? null;

/** Rust decides who may change which key (src-tauri/src/settings.rs). */
export const setSetting = (key: string, value: string) =>
  call("set_setting", { key, value });
