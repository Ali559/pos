// Dev-only stand-in for tauri-plugin-sql when the UI runs in a plain browser.
// Runs the real migrations (and the demo catalog) in an in-memory sql.js
// database, so screens can be built and checked without the desktop shell.
// Nothing is persisted: a reload starts fresh.
import initSqlJs from "sql.js";
import wasmUrl from "sql.js/dist/sql-wasm.wasm?url";
import type { Db } from "./client";

const migrations = import.meta.glob<string>("../../src-tauri/migrations/*.sql", {
  query: "?raw",
  import: "default",
  eager: true,
});
import demoSeed from "../../src-tauri/seeds/demo.sql?raw";

type Bindable = number | string | Uint8Array | null;

// plugin-sql returns booleans/undefined as SQLite values; mirror that
const toBindable = (v: unknown): Bindable =>
  v === undefined || v === null
    ? null
    : typeof v === "boolean"
      ? Number(v)
      : (v as Bindable);

export const createBrowserDb = async (): Promise<Db> => {
  const SQL = await initSqlJs({ locateFile: () => wasmUrl });
  const sqlite = new SQL.Database();
  sqlite.run("PRAGMA foreign_keys = ON");
  for (const path of Object.keys(migrations).sort()) {
    sqlite.exec(migrations[path]);
  }
  sqlite.exec(demoSeed);
  console.info("[db] browser fallback: in-memory SQLite with demo data");

  return {
    async select<T>(sql: string, params: unknown[] = []) {
      const stmt = sqlite.prepare(sql);
      stmt.bind(params.map(toBindable));
      const rows: T[] = [];
      while (stmt.step()) rows.push(stmt.getAsObject() as T);
      stmt.free();
      return rows;
    },
    async execute(sql: string, params: unknown[] = []) {
      sqlite.run(sql, params.map(toBindable));
      const id = sqlite.exec("SELECT last_insert_rowid()")[0]?.values[0][0];
      return {
        rowsAffected: sqlite.getRowsModified(),
        lastInsertId: Number(id),
      };
    },
  };
};
