import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { glslify } from "vite-plugin-glslify";
import svgr from "vite-plugin-svgr";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

const root = path.dirname(fileURLToPath(import.meta.url));
const psudoEntry = path.resolve(root, "lib/pkg/index.js");

export default defineConfig({
  plugins: [react(), glslify(), svgr(), wasm(), topLevelAwait()],
  resolve: {
    alias: {
      psudo: psudoEntry,
    },
  },
  worker: {
    plugins: [wasm()],
    format: "es",
  },
  optimizeDeps: {
    exclude: ["psudo"],
  },
  define: {
    "process.env": {},
  },
});
