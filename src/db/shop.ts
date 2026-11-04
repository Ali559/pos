import { selectOne } from "./client";
import { call } from "@/lib/ipc";
import type { MediaFile } from "@/lib/media";

export interface ShopProfile {
  name_en: string;
  name_ku: string;
  name_ar: string | null;
  logo_media_id: number | null;
  phone: string | null;
  phone_alt: string | null;
  address_en: string | null;
  address_ku: string | null;
  address_ar: string | null;
  tax_number: string | null;
  receipt_footer_en: string | null;
  receipt_footer_ku: string | null;
  receipt_footer_ar: string | null;
  currency: string;
}

export interface ShopProfileWithLogo extends ShopProfile {
  logo: MediaFile | null;
}

const FIELDS: (keyof ShopProfile)[] = [
  "name_en", "name_ku", "name_ar", "logo_media_id", "phone", "phone_alt",
  "address_en", "address_ku", "address_ar", "tax_number",
  "receipt_footer_en", "receipt_footer_ku", "receipt_footer_ar", "currency",
];

export const emptyShopProfile = (): ShopProfile => ({
  name_en: "", name_ku: "", name_ar: null, logo_media_id: null,
  phone: null, phone_alt: null, address_en: null, address_ku: null,
  address_ar: null, tax_number: null, receipt_footer_en: null,
  receipt_footer_ku: null, receipt_footer_ar: null, currency: "IQD",
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
