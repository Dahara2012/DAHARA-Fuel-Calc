import { readFileSync, writeFileSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));

// Read version from Cargo.toml
const cargo = readFileSync(join(root, "src-tauri", "Cargo.toml"), "utf-8");
const match = cargo.match(/^version\s*=\s*"([^"]+)"/m);
if (!match) {
  console.error("Could not find version in Cargo.toml");
  process.exit(1);
}
const version = match[1];

// Update package.json
const pkgPath = join(root, "package.json");
const pkg = JSON.parse(readFileSync(pkgPath, "utf-8"));
pkg.version = version;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

// Update tauri.conf.json
const tauriPath = join(root, "src-tauri", "tauri.conf.json");
const tauri = JSON.parse(readFileSync(tauriPath, "utf-8"));
tauri.version = version;
writeFileSync(tauriPath, JSON.stringify(tauri, null, 2) + "\n");

console.log(`synced version ${version} to package.json and tauri.conf.json`);
