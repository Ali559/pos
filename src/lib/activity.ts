// Turns an activity-log entry into a sentence in the shop's language. Each
// action code has a message under `activity.actions` (dots become
// underscores: "product.create" → activity.actions.product_create) and the
// entry's details fill it in. Unknown codes show as they are.
import type { AuditEntry } from "@/db/audit";
import { formatCurrency } from "@/lib/format";
import { formatDateTime } from "@/lib/locale";
import { i18n } from "@/utils/i18n";

type Details = Record<string, unknown>;

const { t, te } = i18n.global;
// Names and numbers inside a sentence keep their own direction
const iso = (v: unknown) => `⁨${v ?? ""}⁩`;
const money = (v: unknown) => iso(formatCurrency(Number(v) || 0));
const when = (v: unknown) => (typeof v === "string" && v ? iso(formatDateTime(new Date(v))) : "");
const tr = (key: string, fallback: string) => (te(key) ? t(key) : fallback);

const settingValue = (key: string, value: unknown) => {
  const v = String(value ?? "");
  if (v === "1" || v === "0") return t(v === "1" ? "activity.on" : "activity.off");
  if (key === "receipt_printer" && !v) return t("activity.printDialog");
  if (["opening_float", "loyalty_earn_per", "loyalty_point_value"].includes(key)) return money(v);
  return iso(v);
};

/** The parameters each message uses. */
const params = (e: AuditEntry, d: Details, users: Map<number, string>): Record<string, unknown> => {
  const target = iso(e.target ?? "—");
  const p: Record<string, unknown> = { target };
  switch (e.action) {
    case "login":
      p.via = t(`activity.via.${d.via === "pin" ? "pin" : "password"}`);
      break;
    case "override":
      p.permission = iso(tr(`permissions.${d.permission}`, String(d.permission ?? "")));
      break;
    case "drawer.open":
    case "drawer.close":
      p.counted = money(d.counted);
      p.expected = money(d.expected);
      break;
    case "schedule.save":
      p.who = iso(users.get(Number(d.user)) ?? "—");
      p.start = when(d.start);
      p.end = when(d.end);
      break;
    case "schedule.copy_week":
      p.n = Number(d.copied) || 0;
      break;
    case "role.create":
    case "role.update":
      p.name = iso(d.name);
      p.n = Array.isArray(d.permissions) ? d.permissions.length : 0;
      break;
    case "user.create":
    case "user.update":
      p.username = iso(d.username);
      break;
    case "setting": {
      const [key, value] = Object.entries(d)[0] ?? ["", ""];
      p.setting = iso(tr(`activity.settings.${key}`, key));
      p.value = settingValue(key, value);
      break;
    }
    case "category.create":
    case "category.update":
    case "product.create":
    case "product.update":
    case "expense_type.create":
    case "customer.create":
    case "supplier.create":
    case "promotion.save":
    case "till.pair":
      p.name = iso(d.name ?? e.target);
      p.stock = iso(d.stock ?? 0);
      break;
    case "sale":
      p.receipt = iso(d.receipt ?? e.target ?? "");
      p.total = money(d.total);
      p.n = Number(d.items) || 0;
      break;
    case "sale.void_line":
      p.n = Array.isArray(d.lines) ? d.lines.length : Number(d.lines) || 1;
      break;
    case "refund":
      p.amount = money(d.refund);
      p.receipt = target;
      break;
    case "stock.receive":
    case "stock.remove":
      p.quantity = iso(d.quantity);
      p.reason = d.reason ? tr(`stock.reasons.${d.reason}`, String(d.reason)) : "";
      break;
    case "stock.count":
      p.n = Number(d.changed) || 0;
      break;
    case "expense.create":
    case "expense.update":
    case "customer.payment":
    case "customer.adjust":
    case "supplier.payment":
    case "supplier.adjust":
      p.amount = money(d.amount);
      p.method = d.method ? tr(`pos.${d.method}`, String(d.method)) : "";
      p.note = iso(d.note ?? "");
      break;
    case "loyalty.adjust":
      p.points = iso(`${Number(d.points) > 0 ? "+" : ""}${d.points}`);
      p.note = iso(d.note ?? "");
      break;
    case "delivery":
      p.total = money(d.total);
      p.paid = money(d.paid);
      p.n = Number(d.lines) || 0;
      break;
    case "prices.bulk":
      p.n = Number(d.changed) || 0;
      break;
    case "import":
      p.created = Number(d.created) || 0;
      p.updated = Number(d.updated) || 0;
      break;
    case "export":
    case "backup":
      p.file = iso(d.file ?? "");
      p.n = Number(d.rows) || 0;
      break;
    case "backup.restore":
      p.date = when(d.created_at);
      break;
  }
  return p;
};

/** Which message an entry uses: some actions read differently by outcome. */
const messageKey = (e: AuditEntry, d: Details) => {
  const base = e.action.replace(/\./g, "_");
  if (e.action === "override") return d.granted ? "override_ok" : "override_failed";
  if (e.action === "drawer.close" && (d.counted === null || d.counted === undefined)) return "drawer_close_uncounted";
  if (e.action === "category.active") return d.active ? "category_shown" : "category_archived";
  if (e.action === "promotion.delete" && d.archived) return "promotion_archived";
  if (e.action === "refund" && !e.target) return "refund_no_receipt";
  if (e.action === "stock.remove" && d.reason) return "stock_remove_reason";
  return base;
};

/** Extra notes after the sentence (discounts, who approved, what changed). */
const extras = (e: AuditEntry, d: Details, users: Map<number, string>) => {
  const out: string[] = [];
  if (e.action === "sale") {
    if (Number(d.discount) > 0) out.push(t("activity.extra.discount", { amount: money(d.discount) }));
    if (Number(d.onAccount) > 0) out.push(t("activity.extra.onAccount", { amount: money(d.onAccount) }));
    if (Number(d.card) > 0) out.push(t("activity.extra.card", { amount: money(d.card) }));
    if (Number(d.points) > 0) out.push(t("activity.extra.points", { n: d.points }));
  }
  if (e.action === "user.update") {
    if (d.passwordChanged) out.push(t("activity.extra.password"));
    if (d.pinChanged) out.push(t("activity.extra.pin"));
    if (d.active === false) out.push(t("activity.extra.inactive"));
  }
  if (e.action === "refund" && d.method) out.push(tr(`pos.${d.method}`, String(d.method)));
  if (d.approvedBy) out.push(t("activity.extra.approvedBy", { name: iso(users.get(Number(d.approvedBy)) ?? "—") }));
  return out;
};

export const describe = (e: AuditEntry, users: Map<number, string>) => {
  let d: Details = {};
  try {
    d = e.details ? (JSON.parse(e.details) as Details) : {};
  } catch {
    d = {};
  }
  const key = `activity.actions.${messageKey(e, d)}`;
  const text = te(key) ? t(key, params(e, d, users)) : e.action;
  const more = extras(e, d, users);
  return more.length ? `${text} · ${more.join(" · ")}` : text;
};
