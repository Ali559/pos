// bun test tests/
import { describe, expect, test } from "bun:test";
import { asEan13, barcodeBars, ean13Check, ean13Modules } from "../src/lib/ean13";

describe("EAN-13", () => {
  test("check digit", () => {
    // 4006381333931 is the textbook example
    expect(ean13Check("400638133393")).toBe("1");
    expect(asEan13("400638133393")).toBe("4006381333931");
    expect(asEan13("4006381333931")).toBe("4006381333931");
    expect(asEan13("4006381333932")).toBeNull();
    expect(asEan13("P-00001")).toBeNull();
  });
  test("95 modules with guards in the right places", () => {
    const m = ean13Modules("4006381333931");
    expect(m.length).toBe(95);
    expect(m.slice(0, 3)).toBe("101");
    expect(m.slice(45, 50)).toBe("01010");
    expect(m.slice(92)).toBe("101");
    // First digit 4 → left parity LGLLGG: digit 0 in L is 0001101
    expect(m.slice(3, 10)).toBe("0001101");
  });
  test("non-EAN codes fall back to Code 128", () => {
    expect(barcodeBars("4006381333931").ean).toBe(true);
    expect(barcodeBars("P-00001").ean).toBe(false);
  });
});
