import { call, inDesktopApp } from "./ipc";

/**
 * Exports rows (first row = header). In the app, Rust checks
 * `analytics.export` and opens the save dialog itself; in the browser dev
 * preview the permission is checked the same way and the browser downloads.
 * Returns false if the user cancelled.
 */
export const exportCsv = async (fileName: string, rows: (string | number | null)[][]) => {
  const cells = rows.map((r) => r.map((c) => (c === null ? "" : String(c))));
  if (inDesktopApp) return call<boolean>("save_csv", { fileName, rows: cells });

  await call("authorize_export");
  const quote = (s: string) => (/[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);
  const text = "﻿" + cells.map((r) => r.map(quote).join(",")).join("\r\n");
  const url = URL.createObjectURL(new Blob([text], { type: "text/csv;charset=utf-8" }));
  const a = Object.assign(document.createElement("a"), { href: url, download: `${fileName}.csv` });
  a.click();
  URL.revokeObjectURL(url);
  return true;
};
