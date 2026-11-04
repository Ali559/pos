import { isTauri } from "@tauri-apps/api/core";

// The same database URL the Rust side preloads and migrates (src-tauri/src/db.rs)
export const DB_URL = "sqlite:pos.db";

export interface QueryResult {
  rowsAffected: number;
  lastInsertId?: number;
}

export interface Db {
  select<T>(sql: string, params?: unknown[]): Promise<T[]>;
  execute(sql: string, params?: unknown[]): Promise<QueryResult>;
}

/** True when running inside the desktop app (not the plain-browser dev preview). */
export const inDesktopApp = isTauri();

let db: Promise<Db> | null = null;

const connect = async (): Promise<Db> => {
  if (inDesktopApp) {
    const { default: Database } = await import("@tauri-apps/plugin-sql");
    return Database.load(DB_URL);
  }
  // `bun run dev` in a normal browser: an in-memory SQLite (sql.js) running
  // the same migrations. Stripped from production builds.
  if (import.meta.env.DEV) {
    const { createBrowserDb } = await import("./browser");
    return createBrowserDb();
  }
  throw new Error("The database is only available in the desktop app.");
};

export const getDb = () => (db ??= connect());

/** First row or null. */
export const selectOne = async <T>(sql: string, params?: unknown[]) =>
  (await (await getDb()).select<T>(sql, params))[0] ?? null;
