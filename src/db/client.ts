import { call } from "@/lib/ipc";

// The WebView reads through the Rust `select` command, which runs on a
// read-only SQLite connection (src-tauri/src/query.rs). Writes go through
// the permission-checked commands; there is no `execute` here on purpose.

export const select = <T>(sql: string, params: unknown[] = []) =>
  call<T[]>("select", { sql, params });

/** First row or null. */
export const selectOne = async <T>(sql: string, params?: unknown[]) =>
  (await select<T>(sql, params))[0] ?? null;

export { inDesktopApp } from "@/lib/ipc";
