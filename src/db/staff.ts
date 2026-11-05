import { mediaColumns, takeMedia, type MediaColumns } from "./media";
import type { MediaFile } from "@/lib/media";
import { select } from "./client";

export interface Permission {
  key: string;
  group_key: string;
  is_dangerous: number;
}

export interface Role {
  id: number;
  name: string;
  description: string | null;
  is_system: number;
  user_count: number;
}

export interface Employee {
  id: number;
  username: string;
  full_name: string;
  role_id: number;
  role_name: string;
  phone: string | null;
  photo_media_id: number | null;
  photo: MediaFile | null;
  hire_date: string | null;
  wage_amount: number | null;
  wage_period: "hour" | "month" | null;
  is_active: number;
  has_pin: number;
  last_login: string | null;
}

export interface Schedule {
  id: number;
  user_id: number;
  starts_at: string;
  ends_at: string;
  note: string | null;
}

export const listPermissions = () =>
  select<Permission>("SELECT key, group_key, is_dangerous FROM permissions ORDER BY sort");

export const listRoles = () =>
  select<Role>(
    `SELECT r.id, r.name, r.description, r.is_system,
            (SELECT COUNT(*) FROM users u WHERE u.role_id = r.id) AS user_count
     FROM roles r ORDER BY r.is_system DESC, r.id`,
  );

export const listRolePermissions = async (roleId: number) =>
  (
    await select<{ permission_key: string }>(
      "SELECT permission_key FROM role_permissions WHERE role_id = $1",
      [roleId],
    )
  ).map((r) => r.permission_key);

// Never selects password_hash / pin_hash
export const listEmployees = async (): Promise<Employee[]> => {
  const rows = await select<Omit<Employee, "photo"> & MediaColumns>(
    `SELECT u.id, u.username, u.full_name, u.role_id,
            r.name AS role_name,
            u.phone, u.photo_media_id, u.hire_date, u.wage_amount, u.wage_period,
            u.is_active, u.pin_hash IS NOT NULL AS has_pin, u.last_login,
            ${mediaColumns("m")}
     FROM users u JOIN roles r ON r.id = u.role_id
     LEFT JOIN media_files m ON m.id = u.photo_media_id
     ORDER BY u.is_active DESC, u.full_name`,
  );
  return rows.map((row) => {
    const [photo, rest] = takeMedia(row);
    return { ...rest, photo };
  });
};

/** Schedules starting in [from, to), as RFC 3339 UTC strings. */
export const listSchedules = (from: string, to: string) =>
  select<Schedule>(
    `SELECT id, user_id, starts_at, ends_at, note FROM shift_schedules
     WHERE starts_at >= $1 AND starts_at < $2 ORDER BY starts_at`,
    [from, to],
  );
