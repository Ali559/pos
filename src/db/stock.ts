import { select } from "./client";

export interface Batch {
  id: number;
  quantity_received: number;
  quantity_remaining: number;
  expiry_date: string | null;
  cost_price: number | null;
  received_at: string;
}

/** Batches with stock left, soonest expiry first (the order sales use them). */
export const listBatches = (productId: number) =>
  select<Batch>(
    `SELECT id, quantity_received, quantity_remaining, expiry_date, cost_price, received_at
     FROM product_batches
     WHERE product_id = $1 AND quantity_remaining > 0
     ORDER BY expiry_date IS NULL, expiry_date, received_at, id`,
    [productId],
  );
