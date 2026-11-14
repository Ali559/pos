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

let base: Promise<{ dir: string | null; url: string | null; token: string | null }> | null = null;

/** URL for a stored file: asset:// from this computer's media folder, the
 *  main computer when this is an extra till (through the app's `tillmedia:`
 *  protocol), or the dev bridge in a browser. */
export const mediaUrl = async (media: Pick<MediaFile, "path">) => {
  if (!inDesktopApp) return import.meta.env.DEV ? `/__bridge/media/${media.path}` : null;
  base ??= invoke("media_base");
  const b = await base;
  if (b.url) return `${b.url}${encodeURIComponent(media.path)}${b.token ? `?t=${b.token}` : ""}`;
  return convertFileSrc(await join(b.dir!, media.path));
};
