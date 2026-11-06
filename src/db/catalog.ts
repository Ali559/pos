import { mediaColumns, takeMedia, type MediaColumns } from "./media";
import { select, selectOne } from "./client";
import type { MediaFile } from "@/lib/media";

export interface Category {
  id: number;
  name: string;
  description: string | null;
  parent_id: number | null;
  icon_key: string;
  /** Own color, or the parent's when a subcategory has none */
  color: string | null;
  own_color: string | null;
  is_active: number;
  display_order: number;
  product_count: number;
  image: MediaFile | null;
}

/** A pack size: a 6-pack, a carton. Price is for the whole pack. */
export interface ProductUnit {
  id: number;
  productId: number;
  name: string;
  /** How many of the product's own unit */
  factor: number;
  barcode: string | null;
  price: number;
}

/** Units that are counted, not weighed or measured */
export const WHOLE_UNITS = ["piece", "pack", "box"];
export const isWeighed = (unit: string) => !WHOLE_UNITS.includes(unit);

export interface Product {
  id: number;
  sku: string;
  name: string;
  barcode: string | null;
  price: number;
  /** Last cost (per product unit) */
  cost: number;
  stock: number;
  minStock: number;
  unit: string;
  iconKey: string | null;
  isActive: number;
  categoryId: number | null;
  /** Top-level category: the category itself, or its parent for a subcategory */
  rootCategoryId: number | null;
  categoryName: string | null;
  categoryColor: string | null;
  categoryIcon: string | null;
  /** The price is typed at the till (the misc item) */
  openPrice: number;
  trackStock: number;
  plu: string | null;
  /** Price at the wholesale level; null = same as retail */
  wholesalePrice: number | null;
  taxExempt: number;
  cover: MediaFile | null;
  /** Pack sizes, filled by listProducts / findProductByCode */
  units: ProductUnit[];
  /** Cheaper each from a quantity (retail) */
  qtyPrices: { minQty: number; price: number }[];
}

export type StockState = "ok" | "low" | "out";

export const stockState = (p: Pick<Product, "stock" | "minStock"> & { trackStock?: number }): StockState =>
  p.trackStock === 0 ? "ok" : p.stock <= 0 ? "out" : p.stock <= p.minStock ? "low" : "ok";

export const listCategories = async (includeArchived = false): Promise<Category[]> => {
  const rows = await select<Omit<Category, "image"> & MediaColumns>(
    `SELECT c.id, c.name, c.description, c.parent_id, c.icon_key,
            COALESCE(c.color, p.color) AS color, c.color AS own_color, c.is_active, c.display_order,
            (SELECT COUNT(*) FROM products pr WHERE pr.category_id = c.id) AS product_count,
            ${mediaColumns("m")}
     FROM categories c LEFT JOIN categories p ON p.id = c.parent_id
     LEFT JOIN media_files m ON m.id = c.image_media_id
     ${includeArchived ? "" : "WHERE c.is_active = 1 AND (p.id IS NULL OR p.is_active = 1)"}
     ORDER BY COALESCE(p.display_order, c.display_order), COALESCE(p.id, c.id),
              c.parent_id IS NOT NULL, c.display_order, c.name`,
  );
  return rows.map((row) => {
    const [image, rest] = takeMedia(row);
    return { ...rest, image };
  });
};

const PRODUCT_COLUMNS = `
  p.id, p.sku, p.name, p.barcode,
  p.selling_price AS price,
  p.cost_price AS cost,
  p.stock_quantity AS stock,
  p.min_stock_level AS minStock,
  p.unit, p.icon_key AS iconKey, p.is_active AS isActive,
  p.category_id AS categoryId,
  COALESCE(c.parent_id, c.id) AS rootCategoryId,
  c.name AS categoryName,
  COALESCE(c.color, pc.color) AS categoryColor,
  c.icon_key AS categoryIcon,
  p.open_price AS openPrice, p.track_stock AS trackStock, p.plu,
  p.wholesale_price AS wholesalePrice, p.tax_exempt AS taxExempt,
  m.id AS m_id, m.uuid AS m_uuid, m.path AS m_path, m.mime AS m_mime,
  m.width AS m_width, m.height AS m_height, m.bytes AS m_bytes, m.blurhash AS m_blurhash`;

const PRODUCT_FROM = `
  FROM products p
  LEFT JOIN categories c ON c.id = p.category_id
  LEFT JOIN categories pc ON pc.id = c.parent_id
  LEFT JOIN product_media pm ON pm.product_id = p.id AND pm.position = 0
  LEFT JOIN media_files m ON m.id = pm.media_id`;

type ProductRow = Omit<Product, "cover" | "units" | "qtyPrices"> & {
  m_id: number | null; m_uuid: string; m_path: string; m_mime: string;
  m_width: number; m_height: number; m_bytes: number; m_blurhash: string;
};

const toProduct = ({ m_id, m_uuid, m_path, m_mime, m_width, m_height, m_bytes, m_blurhash, ...p }: ProductRow): Product => ({
  ...p,
  units: [],
  qtyPrices: [],
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
      `(p.name LIKE $${n} OR p.sku LIKE $${n} OR p.barcode LIKE $${n})`,
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
  return withUnits(rows.map(toProduct));
};

const UNIT_COLUMNS = "id, product_id AS productId, name, factor, barcode, price";

/** Attaches pack sizes to a list of products */
const withUnits = async (products: Product[]) => {
  if (!products.length) return products;
  const [units, tiers] = await Promise.all([
    select<ProductUnit>(`SELECT ${UNIT_COLUMNS} FROM product_units ORDER BY product_id, position, id`),
    select<{ product_id: number; minQty: number; price: number }>("SELECT product_id, min_qty AS minQty, price FROM product_qty_prices ORDER BY product_id, min_qty"),
  ]);
  const byProduct = new Map<number, ProductUnit[]>();
  for (const u of units) byProduct.set(u.productId, [...(byProduct.get(u.productId) ?? []), u]);
  const tiersBy = new Map<number, { minQty: number; price: number }[]>();
  for (const { product_id, ...t } of tiers) tiersBy.set(product_id, [...(tiersBy.get(product_id) ?? []), t]);
  return products.map((p) => ({ ...p, units: byProduct.get(p.id) ?? [], qtyPrices: tiersBy.get(p.id) ?? [] }));
};

export const getProductById = async (id: number) => {
  const row = await selectOne<ProductRow>(`SELECT ${PRODUCT_COLUMNS} ${PRODUCT_FROM} WHERE p.id = $1`, [id]);
  return row ? (await withUnits([toProduct(row)]))[0]! : null;
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
  return row ? (await withUnits([toProduct(row)]))[0]! : null;
};

/** Everything the product editor needs. */
export interface ProductDetail {
  id: number;
  sku: string;
  name: string;
  category_id: number | null;
  barcode: string | null;
  cost_price: number;
  selling_price: number;
  min_stock_level: number;
  unit: string;
  icon_key: string | null;
  is_active: number;
  stock_quantity: number;
  plu: string | null;
  open_price: number;
  track_stock: number;
  wholesale_price: number | null;
  tax_exempt: number;
  extraBarcodes: string[];
  media: MediaFile[];
  units: ProductUnit[];
  qtyPrices: { minQty: number; price: number }[];
}

export interface PriceChange {
  id: number;
  field: "price" | "wholesale" | "cost";
  old_value: number | null;
  new_value: number | null;
  source: "edit" | "bulk" | "delivery";
  user: string | null;
  changed_at: string;
}

export const getPriceHistory = (productId: number) =>
  select<PriceChange>(
    `SELECT h.id, h.field, h.old_value, h.new_value, h.source, u.full_name AS user, h.changed_at
     FROM price_history h LEFT JOIN users u ON u.id = h.user_id
     WHERE h.product_id = $1 ORDER BY h.changed_at DESC, h.id DESC LIMIT 50`,
    [productId],
  );

export const getProduct = async (id: number): Promise<ProductDetail | null> => {
  const p = await selectOne<Omit<ProductDetail, "extraBarcodes" | "media" | "units" | "qtyPrices">>(
    `SELECT id, sku, name, category_id, barcode, cost_price, selling_price,
            min_stock_level, unit, icon_key, is_active, stock_quantity, plu, open_price, track_stock,
            wholesale_price, tax_exempt
     FROM products WHERE id = $1`,
    [id],
  );
  if (!p) return null;
  const [codes, media, units, qtyPrices] = await Promise.all([
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
    select<ProductUnit>(`SELECT ${UNIT_COLUMNS} FROM product_units WHERE product_id = $1 ORDER BY position, id`, [id]),
    select<{ minQty: number; price: number }>("SELECT min_qty AS minQty, price FROM product_qty_prices WHERE product_id = $1 ORDER BY min_qty", [id]),
  ]);
  return { ...p, extraBarcodes: codes.map((c) => c.barcode), media, units, qtyPrices };
};
