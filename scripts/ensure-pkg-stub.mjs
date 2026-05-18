import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const pkgDir = path.join(root, "lib/pkg");
const npmDir = path.join(root, "lib/npm");
const marker = path.join(pkgDir, "package.json");

if (existsSync(marker)) {
  process.exit(0);
}

mkdirSync(pkgDir, { recursive: true });
for (const name of [
  "package.json",
  "index.js",
  "index.d.ts",
  "sync.js",
  "psudo.worker.js",
]) {
  cpSync(path.join(npmDir, name), path.join(pkgDir, name));
}

const cargoToml = readFileSync(path.join(root, "lib/Cargo.toml"), "utf8");
const version = cargoToml.match(/^version = "([^"]+)"/m)?.[1];
if (version) {
  const pkg = JSON.parse(readFileSync(marker, "utf8"));
  pkg.version = version;
  writeFileSync(marker, `${JSON.stringify(pkg, null, 2)}\n`);
}

console.log("[psudo] created lib/pkg stubs from lib/npm (run wasm-build for WASM)");
