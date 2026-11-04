import { selectOne } from "./client";
import { call } from "@/lib/ipc";
import type { MediaFile } from "@/lib/media";

export type ShopType = "mini_market" | "supermarket" | "other";

/** Names are in the shop's one language, chosen at setup. */
export interface ShopProfile {
  name: string;
  shop_type: ShopType;
  logo_media_id: number | null;
  phone: string | null;
  phone_alt: string | null;
  address: string | null;
  tax_number: string | null;
  receipt_footer: string | null;
  currency: string;
}

export interface ShopProfileWithLogo extends ShopProfile {
  logo: MediaFile | null;
}

const FIELDS: (keyof ShopProfile)[] = [
  "name", "shop_type", "logo_media_id", "phone", "phone_alt", "address",
  "tax_number", "receipt_footer", "currency",
];

export const emptyShopProfile = (): ShopProfile => ({
  name: "", shop_type: "mini_market", logo_media_id: null, phone: null,
  phone_alt: null, address: null, tax_number: null, receipt_footer: null,
  currency: "IQD",
});

export const getShopProfile = async (): Promise<ShopProfileWithLogo | null> => {
  const row = await selectOne<
    ShopProfile & {
      m_id: number | null; m_uuid: string; m_path: string; m_mime: string;
      m_width: number; m_height: number; m_bytes: number; m_blurhash: string;
    }
  >(
    `SELECT s.${FIELDS.join(", s.")},
            m.id AS m_id, m.uuid AS m_uuid, m.path AS m_path, m.mime AS m_mime,
            m.width AS m_width, m.height AS m_height, m.bytes AS m_bytes,
            m.blurhash AS m_blurhash
     FROM shop_profile s
     LEFT JOIN media_files m ON m.id = s.logo_media_id
     WHERE s.id = 1`,
  );
  if (!row) return null;
  const profile = Object.fromEntries(FIELDS.map((f) => [f, row[f]])) as unknown as ShopProfile;
  return {
    ...profile,
    logo: row.m_id
      ? {
          id: row.m_id, uuid: row.m_uuid, path: row.m_path, mime: row.m_mime,
          width: row.m_width, height: row.m_height, bytes: row.m_bytes,
          blurhash: row.m_blurhash,
        }
      : null,
  };
};

/** Creates or replaces the single shop_profile row (needs `settings.shop`). */
export const saveShopProfile = (profile: ShopProfile) =>
  call("save_shop_profile", { profile });
