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
