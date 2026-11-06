// Shelf and price labels: the product's name, its price, and a barcode
// (EAN-13 when it has one, else Code 128 of its barcode or SKU).
//   - sheet:   an A4 page of labels through the print dialog (LabelsPrint.vue)
//   - printer: one label at a time on the receipt printer, each cut off
import { nextTick, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { barcodeBars } from "./ean13";
import { call, inDesktopApp } from "./ipc";
import { formatCurrency } from "./format";
import { packBits, toBase64 } from "./receiptRaster";
import { getSetting } from "@/db/settings";
import { i18n } from "@/utils/i18n";

export interface LabelItem {
  name: string;
  price: number;
  /** Barcode, or SKU when there's none */
  code: string;
  /** "per kg" for weighed items, else null */
  per: string | null;
}

export const printingLabels = ref<{ labels: LabelItem[] } | null>(null);

const DOTS = { 58: 384, 80: 576 } as const;

/** One label, drawn at the receipt printer's resolution */
export const renderLabel = (item: LabelItem, width: 58 | 80) => {
  const W = DOTS[width];
  const H = width === 80 ? 300 : 260;
  const pad = 10;
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext("2d")!;
  const family = getComputedStyle(document.body).fontFamily;
  const rtl = document.documentElement.dir === "rtl";
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, W, H);
  ctx.fillStyle = "#000";
  ctx.direction = rtl ? "rtl" : "ltr";
  ctx.textBaseline = "top";
  ctx.textAlign = "center";

  // Name: up to two lines
  const nameSize = width === 80 ? 28 : 22;
  ctx.font = `700 ${nameSize}px ${family}`;
  const words = item.name.split(/\s+/);
  const lines: string[] = [];
  let line = "";
  for (const w of words) {
    const next = line ? `${line} ${w}` : w;
    if (ctx.measureText(next).width > W - 2 * pad && line) {
      lines.push(line);
      line = w;
    } else line = next;
  }
  if (line) lines.push(line);
  let y = pad;
  for (const l of lines.slice(0, 2)) {
    ctx.fillText(l, W / 2, y);
    y += nameSize * 1.25;
  }

  // Price, big
  const priceSize = width === 80 ? 56 : 44;
  ctx.font = `800 ${priceSize}px ${family}`;
  const price = `⁦${formatCurrency(item.price)}⁩${item.per ? ` / ${item.per}` : ""}`;
  ctx.fillText(price, W / 2, y + 4);
  y += priceSize + 14;

  // Barcode with its digits, whole dots per module so it scans
  const { bars, width: modules, text } = barcodeBars(item.code);
  const module = Math.max(2, Math.floor((W - 2 * pad) / (modules + 10)));
  const left = Math.round((W - module * modules) / 2);
  const barH = Math.max(40, H - y - 30);
  for (const [x, w] of bars) ctx.fillRect(left + x * module, y, w * module, barH);
  ctx.font = `400 ${width === 80 ? 20 : 16}px ${family}`;
  ctx.direction = "ltr";
  ctx.fillText(text, W / 2, y + barH + 4);

  const pixels = ctx.getImageData(0, 0, W, H).data;
  return { widthBytes: W / 8, height: H, data: toBase64(packBits(pixels, W, H)) };
};

/** Prints labels: on the receipt printer, or as an A4 sheet */
export const printLabels = async (labels: LabelItem[], format: "sheet" | "printer") => {
  if (format === "printer") {
    const width = (await getSetting("receipt_width")) === "58" ? 58 : 80;
    for (const item of labels) await call("print_raster", { ...renderLabel(item, width), openDrawer: false, target: null });
    return;
  }
  printingLabels.value = { labels };
  await nextTick();
  try {
    if (inDesktopApp) await invoke("print_receipt");
    else window.print();
  } finally {
    setTimeout(() => (printingLabels.value = null), 1500);
  }
};

/** "kg" etc. for the label of a weighed product */
export const perUnit = (unit: string) =>
  ["piece", "pack", "box"].includes(unit) ? null : i18n.global.t(`catalog.units.${unit}`).toLocaleLowerCase();
