// Slips for the end of a shift and the end of the day, in the shop's
// language. Printed with printSlip (receipt printer or print dialog).
import type { DayReport } from "@/db/reports";
import { formatCurrency, formatUsd } from "@/lib/format";
import { formatDate, formatDateTime } from "@/lib/locale";
import type { SlipLine } from "@/lib/slip";
import { useShop } from "@/stores/shop";
import { i18n } from "@/utils/i18n";

const t = (key: string, values?: Record<string, unknown>) => i18n.global.t(key, values ?? {});
const money = (n: number) => formatCurrency(n);
const row = (text: string, value: string, bold = false, indent = false): SlipLine => ({ kind: "row", text, value, bold, indent });

/** Mirrors shifts::ShiftSummary */
export interface ShiftSummary {
  shiftId: number;
  clockedInAt: string;
  openingFloat: number;
  openingCounted: boolean;
  sales: number;
  cashSales: number;
  cardSales: number;
  cashRefunds: number;
  cardRefunds: number;
  debtPaidCash: number;
  debtPaidCard: number;
  onAccount: number;
  paidToSuppliers: number;
  expectedCash: number;
  salesTotal: number;
  discounts: number;
  promotions: number;
  tax: number;
  pointsPaid: number;
  returns: number;
  returnsTotal: number;
  /** Dollars, when the shop takes them (see shifts.rs) */
  usdEnabled: boolean;
  openingUsd: number;
  usdIn: number;
  usdSales: number;
  /** Dinars given back as change for dollars */
  usdChange: number;
  paidToSuppliersUsd: number;
  expectedUsd: number;
}

/** The drawer holds dollars worth showing */
export const hasUsd = (s: ShiftSummary) => s.usdEnabled && (s.openingUsd !== 0 || s.usdIn !== 0 || s.paidToSuppliersUsd !== 0);

const header = (title: string): SlipLine[] => {
  const shop = useShop().profile.value;
  return [
    ...(shop?.name ? [{ kind: "title" as const, text: shop.name }] : []),
    { kind: "center", text: title },
    { kind: "center", text: formatDateTime(new Date()) },
    { kind: "rule" },
  ];
};

export const shiftSlip = (s: ShiftSummary, who: string, counted: number | null, countedUsd: number | null = null): SlipLine[] => {
  const lines: SlipLine[] = [
    ...header(t("reports.shiftTitle")),
    row(t("receipt.cashier"), who),
    row(t("reports.started"), formatDateTime(new Date(s.clockedInAt))),
    row(t("reports.sales"), String(s.sales)),
    row(t("reports.salesTotal"), money(s.salesTotal), true),
  ];
  if (s.discounts) lines.push(row(t("reports.discounts"), `−${money(s.discounts)}`));
  if (s.promotions) lines.push(row(t("reports.promotions"), `−${money(s.promotions)}`));
  if (s.tax) lines.push(row(t("pos.tax"), money(s.tax)));
  if (s.returns) lines.push(row(t("reports.returns", { n: s.returns }), `−${money(s.returnsTotal)}`));
  lines.push({ kind: "heading", text: t("reports.drawer") });
  lines.push(row(t("shiftEnd.float"), money(s.openingFloat), false, true));
  lines.push(row(t("shiftEnd.cashSales"), `+${money(s.cashSales)}`, false, true));
  if (s.cashRefunds) lines.push(row(t("shiftEnd.cashRefunds"), money(s.cashRefunds), false, true));
  if (s.debtPaidCash) lines.push(row(t("shiftEnd.debtPaid"), `+${money(s.debtPaidCash)}`, false, true));
  if (s.paidToSuppliers) lines.push(row(t("shiftEnd.paidToSuppliers"), money(s.paidToSuppliers), false, true));
  if (s.usdChange) lines.push(row(t("shiftEnd.usdChange"), `−${money(s.usdChange)}`, false, true));
  lines.push(row(t("shiftEnd.expected"), money(s.expectedCash), true));
  if (counted !== null) {
    lines.push(row(t("shiftEnd.counted"), money(counted), true));
    const diff = counted - s.expectedCash;
    lines.push(row(diff === 0 ? t("shiftEnd.balanced") : diff > 0 ? t("reports.over") : t("reports.short"), diff === 0 ? "" : money(Math.abs(diff)), true));
  }
  if (hasUsd(s)) {
    lines.push({ kind: "heading", text: t("shiftEnd.dollars") });
    lines.push(row(t("shiftEnd.float"), formatUsd(s.openingUsd), false, true));
    if (s.usdIn) lines.push(row(t("shiftEnd.usdIn"), `+${formatUsd(s.usdIn)}`, false, true));
    if (s.paidToSuppliersUsd) lines.push(row(t("shiftEnd.paidToSuppliers"), formatUsd(s.paidToSuppliersUsd), false, true));
    lines.push(row(t("shiftEnd.expected"), formatUsd(s.expectedUsd), true));
    if (countedUsd !== null) {
      lines.push(row(t("shiftEnd.counted"), formatUsd(countedUsd), true));
      const diff = Math.round((countedUsd - s.expectedUsd) * 100) / 100;
      lines.push(row(diff === 0 ? t("shiftEnd.balanced") : diff > 0 ? t("reports.over") : t("reports.short"), diff === 0 ? "" : formatUsd(Math.abs(diff)), true));
    }
  }
  lines.push({ kind: "rule" });
  const card = s.cardSales + s.cardRefunds + s.debtPaidCard;
  if (card) lines.push(row(t("shiftEnd.cardSales"), money(card)));
  if (s.onAccount) lines.push(row(t("shiftEnd.onAccount"), money(s.onAccount)));
  if (s.pointsPaid) lines.push(row(t("reports.pointsPaid"), money(s.pointsPaid)));
  return lines;
};

const methodName = (m: string) => (i18n.global.te(`pos.${m}`) ? t(`pos.${m}`) : m);

export const daySlip = (r: DayReport, label: string): SlipLine[] => {
  const x = r.totals;
  const lines: SlipLine[] = [
    ...header(t("reports.dayTitle")),
    { kind: "center", text: label },
    { kind: "rule" },
    row(t("reports.sales"), String(x.sales)),
    row(t("reports.salesTotal"), money(x.total), true),
  ];
  if (x.discounts) lines.push(row(t("reports.discounts"), `−${money(x.discounts)}`));
  if (x.promotions) lines.push(row(t("reports.promotions"), `−${money(x.promotions)}`));
  if (x.tax) lines.push(row(t("pos.tax"), money(x.tax)));
  if (x.returns) lines.push(row(t("reports.returns", { n: x.returns }), `−${money(x.refunded)}`));
  if (r.methods.length) {
    lines.push({ kind: "heading", text: t("reports.byMethod") });
    for (const m of r.methods) {
      lines.push(row(methodName(m.method), money(m.taken + m.refunded), false, true));
      if (m.refunded) lines.push(row(t("reports.ofWhichRefunded"), money(m.refunded), false, true));
    }
  }
  if (x.debtCash || x.debtCard) lines.push(row(t("reports.debtRepaid"), money(x.debtCash + x.debtCard)));
  if (x.suppliers) lines.push(row(t("shiftEnd.paidToSuppliers"), money(x.suppliers)));
  if (r.cashiers.length) {
    lines.push({ kind: "heading", text: t("reports.byCashier") });
    for (const c of r.cashiers) {
      lines.push(row(c.full_name, money(c.total), true));
      lines.push(row(t("reports.sales"), String(c.sales), false, true));
      if (c.cash) lines.push(row(t("pos.cash"), money(c.cash), false, true));
      if (c.card) lines.push(row(t("pos.card"), money(c.card), false, true));
      if (c.discounts) lines.push(row(t("reports.discounts"), `−${money(c.discounts)}`, false, true));
      if (c.refunded) lines.push(row(t("reports.refunds"), `−${money(c.refunded)}`, false, true));
      if (c.drawer !== null) {
        lines.push(row(c.drawer === 0 ? t("shiftEnd.balanced") : c.drawer > 0 ? t("reports.over") : t("reports.short"), c.drawer === 0 ? "" : money(Math.abs(c.drawer)), false, true));
      }
      if (c.drawer_usd) {
        lines.push(row(c.drawer_usd > 0 ? t("reports.over") : t("reports.short"), formatUsd(Math.abs(c.drawer_usd)), false, true));
      }
    }
  }
  lines.push({ kind: "rule" });
  lines.push({ kind: "center", text: t("reports.printedOn", { date: formatDate(new Date()) }) });
  return lines;
};
