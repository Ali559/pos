// A slip: a short report on receipt paper (end of shift, end of day).
// Built as a list of lines so it prints the same two ways receipts do:
// drawn to a bitmap and sent as ESC/POS to the receipt printer, or laid
// out in #print-root (SlipPrint.vue) for the print dialog.
import { nextTick, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getSetting } from "@/db/settings";
import { call, inDesktopApp } from "@/lib/ipc";
import { packBits, toBase64, type Raster } from "@/lib/receiptRaster";

export type SlipLine =
  | { kind: "title"; text: string }
  | { kind: "center"; text: string }
  | { kind: "heading"; text: string }
  | { kind: "row"; text: string; value: string; bold?: boolean; indent?: boolean }
  | { kind: "rule" };

export const printingSlip = ref<{ lines: SlipLine[]; width: 58 | 80 } | null>(null);

const DOTS = { 58: 384, 80: 576 } as const;
const MAX_HEIGHT = 6000;

/** Draws the slip at the printer's resolution (203 dpi). */
export const renderSlip = (lines: SlipLine[], width: 58 | 80): Raster => {
  const W = DOTS[width];
  const pad = 8;
  const base = width === 80 ? 24 : 19;
  const rtl = document.documentElement.dir === "rtl";
  const family = getComputedStyle(document.body).fontFamily;
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = MAX_HEIGHT;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, W, MAX_HEIGHT);
  ctx.fillStyle = "#000";
  ctx.direction = rtl ? "rtl" : "ltr";
  ctx.textBaseline = "top";
  let y = pad;
  const font = (size: number, bold = false) => (ctx.font = `${bold ? 700 : 400} ${size}px ${family}`);
  const startX = (indent: boolean) => (rtl ? W - pad - (indent ? 16 : 0) : pad + (indent ? 16 : 0));
  const endX = rtl ? pad : W - pad;
  const wrap = (text: string, max: number) => {
    const out: string[] = [];
    let line = "";
    for (const w of text.split(/\s+/)) {
      const next = line ? `${line} ${w}` : w;
      if (ctx.measureText(next).width > max && line) {
        out.push(line);
        line = w;
      } else line = next;
    }
    if (line) out.push(line);
    return out;
  };
  for (const l of lines) {
    if (y > MAX_HEIGHT - 100) break;
    if (l.kind === "rule") {
      y += 6;
      for (let x = pad; x < W - pad; x += 10) ctx.fillRect(x, y, 6, 2);
      y += 10;
    } else if (l.kind === "title" || l.kind === "center" || l.kind === "heading") {
      const size = l.kind === "title" ? Math.round(base * 1.35) : base;
      font(size, l.kind !== "center");
      ctx.textAlign = l.kind === "heading" ? "start" : "center";
      if (l.kind === "heading") y += 6;
      for (const t of wrap(l.text, W - 2 * pad)) {
        ctx.fillText(t, l.kind === "heading" ? startX(false) : W / 2, y);
        y += Math.round(size * 1.35);
      }
    } else {
      font(base, l.bold);
      const vw = ctx.measureText(l.value).width;
      ctx.textAlign = "end";
      ctx.fillText(l.value, endX, y);
      ctx.textAlign = "start";
      for (const t of wrap(l.text, W - 2 * pad - vw - 12 - (l.indent ? 16 : 0))) {
        ctx.fillText(t, startX(!!l.indent), y);
        y += Math.round(base * 1.35);
      }
    }
  }
  y += pad;
  const height = Math.min(MAX_HEIGHT, Math.ceil(y));
  const pixels = ctx.getImageData(0, 0, W, height).data;
  return { widthBytes: W / 8, height, data: toBase64(packBits(pixels, W, height)) };
};

/** Prints a slip on the receipt printer, or through the print dialog. */
export const printSlip = async (lines: SlipLine[]) => {
  const [w, printer] = await Promise.all([getSetting("receipt_width"), getSetting("receipt_printer")]);
  const width = w === "58" ? (58 as const) : (80 as const);
  if (printer) {
    await call("print_raster", { ...renderSlip(lines, width), openDrawer: false, target: null });
    return;
  }
  printingSlip.value = { lines, width };
  await nextTick();
  try {
    if (inDesktopApp) await invoke("print_receipt");
    else window.print();
  } finally {
    setTimeout(() => (printingSlip.value = null), 1500);
  }
};
