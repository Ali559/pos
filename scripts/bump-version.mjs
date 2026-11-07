// Sets the app's version everywhere it's written down:
//   bun scripts/bump-version.mjs 0.2.0
// package.json, src-tauri/tauri.conf.json (what the updater compares) and
// src-tauri/Cargo.toml. See RELEASING.md.
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error("Usage: bun scripts/bump-version.mjs <major.minor.patch>");
  process.exit(1);
}

const json = (path, edit) => {
  const data = JSON.parse(readFileSync(path, "utf8"));
  edit(data);
  writeFileSync(path, JSON.stringify(data, null, 2) + "\n");
};
json("package.json", (d) => (d.version = version));
json("src-tauri/tauri.conf.json", (d) => (d.version = version));

const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
writeFileSync("src-tauri/Cargo.toml", cargo.replace(/^version = ".*"$/m, `version = "${version}"`));

console.log(`Version set to ${version}. Next: commit, then git tag v${version} && git push origin main v${version}`);
