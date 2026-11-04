// Builds src/data/icon-index.json from @tabler/icons metadata: the names,
// categories and tags the icon picker searches and the suggester scores.
// Drawing data comes from @iconify-json/tabler at runtime; this file only
// holds the words. Run after upgrading either package:
//   bun scripts/build-icon-index.mjs
import { readFileSync, writeFileSync } from "node:fs";

const meta = JSON.parse(
  readFileSync(new URL("../node_modules/@tabler/icons/icons.json", import.meta.url)),
);
const drawable = JSON.parse(
  readFileSync(new URL("../node_modules/@iconify-json/tabler/icons.json", import.meta.url)),
).icons;

// Brand logos and "-off" (crossed-out) variants are noise for a shop catalog
const skip = (name) => name.startsWith("brand-") || name.endsWith("-off");

const index = Object.values(meta)
  .filter((i) => i.styles?.outline && drawable[i.name] && !skip(i.name))
  .map((i) => [
    i.name,
    i.category ?? "",
    [...new Set((i.tags ?? []).map((t) => String(t).toLowerCase()))].join(" "),
  ])
  .sort((a, b) => a[0].localeCompare(b[0]));

writeFileSync(
  new URL("../src/data/icon-index.json", import.meta.url),
  JSON.stringify(index),
);
console.log(`icon-index.json: ${index.length} icons`);
