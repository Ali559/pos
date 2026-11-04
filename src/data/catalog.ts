// Sample catalog used by the register and inventory screens until they read
// from SQLite (see ARCHITECTURE.md → Data layer). Shapes mirror `products`
// and `categories` in db.sql.

export type CategoryId = "beverages" | "snacks" | "dairy" | "bakery" | "produce";

export interface Product {
  id: number;
  sku: string;
  name: string;
  category: CategoryId;
  barcode: string;
  price: number;
  stock: number;
  minStock: number;
}

export const categories: CategoryId[] = [
  "beverages",
  "snacks",
  "dairy",
  "bakery",
  "produce",
];

export const products: Product[] = [
  { id: 1, sku: "BEV-001", name: "Coca-Cola 330ml", category: "beverages", barcode: "5449000000996", price: 0.75, stock: 142, minStock: 24 },
  { id: 2, sku: "BEV-002", name: "Mineral water 1.5L", category: "beverages", barcode: "6281006432108", price: 0.5, stock: 88, minStock: 24 },
  { id: 3, sku: "BEV-003", name: "Ayran 250ml", category: "beverages", barcode: "8690526012345", price: 0.6, stock: 9, minStock: 12 },
  { id: 4, sku: "BEV-004", name: "Black tea 500g", category: "beverages", barcode: "8690000123401", price: 4.25, stock: 31, minStock: 6 },
  { id: 5, sku: "SNK-001", name: "Potato chips salted", category: "snacks", barcode: "6281036111110", price: 1.0, stock: 57, minStock: 12 },
  { id: 6, sku: "SNK-002", name: "Roasted pistachios 200g", category: "snacks", barcode: "6260001234567", price: 3.5, stock: 4, minStock: 8 },
  { id: 7, sku: "SNK-003", name: "Dark chocolate bar", category: "snacks", barcode: "7622300489434", price: 1.75, stock: 40, minStock: 10 },
  { id: 8, sku: "DRY-001", name: "Fresh milk 1L", category: "dairy", barcode: "6281007012345", price: 1.25, stock: 26, minStock: 12 },
  { id: 9, sku: "DRY-002", name: "Yogurt 500g", category: "dairy", barcode: "6281007054321", price: 1.1, stock: 0, minStock: 10 },
  { id: 10, sku: "DRY-003", name: "White cheese 400g", category: "dairy", barcode: "6281007099887", price: 3.2, stock: 18, minStock: 6 },
  { id: 11, sku: "BAK-001", name: "Samoon bread (5)", category: "bakery", barcode: "2000000000011", price: 0.5, stock: 60, minStock: 20 },
  { id: 12, sku: "BAK-002", name: "Klicha date cookies", category: "bakery", barcode: "2000000000028", price: 2.75, stock: 14, minStock: 6 },
  { id: 13, sku: "PRD-001", name: "Tomatoes 1kg", category: "produce", barcode: "2000000000035", price: 0.9, stock: 35, minStock: 10 },
  { id: 14, sku: "PRD-002", name: "Cucumbers 1kg", category: "produce", barcode: "2000000000042", price: 0.8, stock: 7, minStock: 10 },
  { id: 15, sku: "PRD-003", name: "Pomegranates 1kg", category: "produce", barcode: "2000000000059", price: 2.0, stock: 22, minStock: 8 },
];

export type StockState = "ok" | "low" | "out";

export const stockState = (p: Product): StockState =>
  p.stock <= 0 ? "out" : p.stock <= p.minStock ? "low" : "ok";
