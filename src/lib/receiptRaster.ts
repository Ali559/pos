// Draws a receipt onto a canvas at the printer's resolution (203 dpi:
// 384 dots across 58 mm paper, 576 across 80 mm) and packs it into the
// 1-bit rows ESC/POS expects. The browser lays out the text, so Kurdish and
// Arabic come out shaped and right-to-left, which printer fonts can't do.
import { lineSold, type SaleDetail } from "@/db/sales";
import type { ShopProfile } from "@/db/shop";
import { code128Bars } from "@/lib/code128";
import { formatCurrency } from "@/lib/format";
import { formatDateTime } from "@/lib/locale";
import { i18n } from "@/utils/i18n";

export interface Raster {
  widthBytes: number;
  height: number;
  /** Base64 of the packed rows, 1 = black, leftmost dot in the high bit */
  data: string;
}

const DOTS = { 58: 384, 80: 576 } as const;
const MAX_HEIGHT = 4000;

const t = (key: string, values?: Record<string, unknown>) => i18n.global.t(key, values ?? {});
// Left-to-right values (phones, receipt numbers) inside right-to-left text:
// isolated, or the bidi algorithm reorders their digit groups
const ltr = (s: string) => `\u2066${s}\u2069`;

const loadImage = (src: string) =>
  new Promise<HTMLImageElement | null>((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = src;
  });

/** Packs a canvas into 1-bit rows (dark pixels print). */
export const packBits = (pixels: Uint8ClampedArray, width: number, height: number) => {
  const widthBytes = width / 8;
  const out = new Uint8Array(widthBytes * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4;
      const alpha = pixels[i + 3]! / 255;
      // Transparent counts as paper (white)
      const lum = (0.299 * pixels[i]! + 0.587 * pixels[i + 1]! + 0.114 * pixels[i + 2]!) * alpha + 255 * (1 - alpha);
      if (lum < 150) out[y * widthBytes + (x >> 3)]! |= 0x80 >> (x & 7);
    }
  }
  return out;
};

const toBase64 = (bytes: Uint8Array) => {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
};

export const renderReceipt = async (
  sale: SaleDetail,
  shop: ShopProfile | null,
  opts: { width: 58 | 80; copy: boolean; logo: string | null },
): Promise<Raster> => {
  const W = DOTS[opts.width];
  const pad = 8;
  const base = opts.width === 80 ? 24 : 19;
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
  const startX = rtl ? W - pad : pad;
  const endX = rtl ? pad : W - pad;

  // Breaks text into lines that fit `max` dots
  const wrap = (text: string, max: number) => {
    const words = text.split(/\s+/);
    const lines: string[] = [];
    let line = "";
    for (const w of words) {
      const next = line ? `${line} ${w}` : w;
      if (ctx.measureText(next).width > max && line) {
        lines.push(line);
        line = w;
      } else line = next;
    }
    if (line) lines.push(line);
    return lines;
  };
  const center = (text: string, size = base, bold = false) => {
    font(size, bold);
    ctx.textAlign = "center";
    for (const line of wrap(text, W - 2 * pad)) {
      ctx.fillText(line, W / 2, y);
      y += Math.round(size * 1.35);
    }
  };
  const row = (start: string, end: string, size = base, bold = false) => {
    font(size, bold);
    const endWidth = ctx.measureText(end).width;
    const lines = wrap(start, W - 2 * pad - endWidth - 12);
    ctx.textAlign = "end";
    ctx.fillText(end, endX, y);
    ctx.textAlign = "start";
    for (const line of lines) {
      ctx.fillText(line, startX, y);
      y += Math.round(size * 1.35);
    }
  };
  const rule = () => {
    y += 6;
    for (let x = pad; x < W - pad; x += 10) ctx.fillRect(x, y, 6, 2);
    y += 10;
  };

  if (opts.logo) {
    const img = await loadImage(opts.logo);
    if (img) {
      const h = Math.min(110, img.height);
      const w = Math.min(W * 0.6, (img.width * h) / img.height);
      const hh = (img.height * w) / img.width;
      ctx.drawImage(img, (W - w) / 2, y, w, hh);
      y += hh + 8;
    }
  }
  if (shop?.name) center(shop.name, Math.round(base * 1.4), true);
  if (shop?.address) center(shop.address);
  const phones = [shop?.phone, shop?.phone_alt].filter(Boolean).join(" / ");
  if (phones) center(ltr(phones));
  if (shop?.tax_number) center(`${t("receipt.taxNumber")}: ${ltr(shop.tax_number)}`);
  if (opts.copy) {
    y += 4;
    ctx.fillRect(pad, y, W - 2 * pad, base + 12);
    ctx.fillStyle = "#fff";
    center(t("receipt.copy"), base, true);
    ctx.fillStyle = "#000";
  }
  rule();
  row(t("receipt.number"), ltr(sale.receipt_number));
  row(t("receipt.date"), formatDateTime(new Date(sale.sale_date)));
  row(t("receipt.cashier"), sale.cashier);
  if (sale.customer_name) row(t("receipt.customer"), sale.customer_name);
  rule();

  for (const l of sale.lines) {
    font(base, true);
    ctx.textAlign = "start";
    for (const line of wrap(l.name, W - 2 * pad)) {
      ctx.fillText(line, startX, y);
      y += Math.round(base * 1.3);
    }
    const sold = lineSold(l);
    row(`${sold.label} × ${formatCurrency(sold.each)}`, formatCurrency(l.subtotal + l.discount));
    if (l.promo_discount > 0) row(`  ${l.promotion_name ?? ""}`, `−${formatCurrency(l.promo_discount)}`);
    if (l.discount - l.promo_discount > 0.0001) row(`  ${t("receipt.discount")}`, `−${formatCurrency(l.discount - l.promo_discount)}`);
  }
  rule();
  row(t("pos.subtotal"), formatCurrency(sale.subtotal));
  if (sale.discount > 0) row(t("receipt.discount"), `−${formatCurrency(sale.discount)}`);
  if (sale.tax > 0) row(t("pos.tax"), formatCurrency(sale.tax));
  y += 4;
  row(t("pos.total"), formatCurrency(sale.total), Math.round(base * 1.3), true);
  for (const p of sale.payments) row(t(`pos.${p.method}`), formatCurrency(p.tendered ?? p.amount));
  if (sale.change) row(t("cash.change"), formatCurrency(sale.change));
  if (sale.refunded > 0) row(t("receipt.refunded"), `−${formatCurrency(sale.refunded)}`);
  if (sale.customer_balance) row(t("receipt.balanceOwed"), formatCurrency(sale.customer_balance), base, true);

  // Barcode of the receipt number, whole dots per module so it scans
  const { bars, width } = code128Bars(sale.receipt_number);
  const module = Math.max(2, Math.floor((W - 4 * pad) / (width + 20)));
  const left = Math.round((W - module * width) / 2);
  y += 14;
  for (const [x, w] of bars) ctx.fillRect(left + x * module, y, w * module, 70);
  y += 76;
  center(ltr(sale.receipt_number));
  if (shop?.receipt_footer) {
    y += 8;
    for (const para of shop.receipt_footer.split("\n")) center(para);
  }
  y += pad;

  const height = Math.min(MAX_HEIGHT, Math.ceil(y));
  const pixels = ctx.getImageData(0, 0, W, height).data;
  return { widthBytes: W / 8, height, data: toBase64(packBits(pixels, W, height)) };
};
