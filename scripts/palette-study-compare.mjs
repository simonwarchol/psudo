#!/usr/bin/env node
/**
 * 1:1 native vs WASM comparison on a shared fixture (same bytes in, same L_tot definition).
 *
 *   pnpm run palette-study-compare
 *
 * Steps:
 * 1. `cargo run --example palette_study_fixture` → lib/target/palette_study/fixture.json + native metrics
 * 2. WASM `optimize_with_metrics` on the same fixture inputs
 * 3. Print side-by-side; flag if |Δ L_tot| > tolerance
 */

import { execSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { readFile } from "node:fs/promises";
import { linearToDisplayRgb8 } from "./palette-study-lib.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const libDir = path.join(root, "lib");
const pkgDir = path.join(libDir, "pkg");
const fixturePath = path.join(libDir, "target/palette_study/fixture.json");

const L_TOT_TOL = 0.05;
const MIN_RGB_TOL = 2.0;

function ensureWasm() {
  if (!existsSync(path.join(pkgDir, "psudo_bg.wasm"))) {
    execSync("pnpm run wasm-build", { cwd: root, stdio: "inherit" });
  }
}

async function loadPsudo() {
  const wasmPath = path.join(pkgDir, "psudo_bg.wasm");
  const mod = await import(pathToFileURL(path.join(pkgDir, "psudo.js")).href);
  const wasmBytes = new Uint8Array(await readFile(wasmPath));
  await mod.default({ module_or_path: wasmBytes });
  return mod;
}

function fixtureToArrays(f) {
  return {
    colors: Uint16Array.from(f.colors),
    locked: Uint16Array.from(f.locked),
    intensities: Uint16Array.from(f.intensities),
    contrastLimits: Uint16Array.from(f.contrast_limits),
    luminance: Uint16Array.from(f.luminance),
    excluded: f.excluded ?? [],
    colorNames: f.color_names ?? [],
  };
}

async function main() {
  console.log("[compare] generating fixture (native)…");
  execSync("cargo run --example palette_study_fixture --release", {
    cwd: libDir,
    stdio: "inherit",
  });

  const fixture = JSON.parse(readFileSync(fixturePath, "utf8"));
  const a = fixtureToArrays(fixture);

  ensureWasm();
  const psudo = await loadPsudo();

  if (typeof psudo.optimize_with_metrics !== "function") {
    throw new Error(
      "optimize_with_metrics missing — run pnpm run wasm-build after pulling latest lib/src/lib.rs",
    );
  }

  console.log("[compare] running WASM optimize_with_metrics…");
  const t0 = performance.now();
  const wasm = await psudo.optimize_with_metrics(
    a.colors,
    a.locked,
    a.intensities,
    a.contrastLimits,
    a.luminance,
    a.excluded,
    a.colorNames,
    fixture.max_iters,
    fixture.confusion_samples,
    fixture.spatial,
    fixture.num_restarts,
  );
  const wasmMs = performance.now() - t0;

  const colorsU16 = new Uint16Array(fixture.channels * 3);
  for (let ch = 0; ch < fixture.channels; ch++) {
    const [r, g, b] = linearToDisplayRgb8(wasm.srgb_linear, ch);
    colorsU16[ch * 3] = r;
    colorsU16[ch * 3 + 1] = g;
    colorsU16[ch * 3 + 2] = b;
  }
  const legacyLoss = await psudo.calculate_palette_loss(
    a.intensities,
    colorsU16,
    a.contrastLimits,
    a.luminance,
    a.excluded,
    a.colorNames,
    fixture.spatial,
  );

  const legacyTotal =
    (legacyLoss.name_distance ?? 0) +
    (legacyLoss.perceptual_distance ?? legacyLoss.perceptural_distance ?? 0) +
    (legacyLoss.perceptual_deficit ?? 0) +
    (legacyLoss.term_loss ?? 0) +
    (legacyLoss.confusion ?? 0) +
    (legacyLoss.min_saturation_reward ?? 0) +
    (legacyLoss.saturation_deficit ?? 0);

  const dL = wasm.l_tot - fixture.native_l_tot;
  const dRgb = wasm.min_display_rgb_distance - fixture.native_min_rgb;

  console.log("\n=== palette_study 1:1 (fixture.json) ===\n");
  console.log(`  channels:     ${fixture.channels}`);
  console.log(`  max_iters:    ${fixture.max_iters}  restarts: ${fixture.num_restarts}`);
  console.log(`  WASM time:    ${(wasmMs / 1000).toFixed(2)}s\n`);
  console.log("  Metric (lower L_tot = better)     Native      WASM       Δ");
  console.log("  ─────────────────────────────────────────────────────────");
  console.log(
    `  L_tot (study metric)            ${fixture.native_l_tot.toFixed(4).padStart(8)}  ${wasm.l_tot.toFixed(4).padStart(8)}  ${dL >= 0 ? "+" : ""}${dL.toFixed(4)}`,
  );
  console.log(
    `  min_display_rgb_distance        ${fixture.native_min_rgb.toFixed(0).padStart(8)}  ${wasm.min_display_rgb_distance.toFixed(0).padStart(8)}  ${dRgb >= 0 ? "+" : ""}${dRgb.toFixed(0)}`,
  );
  console.log(
    `\n  L_tot via calculate_palette_loss (old WASM study path): ${legacyTotal.toFixed(4)}`,
  );
  console.log(
    "  ↑ This often looks worse; it evaluates rounded 8-bit RGB, not oklab_best.\n",
  );

  const ok =
    Math.abs(dL) <= L_TOT_TOL && Math.abs(dRgb) <= MIN_RGB_TOL;
  if (ok) {
    console.log(
      `  ✓ Within tolerance (|ΔL_tot| ≤ ${L_TOT_TOL}, |Δmin_rgb| ≤ ${MIN_RGB_TOL})`,
    );
  } else {
    console.log(
      `  ⚠ Outside tolerance — small gaps are normal (WASM sequential restarts vs native Rayon); large gaps may indicate a bug.`,
    );
    process.exitCode = 1;
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
