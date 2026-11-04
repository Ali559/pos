import { getDb, selectOne } from "./client";

export const getSetting = async (key: string) =>
  (await selectOne<{ value: string | null }>(
    "SELECT value FROM settings WHERE key = $1",
    [key],
  ))?.value ?? null;

export const setSetting = async (key: string, value: string) => {
  await (await getDb()).execute(
    `INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, CURRENT_TIMESTAMP)
     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP`,
    [key, value],
  );
};
