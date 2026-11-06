import { call } from "@/lib/ipc";

// Backups are made, listed and restored by Rust (src-tauri/src/backup.rs).
// Folders and files are picked with dialogs Rust opens; the WebView never
// names a path.

export interface BackupManifest {
  appVersion: string;
  schemaVersion: number | null;
  shopName: string | null;
  createdAt: string;
  kind: "auto" | "manual" | "before_restore";
  sales: number;
  products: number;
  mediaFiles: number;
}

export interface BackupFile {
  name: string;
  bytes: number;
  manifest: BackupManifest | null;
}

export interface BackupStatus {
  dir: string;
  isDefaultDir: boolean;
  dirAvailable: boolean;
  auto: boolean;
  keep: number;
  lastOkAt: string | null;
  lastAttemptAt: string | null;
  /** An error code (see errors.* in the locales) or an English message */
  lastError: string | null;
  files: BackupFile[];
}

export interface InspectedBackup {
  token: string;
  name: string;
  manifest: BackupManifest;
}

export const backupStatus = () => call<BackupStatus>("backup_status");
export const backupNow = () => call<BackupFile>("backup_now");
/** false if the dialog was cancelled */
export const chooseBackupFolder = (reset = false) => call<boolean>("choose_backup_folder", { reset });
/** name: a file in the backup folder; null opens a file dialog. null result = cancelled */
export const inspectBackup = (name: string | null) => call<InspectedBackup | null>("inspect_backup", { name });
/** Restarts the app on success */
export const restoreBackup = (token: string) => call("restore_backup", { token });
