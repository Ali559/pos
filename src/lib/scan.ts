/** What `resolve_scan` (src-tauri/src/scale.rs) says a scanned code is */
export interface ScanResult {
  productId: number;
  /** A pack size's barcode */
  unitId: number | null;
  /** In the pack, or the product's own unit (weight from a scale label) */
  quantity: number;
  /** A price label's total */
  amount: number | null;
  /** Set for scale labels; checkout decodes it again */
  scaleBarcode: string | null;
}
