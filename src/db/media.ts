// Joining a media_files row onto another query: the columns to select and
// the mapper that folds them back into one MediaFile (or null).
import type { MediaFile } from "@/lib/media";

export const mediaColumns = (alias: string) =>
  `${alias}.id AS m_id, ${alias}.uuid AS m_uuid, ${alias}.path AS m_path, ${alias}.mime AS m_mime,
   ${alias}.width AS m_width, ${alias}.height AS m_height, ${alias}.bytes AS m_bytes, ${alias}.blurhash AS m_blurhash`;

export interface MediaColumns {
  m_id: number | null;
  m_uuid: string;
  m_path: string;
  m_mime: string;
  m_width: number;
  m_height: number;
  m_bytes: number;
  m_blurhash: string;
}

/** Splits the joined columns off a row: [the media or null, the rest]. */
export const takeMedia = <T extends MediaColumns>(row: T): [MediaFile | null, Omit<T, keyof MediaColumns>] => {
  const { m_id, m_uuid, m_path, m_mime, m_width, m_height, m_bytes, m_blurhash, ...rest } = row;
  const media = m_id
    ? { id: m_id, uuid: m_uuid, path: m_path, mime: m_mime, width: m_width, height: m_height, bytes: m_bytes, blurhash: m_blurhash }
    : null;
  return [media, rest];
};
