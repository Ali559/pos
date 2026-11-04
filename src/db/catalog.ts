import { select, selectOne } from "./client";
import type { MediaFile } from "@/lib/media";

export interface Category {
  id: number;
  name: string;
  name_ku: string | null;
  name_ar: string | null;
  description: string | null;
  parent_id: number | null;
  icon_key: string;
  /** Own color, or the parent's when a subcategory has none */
  color: string | null;
  own_color: string | null;
  is_active: number;
  display_order: number;
  product_count: number;
}

export interface Product {
  id: number;
  sku: string;
  name: string;
  name_ku: string | null;
  name_ar: string | null;
  barcode: string | null;
  price: number;
  stock: number;
  minStock: number;
  unit: string;
  iconKey: string | null;
  isActive: number;
  categoryId: number | null;
  /** Top-level category: the category itself, or its parent for a subcategory */
  rootCategoryId: number | null;
  categoryName: string | null;
  categoryName_ku: string | null;
  categoryName_ar: string | null;
  categoryColor: string | null;
  categoryIcon: string | null;
  cover: MediaFile | null;
}

export type StockState = "ok" | "low" | "out";

export const stockState = (p: Pick<Product, "stock" | "minStock">): StockState =>
  p.stock <= 0 ? "out" : p.stock <= p.minStock ? "low" : "ok";

export const listCategories = (includeArchived = false) =>
  select<Category>(
    `SELECT c.id, c.name, c.name_ku, c.name_ar, c.description, c.parent_id, c.icon_key,
            COALESCE(c.color, p.color) AS color, c.color AS own_color, c.is_active, c.display_order,
            (SELECT COUNT(*) FROM products pr WHERE pr.category_id = c.id) AS product_count
     FROM categories c LEFT JOIN categories p ON p.id = c.parent_id
     ${includeArchived ? "" : "WHERE c.is_active = 1 AND (p.id IS NULL OR p.is_active = 1)"}
     ORDER BY COALESCE(p.display_order, c.display_order), COALESCE(p.id, c.id),
              c.parent_id IS NOT NULL, c.display_order, c.name`,
  );

const PRODUCT_COLUMNS = `
  p.id, p.sku, p.name, p.name_ku, p.name_ar, p.barcode,
  p.selling_price AS price,
  p.stock_quantity AS stock,
  p.min_stock_level AS minStock,
  p.unit, p.icon_key AS iconKey, p.is_active AS isActive,
  p.category_id AS categoryId,
  COALESCE(c.parent_id, c.id) AS rootCategoryId,
  c.name AS categoryName, c.name_ku AS categoryName_ku, c.name_ar AS categoryName_ar,
  COALESCE(c.color, pc.color) AS categoryColor,
  c.icon_key AS categoryIcon,
  m.id AS m_id, m.uuid AS m_uuid, m.path AS m_path, m.mime AS m_mime,
  m.width AS m_width, m.height AS m_height, m.bytes AS m_bytes, m.blurhash AS m_blurhash`;

const PRODUCT_FROM = `
  FROM products p
  LEFT JOIN categories c ON c.id = p.category_id
  LEFT JOIN categories pc ON pc.id = c.parent_id
  LEFT JOIN product_media pm ON pm.product_id = p.id AND pm.position = 0
  LEFT JOIN media_files m ON m.id = pm.media_id`;

type ProductRow = Omit<Product, "cover"> & {
  m_id: number | null; m_uuid: string; m_path: string; m_mime: string;
  m_width: number; m_height: number; m_bytes: number; m_blurhash: string;
};

const toProduct = ({ m_id, m_uuid, m_path, m_mime, m_width, m_height, m_bytes, m_blurhash, ...p }: ProductRow): Product => ({
  ...p,
  cover: m_id
    ? { id: m_id, uuid: m_uuid, path: m_path, mime: m_mime, width: m_width, height: m_height, bytes: m_bytes, blurhash: m_blurhash }
    : null,
});

export interface ProductQuery {
  search?: string;
  /** A top-level category also matches products in its subcategories */
  categoryId?: number | null;
  includeInactive?: boolean;
  limit?: number;
  offset?: number;
}

const productFilter = ({ search, categoryId, includeInactive }: ProductQuery) => {
  const where: string[] = includeInactive ? [] : ["p.is_active = 1"];
  const params: unknown[] = [];
  if (categoryId) {
    params.push(categoryId);
    where.push(`(p.category_id = $${params.length} OR c.parent_id = $${params.length})`);
  }
  const q = search?.trim();
  if (q) {
    params.push(`%${q}%`);
    const n = params.length;
    where.push(
      `(p.name LIKE $${n} OR p.name_ku LIKE $${n} OR p.name_ar LIKE $${n} OR p.sku LIKE $${n} OR p.barcode LIKE $${n})`,
    );
  }
  return { sql: where.length ? ` WHERE ${where.join(" AND ")}` : "", params };
};

export const listProducts = async (query: ProductQuery = {}) => {
  const { sql, params } = productFilter(query);
  const page =
    query.limit != null
      ? ` LIMIT ${Math.trunc(query.limit)} OFFSET ${Math.trunc(query.offset ?? 0)}`
      : "";
  const rows = await select<ProductRow>(
    `SELECT ${PRODUCT_COLUMNS} ${PRODUCT_FROM}${sql}
     ORDER BY COALESCE(pc.display_order, c.display_order), c.display_order, p.name${page}`,
    params,
  );
  return rows.map(toProduct);
};

export const countProducts = async (query: ProductQuery = {}) => {
  const { sql, params } = productFilter(query);
  return (
    (await selectOne<{ n: number }>(
      `SELECT COUNT(*) AS n FROM products p LEFT JOIN categories c ON c.id = p.category_id${sql}`,
      params,
    ))?.n ?? 0
  );
};

/** Exact match on primary barcode, any extra barcode, or SKU. Active products only. */
export const findProductByCode = async (code: string) => {
  const row = await selectOne<ProductRow>(
    `SELECT ${PRODUCT_COLUMNS} ${PRODUCT_FROM}
     WHERE p.is_active = 1
       AND (p.barcode = $1 OR p.sku = $1
            OR p.id IN (SELECT product_id FROM product_barcodes WHERE barcode = $1))
     LIMIT 1`,
    [code],
  );
  return row ? toProduct(row) : null;
};

/** Everything the product editor needs. */
export interface ProductDetail {
  id: number;
  sku: string;
  name: string;
  name_ku: string | null;
  name_ar: string | null;
  category_id: number | null;
  barcode: string | null;
  cost_price: number;
  selling_price: number;
  min_stock_level: number;
  unit: string;
  icon_key: string | null;
  is_active: number;
  stock_quantity: number;
  extraBarcodes: string[];
  media: MediaFile[];
}

export const getProduct = async (id: number): Promise<ProductDetail | null> => {
  const p = await selectOne<Omit<ProductDetail, "extraBarcodes" | "media">>(
    `SELECT id, sku, name, name_ku, name_ar, category_id, barcode, cost_price, selling_price,
            min_stock_level, unit, icon_key, is_active, stock_quantity
     FROM products WHERE id = $1`,
    [id],
  );
  if (!p) return null;
  const [codes, media] = await Promise.all([
    select<{ barcode: string }>(
      "SELECT barcode FROM product_barcodes WHERE product_id = $1 ORDER BY id",
      [id],
    ),
    select<MediaFile>(
      `SELECT m.id, m.uuid, m.path, m.mime, m.width, m.height, m.bytes, m.blurhash
       FROM product_media pm JOIN media_files m ON m.id = pm.media_id
       WHERE pm.product_id = $1 ORDER BY pm.position`,
      [id],
    ),
  ]);
  return { ...p, extraBarcodes: codes.map((c) => c.barcode), media };
};
