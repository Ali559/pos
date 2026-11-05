import { nextTick, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getSale, type SaleDetail } from "@/db/sales";
import { getSetting } from "@/db/settings";
import { inDesktopApp } from "@/lib/ipc";
import { call } from "@/lib/ipc";
import { mediaUrl } from "@/lib/media";
import { renderReceipt } from "@/lib/receiptRaster";
import { useShop } from "@/stores/shop";

// Two ways out: with a receipt printer set (`receipt_printer`), the receipt
// is drawn to a bitmap and Rust sends it as ESC/POS, no dialog. Otherwise
// it's rendered into #print-root (PrintRoot.vue, mounted once in App.vue)
// and the system print dialog opens; print CSS hides everything else.
export const printing = ref<{ sale: SaleDetail; width: 58 | 80; copy: boolean; logo: string | null } | null>(null);

const imagesLoaded = (root: Element) =>
  Promise.all(
    [...root.querySelectorAll("img")]
      .filter((img) => !img.complete)
      .map((img) => new Promise((done) => {
        img.addEventListener("load", done, { once: true });
        img.addEventListener("error", done, { once: true });
      })),
  );

/** Draws and sends a receipt to the receipt printer. */
const printDirect = async (job: NonNullable<typeof printing.value>, openDrawer: boolean, target?: string) => {
  const shop = useShop().profile.value;
  let raster;
  try {
    raster = await renderReceipt(job.sale, shop, job);
  } catch {
    // A logo the canvas can't read back (cross-origin) taints it: go without
    raster = await renderReceipt(job.sale, shop, { ...job, logo: null });
  }
  await call("print_raster", { ...raster, openDrawer, target: target ?? null });
};

/** Prints the sale's receipt. `copy` marks a reprint; `openDrawer` kicks the
 *  cash drawer when the receipt printer drives one. */
export const printReceipt = async (saleId: number, opts: { copy?: boolean; openDrawer?: boolean; target?: string } = {}) => {
  const shop = useShop();
  await shop.load();
  const logo = shop.profile.value?.logo;
  const [sale, width, logoSrc, printer] = await Promise.all([
    getSale(saleId),
    getSetting("receipt_width"),
    logo ? mediaUrl(logo) : null,
    getSetting("receipt_printer"),
  ]);
  if (!sale) return;
  const job = { sale, width: width === "58" ? (58 as const) : (80 as const), copy: !!opts.copy, logo: logoSrc };
  if (opts.target || printer) return printDirect(job, !!opts.openDrawer, opts.target);
  printing.value = job;
  await nextTick();
  const root = document.getElementById("print-root");
  if (root) await imagesLoaded(root);
  try {
    // window.print() is a no-op in some webviews; Rust asks the webview itself
    if (inDesktopApp) await invoke("print_receipt");
    else window.print();
  } finally {
    // Leave it rendered until the dialog closes (print is async on macOS)
    setTimeout(() => (printing.value = null), 1500);
  }
};

/** A sample receipt to check a printer before saving it. */
export const printTestPage = async (target: string) => {
  const shop = useShop();
  await shop.load();
  const now = new Date().toISOString();
  const sale: SaleDetail = {
    id: 0, receipt_number: "R-000000", sale_date: now, cashier: "—", approver: null,
    subtotal: 1500, discount: 0, tax: 0, total: 1500, status: "completed",
    method: "cash", tendered: 2000, change: 500, refunded: 0,
    payments: [{ method: "cash", amount: 1500, tendered: 2000 }],
    lines: [{ id: 0, product_id: 0, name: "١٢٣ ABC کوردی عربي", quantity: 2, unit_price: 750, discount: 0, subtotal: 1500, tax_rate: 0, returned: 0, unit: "piece" }],
    returns: [],
  };
  const width = (await getSetting("receipt_width")) === "58" ? 58 : 80;
  await printDirect({ sale, width, copy: true, logo: null }, false, target);
};
