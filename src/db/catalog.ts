import { select, selectOne } from "./client";

export interface Category {
  id: number;
  name: string;
  name_ku: string | null;
  name_ar: string | null;
  color: string | null;
}

export interface Product {
  id: number;
  sku: string;
  name: string;
  barcode: string | null;
  price: number;
  stock: number;
  minStock: number;
  categoryId: number | null;
  categoryName: string | null;
  categoryName_ku: string | null;
  categoryName_ar: string | null;
  categoryColor: string | null;
}

export type StockState = "ok" | "low" | "out";

export const stockState = (p: Pick<Product, "stock" | "minStock">): StockState =>
  p.stock <= 0 ? "out" : p.stock <= p.minStock ? "low" : "ok";

const PRODUCT_COLUMNS = `
  p.id, p.sku, p.name, p.barcode,
  p.selling_price AS price,
  p.stock_quantity AS stock,
  p.min_stock_level AS minStock,
  p.category_id AS categoryId,
  c.name AS categoryName, c.name_ku AS categoryName_ku, c.name_ar AS categoryName_ar,
  c.color AS categoryColor`;

const PRODUCT_FROM = `
  FROM products p
  LEFT JOIN categories c ON c.id = p.category_id
  WHERE p.is_active = 1`;

export const listCategories = async () =>
  select<Category>(
    `SELECT id, name, name_ku, name_ar, color FROM categories
     WHERE is_active = 1 ORDER BY display_order, name`,
  );

export interface ProductQuery {
  search?: string;
  categoryId?: number | null;
  limit?: number;
  offset?: number;
}

const productFilter = ({ search, categoryId }: ProductQuery) => {
  const where: string[] = [];
  const params: unknown[] = [];
  if (categoryId) {
    params.push(categoryId);
    where.push(`p.category_id = $${params.length}`);
  }
  const q = search?.trim();
  if (q) {
    params.push(`%${q}%`);
    const n = params.length;
    where.push(`(p.name LIKE $${n} OR p.sku LIKE $${n} OR p.barcode LIKE $${n})`);
  }
  return { sql: where.map((w) => ` AND ${w}`).join(""), params };
};

export const listProducts = async (query: ProductQuery = {}) => {
  const { sql, params } = productFilter(query);
  const page =
    query.limit != null
      ? ` LIMIT ${Math.trunc(query.limit)} OFFSET ${Math.trunc(query.offset ?? 0)}`
      : "";
  return select<Product>(
    `SELECT ${PRODUCT_COLUMNS} ${PRODUCT_FROM}${sql} ORDER BY c.display_order, p.name${page}`,
    params,
  );
};

export const countProducts = async (query: ProductQuery = {}) => {
  const { sql, params } = productFilter(query);
  return (
    (await selectOne<{ n: number }>(
      `SELECT COUNT(*) AS n ${PRODUCT_FROM}${sql}`,
      params,
    ))?.n ?? 0
  );
};

/** Exact match on primary barcode, any extra barcode, or SKU. */
export const findProductByCode = (code: string) =>
  selectOne<Product>(
    `SELECT ${PRODUCT_COLUMNS} ${PRODUCT_FROM}
       AND (p.barcode = $1 OR p.sku = $1
            OR p.id IN (SELECT product_id FROM product_barcodes WHERE barcode = $1))
     LIMIT 1`,
    [code],
  );
