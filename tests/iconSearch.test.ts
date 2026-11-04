// bun test tests/
import { describe, expect, test } from "bun:test";
import raw from "../src/data/icon-index.json";
import { keywordsFor, search, suggest, toEntries, tokenize } from "../src/lib/iconSearch";

const index = toEntries(raw as [string, string, string][]);
const top = (text: string, n = 3) => suggest(index, text).slice(0, n).map((i) => i.name);

describe("tokenize", () => {
  test("drops sizes and the Arabic article", () => {
    expect(tokenize("Coca-Cola 330ml x6")).toEqual(["coca", "cola"]);
    expect(tokenize("الحليب 1L")).toEqual(["حلیب"]);
  });
});

describe("suggest", () => {
  const cases: [string, string][] = [
    ["Fresh milk 1L", "milk"],
    ["شیری تازە", "milk"],
    ["حليب طازج", "milk"],
    ["نانی سەمون", "bread"],
    ["خبز صمون", "bread"],
    ["Coca-Cola 330ml", "bottle"],
    ["هێلکە ٣٠ دانە", "egg"],
    ["قهوة", "coffee"],
    ["ماسی", "fish"],
    ["Klicha date cookies", "cookie"],
    ["شوكولاتة داكنة", "chocolate"],
    ["پەمپەرز", "baby-carriage"],
    ["Ice cream", "ice-cream"],
  ];
  for (const [text, want] of cases) {
    test(`${text} → ${want}`, () => expect(top(text)).toContain(want));
  }

  test("no loose matches when there are solid ones", () => {
    expect(top("گۆشت و مریشک", 8)).not.toContain("dots");
  });

  test("nothing meaningful, no suggestions", () => {
    expect(suggest(index, "330ml 2")).toEqual([]);
  });

  test("keywords merge languages", () => {
    expect(keywordsFor("شیر و پەنیر")).toEqual(expect.arrayContaining(["milk", "cheese"]));
  });
});

describe("search", () => {
  test("English words match names and tags", () => {
    expect(search(index, "milk").map((i) => i.name)).toContain("milk");
  });
  test("Kurdish query searches by keywords", () => {
    expect(search(index, "نان").map((i) => i.name)).toContain("bread");
  });
});

import { guessCategory } from "../src/lib/barcodeLookup";

describe("guessCategory", () => {
  const cat = (id: number, name: string, parent_id: number | null = null) =>
    ({ id, name, parent_id, is_active: 1 }) as never;
  const cats = [cat(1, "خواردنەوە"), cat(2, "خواردنەوەی گازی", 1), cat(3, "شیرەمەنی"), cat(4, "Snacks")];

  test("Open Food Facts tags find Kurdish categories, most specific first", () => {
    expect(guessCategory(["en:beverages", "en:carbonated-drinks", "en:sodas"], cats)?.id).toBe(2);
    expect(guessCategory(["en:dairies", "en:milks"], cats)?.id).toBe(3);
  });
  test("no match, no guess", () => {
    expect(guessCategory(["en:pet-food"], cats)).toBeNull();
  });
});

import { code128Bars, code128Values } from "../src/lib/code128";

describe("code128", () => {
  test("adds start, checksum and stop", () => {
    // "R-000042": checksum worked by hand
    const v = code128Values("R-000042");
    expect(v[0]).toBe(104);
    expect(v.at(-1)).toBe(106);
    const data = v.slice(1, -2);
    expect(v.at(-2)).toBe(data.reduce((s, x, i) => s + x * (i + 1), 104) % 103);
  });
  test("each symbol is 11 modules, the stop 13", () => {
    const { width } = code128Bars("R-000042");
    expect(width).toBe(11 * (8 + 2) + 13);
  });
});
