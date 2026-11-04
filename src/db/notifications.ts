import { select } from "./client";

export type NotificationKind = "stock_low" | "stock_out" | "expiring" | "expired" | "expense_due";

export interface AppNotification {
  id: number;
  kind: NotificationKind;
  product_id: number | null;
  expense_id: number | null;
  payload: Record<string, unknown>;
  created_at: string;
  read_at: string | null;
}

/** Open, not dismissed, of the given kinds; newest first. */
export const listNotifications = async (kinds: NotificationKind[]) => {
  if (!kinds.length) return [];
  const marks = kinds.map((_, i) => `$${i + 1}`).join(", ");
  const rows = await select<Omit<AppNotification, "payload"> & { payload: string | null }>(
    `SELECT id, kind, product_id, expense_id, payload, created_at, read_at FROM notifications
     WHERE resolved_at IS NULL AND dismissed_at IS NULL AND kind IN (${marks})
     ORDER BY CASE kind WHEN 'stock_out' THEN 0 WHEN 'expired' THEN 1 ELSE 2 END, created_at DESC`,
    kinds,
  );
  return rows.map((r) => ({ ...r, payload: r.payload ? JSON.parse(r.payload) : {} }));
};
