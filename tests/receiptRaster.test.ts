// bun test tests/
import { describe, expect, mock, test } from "bun:test";

// receiptRaster imports the app's i18n and formatting; packBits needs none of it
mock.module("@/lib/format", () => ({ formatCurrency: String, formatUsd: String }));
mock.module("@/lib/locale", () => ({ formatDateTime: String }));
mock.module("@/utils/i18n", () => ({ i18n: { global: { t: String } } }));
const { packBits } = await import("../src/lib/receiptRaster");

const rgba = (rows: number[][]) =>
  new Uint8ClampedArray(rows.flat().flatMap((v) => (v < 0 ? [0, 0, 0, 0] : [v, v, v, 255])));

describe("packBits", () => {
  test("dark dots are 1s, leftmost in the high bit", () => {
    // 16 wide, 1 tall: black, white, black, then white
    const row = [0, 255, 0, ...Array(13).fill(255)];
    expect([...packBits(rgba([row]), 16, 1)]).toEqual([0b1010_0000, 0]);
  });
  test("transparent counts as paper", () => {
    expect([...packBits(rgba([Array(8).fill(-1)]), 8, 1)]).toEqual([0]);
  });
});
