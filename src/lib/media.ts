import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";
import { inDesktopApp } from "@/db/client";

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

/** Image processing happens in Rust, so uploads only work in the desktop app. */
export const canUploadMedia = inDesktopApp;

export const ACCEPTED_IMAGE_TYPES = "image/jpeg,image/png,image/webp,image/gif,image/bmp";

/** Sends the raw file to Rust, which converts it to WebP (q80) and computes a blurhash. */
export const uploadMedia = async (file: File): Promise<MediaFile> => {
  if (!canUploadMedia) throw new Error("Image upload works in the desktop app only.");
  const bytes = new Uint8Array(await file.arrayBuffer());
  return invoke<MediaFile>("upload_media", bytes, {
    headers: { "x-file-name": encodeURIComponent(file.name) },
  });
};

let mediaDir: Promise<string> | null = null;

/** asset:// URL for a stored file, or null outside the desktop app. */
export const mediaUrl = async (media: Pick<MediaFile, "path">) => {
  if (!inDesktopApp) return null;
  mediaDir ??= invoke<string>("get_media_dir");
  return convertFileSrc(await join(await mediaDir, media.path));
};
