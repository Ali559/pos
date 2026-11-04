import { nextTick, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getSale, type SaleDetail } from "@/db/sales";
import { getSetting } from "@/db/settings";
import { inDesktopApp } from "@/lib/ipc";
import { mediaUrl } from "@/lib/media";
import { useShop } from "@/stores/shop";

// The receipt is rendered into #print-root (PrintRoot.vue, mounted once in
// App.vue); print CSS hides everything else while the dialog is up.
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

/** Prints the sale's receipt. `copy` marks a reprint. */
export const printReceipt = async (saleId: number, opts: { copy?: boolean } = {}) => {
  const shop = useShop();
  await shop.load();
  const logo = shop.profile.value?.logo;
  const [sale, width, logoSrc] = await Promise.all([
    getSale(saleId),
    getSetting("receipt_width"),
    logo ? mediaUrl(logo) : null,
  ]);
  if (!sale) return;
  printing.value = { sale, width: width === "58" ? 58 : 80, copy: !!opts.copy, logo: logoSrc };
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
