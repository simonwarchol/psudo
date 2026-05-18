import { existsSync } from "node:fs";
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const wasm = path.join(root, "lib/pkg/psudo_bg.wasm");

if (!existsSync(wasm)) {
  console.log("[psudo] lib/pkg/psudo_bg.wasm missing — running wasm-build…");
  execSync("npm run wasm-build", { cwd: root, stdio: "inherit" });
}
