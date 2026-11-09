// Exports a report table as CSV, an Excel workbook, or a PDF. CSV and Excel
// are written by Rust (it opens the save dialog and checks
// `analytics.export`); the PDF is an A4 page (ReportPrint.vue) sent to the
// print dialog, where "Save as PDF" keeps Kurdish and Arabic shaped exactly
// as on screen.
import { nextTick, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { call, inDesktopApp } from "./ipc";
import { exportCsv } from "./exportCsv";

export type ExportFormat = "csv" | "xlsx" | "pdf";
export type Cell = string | number | null;

export const printingReport = ref<{ title: string; subtitle: string; rows: Cell[][] } | null>(null);

export const exportTable = async (
  format: ExportFormat,
  file: { name: string; title: string; subtitle: string },
  rows: Cell[][],
): Promise<boolean> => {
  if (format === "csv") return exportCsv(file.name, rows);
  if (format === "xlsx") {
    if (!inDesktopApp) throw new Error("Excel files are saved by the desktop app.");
    return call<boolean>("save_xlsx", {
      fileName: file.name,
      sheet: file.title,
      rows,
      rtl: document.documentElement.dir === "rtl",
    });
  }
  // PDF: same permission and audit trail as the files
  await call("authorize_export");
  printingReport.value = { title: file.title, subtitle: file.subtitle, rows };
  await nextTick();
  try {
    if (inDesktopApp) await invoke("print_receipt");
    else window.print();
    await call("record_export", { details: { file: `${file.name}.pdf`, rows: rows.length } });
  } finally {
    setTimeout(() => (printingReport.value = null), 1500);
  }
  return true;
};
