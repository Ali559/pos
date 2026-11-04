import { select } from "./client";
import type { MediaFile } from "@/lib/media";

export interface ExpenseType {
  id: number;
  name: string;
  icon_key: string;
  color: string | null;
  is_active: number;
}

export interface Expense {
  id: number;
  type_id: number;
  amount: number;
  spent_on: string;
  note: string | null;
  recurring: "weekly" | "monthly" | null;
  recurring_parent_id: number | null;
  status: "confirmed" | "draft";
  type_name: string;
  type_icon: string;
  type_color: string | null;
  receipt: MediaFile | null;
}

export const listExpenseTypes = (includeArchived = false) =>
  select<ExpenseType>(
    `SELECT id, name, icon_key, color, is_active FROM expense_types
     ${includeArchived ? "" : "WHERE is_active = 1"} ORDER BY display_order, id`,
  );

type Row = Omit<Expense, "receipt"> & {
  m_id: number | null; m_uuid: string; m_path: string; m_mime: string;
  m_width: number; m_height: number; m_bytes: number; m_blurhash: string;
};

/** `from`/`to` are local dates, `to` exclusive. Drafts are always included. */
export const listExpenses = async (q: { from: string; to: string; typeId?: number | null }) => {
  const params: unknown[] = [q.from, q.to];
  let typeFilter = "";
  if (q.typeId) {
    params.push(q.typeId);
    typeFilter = ` AND e.type_id = $3`;
  }
  const rows = await select<Row>(
    `SELECT e.id, e.type_id, e.amount, e.spent_on, e.note, e.recurring, e.recurring_parent_id, e.status,
            t.name AS type_name,
            t.icon_key AS type_icon, t.color AS type_color,
            m.id AS m_id, m.uuid AS m_uuid, m.path AS m_path, m.mime AS m_mime, m.width AS m_width,
            m.height AS m_height, m.bytes AS m_bytes, m.blurhash AS m_blurhash
     FROM expenses e
     JOIN expense_types t ON t.id = e.type_id
     LEFT JOIN media_files m ON m.id = e.receipt_media_id
     WHERE ((e.spent_on >= $1 AND e.spent_on < $2) OR e.status = 'draft')${typeFilter}
     ORDER BY e.status = 'draft' DESC, e.spent_on DESC, e.id DESC`,
    params,
  );
  return rows.map(({ m_id, m_uuid, m_path, m_mime, m_width, m_height, m_bytes, m_blurhash, ...e }) => ({
    ...e,
    receipt: m_id
      ? { id: m_id, uuid: m_uuid, path: m_path, mime: m_mime, width: m_width, height: m_height, bytes: m_bytes, blurhash: m_blurhash }
      : null,
  }));
};
