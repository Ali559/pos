import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";
import { callUpload, inDesktopApp } from "@/lib/ipc";

/** A row of `media_files`, as returned by the Rust `upload_media` command. */
export interface MediaFile {
  id: number;
  uuid: string;
  path: string;
  mime: string;
  width: number;
  height: number;
  bytes: number;
  blurhash: string;
}

/** Image processing happens in Rust: the desktop app, or its dev bridge. */
export const canUploadMedia = inDesktopApp || import.meta.env.DEV;

// HEIC (iPhone photos) is converted on macOS; elsewhere Rust says so
export const ACCEPTED_IMAGE_TYPES = "image/jpeg,image/png,image/webp,image/gif,image/bmp,image/heic,image/heif,.heic,.heif";

/** Sends the raw file to Rust, which converts it to WebP (q80) and computes a blurhash. */
export const uploadMedia = async (file: File): Promise<MediaFile> => {
  const bytes = new Uint8Array(await file.arrayBuffer());
  return callUpload<MediaFile>(bytes, file.name);
};

let mediaDir: Promise<string> | null = null;

/** asset:// URL for a stored file (the dev bridge serves it in a browser). */
export const mediaUrl = async (media: Pick<MediaFile, "path">) => {
  if (!inDesktopApp) return import.meta.env.DEV ? `/__bridge/media/${media.path}` : null;
  mediaDir ??= invoke<string>("get_media_dir");
  return convertFileSrc(await join(await mediaDir, media.path));
};
