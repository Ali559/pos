import { invoke, isTauri } from "@tauri-apps/api/core";

/** True inside the desktop app; false in a plain browser tab (dev preview). */
export const inDesktopApp = isTauri();

const bridgeHeaders = { "x-pos-bridge": "1" };

const bridgeFetch = async <T>(path: string, init: RequestInit): Promise<T> => {
  const res = await fetch(`/__bridge${path}`, {
    method: "POST",
    ...init,
    headers: { ...bridgeHeaders, ...init.headers },
  });
  const body = await res.json().catch(() => null);
  if (!res.ok) {
    throw body ?? {
      code: "internal",
      message: "The dev bridge isn't reachable. Start the app with `bun run tauri dev`.",
    };
  }
  return body as T;
};

/**
 * Calls a Rust command (src-tauri/src/commands.rs). Commands take one `args`
 * object. Outside the desktop app (dev only) the call goes to the running
 * app's dev bridge, so a browser tab uses the same Rust code and database.
 */
export const call = <T = void>(command: string, args?: unknown): Promise<T> => {
  if (inDesktopApp) {
    return invoke<T>(command, args === undefined ? {} : { args });
  }
  if (!import.meta.env.DEV) {
    return Promise.reject(new Error("Only available in the desktop app."));
  }
  return bridgeFetch<T>(`/invoke/${command}`, {
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ args }),
  });
};

/** Sends raw bytes to `upload_media` (no JSON/base64 overhead). */
export const callUpload = <T>(bytes: Uint8Array, fileName: string): Promise<T> => {
  const headers = { "x-file-name": encodeURIComponent(fileName) };
  if (inDesktopApp) return invoke<T>("upload_media", bytes, { headers });
  return bridgeFetch<T>("/upload_media", { headers, body: bytes });
};
