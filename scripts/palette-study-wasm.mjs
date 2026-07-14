#!/usr/bin/env node
/**
 * Browser-identical palette_study using the published WASM package (psudo/sync).
 *
 *   pnpm run palette-study-wasm
 *   PALETTE_STUDY_CHANNELS=4,6 PALETTE_STUDY_PARENTS=5 pnpm run palette-study-wasm
 *   pnpm run palette-study-wasm -- --document path/to/story.json
 *
 * Env (same names as native `palette_study` example):
 *   PALETTE_STUDY_PARENTS, PALETTE_STUDY_CHANNELS, PALETTE_STUDY_ROWS,
 *   PALETTE_STUDY_MAX_ITERS, PALETTE_STUDY_CONFUSION_SAMPLES, PALETTE_STUDY_RESTARTS,
 *   PALETTE_STUDY_SPATIAL=1
 */

import { execSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import {
  buildStudyInputs,
  channelGroupsFromDocument,
  colorsU16FromRgb,
  DEFAULT_PARENTS,
  DEFAULT_RESTARTS,
  DEFAULT_ROWS,
  formatDuration,
  htmlReportHeader,
  linearToDisplayRgb8,
  parseChannelCounts,
  parseEnvBool,
  parseEnvInt,
  scaledBudget,
  spreadInitialColorsU16,
  svgSwatches,
} from "./palette-study-lib.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const pkgDir = path.join(root, "lib/pkg");
const outDir = path.join(root, "lib/target/palette_study_wasm");

function ensureWasm() {
  const wasm = path.join(pkgDir, "psudo_bg.wasm");
  if (!existsSync(wasm)) {
    console.log("[palette-study-wasm] building lib/pkg…");
    execSync("pnpm run wasm-build", { cwd: root, stdio: "inherit" });
  }
}

async function loadPsudo() {
  const wasmPath = path.join(pkgDir, "psudo_bg.wasm");
  const jsUrl = pathToFileURL(path.join(pkgDir, "psudo.js")).href;
  const mod = await import(jsUrl);
  // Node fetch cannot load bare filesystem paths; pass bytes directly.
  const wasmBytes = new Uint8Array(await readFile(wasmPath));
  await mod.default({ module_or_path: wasmBytes });
  return mod;
}

function linearToColorsU16(linear) {
  const n = linear.length / 3;
  const out = new Uint16Array(linear.length);
  for (let ch = 0; ch < n; ch++) {
    const [r, g, b] = linearToDisplayRgb8(linear, ch);
    out[ch * 3] = r;
    out[ch * 3 + 1] = g;
    out[ch * 3 + 2] = b;
  }
  return out;
}

async function runOneOptimize(psudo, inputs) {
  const t0 = performance.now();
  const run = await psudo.optimize_with_metrics(
    inputs.colors,
    inputs.locked,
    inputs.intensities,
    inputs.contrastLimits,
    inputs.luminance,
    inputs.excluded,
    inputs.colorNames,
    inputs.maxIters,
    inputs.confusionSamples,
    inputs.spatial,
    inputs.numRestarts,
  );
  const elapsedMs = performance.now() - t0;
  const linear = run.srgb_linear;
  const total = run.l_tot;
  const minRgb = run.min_display_rgb_distance;
  const rgb8 = [];
  for (let ch = 0; ch < linear.length / 3; ch++) {
    rgb8.push(linearToDisplayRgb8(linear, ch));
  }
  return { linear, total, minRgb, elapsedMs, rgb8 };
}

async function runSyntheticBatch(psudo, channels, nParents, nRows, config) {
  const scaledRestarts = scaledBudget(config.numRestarts, channels);
  const nmIters = Math.floor(scaledBudget(config.maxIters, channels) / 2);
  console.error(
    `[palette-study-wasm] ${channels}ch: NM ~${nmIters} iters/restart, ${scaledRestarts} restarts, ${nParents} palettes`,
  );

  const shared = buildStudyInputs(channels, new Uint16Array(channels * 3), {
    nRows,
    intensitySeed: 9000 + channels,
  });
  const seedBase = 50_000 + channels * 10_000;
  const runs = [];

  for (let i = 0; i < nParents; i++) {
    const colors = spreadInitialColorsU16(channels, seedBase + i);
    const inputs = {
      ...shared,
      colors,
      locked: new Uint16Array(channels),
    };
    const run = await runOneOptimize(psudo, inputs);
    console.error(
      `[palette-study-wasm] ${channels}ch #${i + 1}/${nParents} L_tot=${run.total.toFixed(4)} min_rgb=${run.minRgb.toFixed(0)} time=${formatDuration(run.elapsedMs)}`,
    );
    runs.push({ index: i, ...run });
  }

  runs.sort((a, b) => a.total - b.total);
  return { channels, runs, scaledRestarts, nmIters };
}

async function runDocumentGroups(psudo, groups, nRows, config) {
  const batches = [];
  for (const group of groups) {
    const n = group.channels.length;
    const colors = colorsU16FromRgb(group.channels);
    const inputs = buildStudyInputs(n, colors, { nRows });
    inputs.maxIters = config.maxIters;
    inputs.confusionSamples = config.confusionSamples;
    inputs.spatial = config.spatial;
    inputs.numRestarts = config.numRestarts;

    console.error(`[palette-study-wasm] document group "${group.name}" (${n} ch)`);
    const run = await runOneOptimize(psudo, inputs);
    batches.push({
      channels: n,
      groupName: group.name,
      groupId: group.id,
      runs: [{ index: 0, label: "document", ...run }],
    });
  }
  return batches;
}

function appendSectionHtml(html, batch, label) {
  const title =
    batch.groupName != null
      ? `${batch.groupName} (${batch.channels} ch)`
      : `${batch.channels}-channel palettes`;
  html.push(`<section class="section"><h2>${title}</h2>`);
  html.push(
    `<p class="note">Sorted by L_tot (lower is better). NM budget: ~${batch.nmIters ?? "—"} iters/restart, ${batch.scaledRestarts ?? "—"} restarts.</p>`,
  );
  for (const run of batch.runs) {
    const tag = run.label ?? `#${run.index + 1}`;
    html.push(`<div class="card">`);
    html.push(svgSwatches(run.rgb8));
    html.push(`<div><div class="meta"><strong>${tag}</strong></div>`);
    html.push(
      `<div>L_tot=<strong>${run.total.toFixed(4)}</strong> · min_rgb=<strong>${run.minRgb.toFixed(0)}</strong> · <span class="time">${formatDuration(run.elapsedMs)}</span></div>`,
    );
    html.push(`</div></div>`);
  }
  html.push(`</section>`);
}

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      document: { type: "string", short: "d" },
      help: { type: "boolean", short: "h" },
    },
    allowPositionals: true,
  });

  if (values.help) {
    console.log(`Usage:
  pnpm run palette-study-wasm
  pnpm run palette-study-wasm -- --document path/to/story.json

Uses psudo/sync (main-thread WASM), same defaults as cargo palette_study.
Document JSON: channelGroups[].channels[].color { r,g,b } (Minerva / validateDocumentData shape).`);
    return;
  }

  ensureWasm();
  const psudo = await loadPsudo();

  const config = {
    maxIters: parseEnvInt("PALETTE_STUDY_MAX_ITERS", 3000),
    confusionSamples: parseEnvInt("PALETTE_STUDY_CONFUSION_SAMPLES", 32),
    numRestarts: parseEnvInt("PALETTE_STUDY_RESTARTS", DEFAULT_RESTARTS),
    spatial: parseEnvBool("PALETTE_STUDY_SPATIAL"),
  };
  const nRows = parseEnvInt("PALETTE_STUDY_ROWS", DEFAULT_ROWS);

  const docPath = values.document ?? positionals[0];
  const studyStart = performance.now();
  let batches = [];

  if (docPath) {
    const abs = path.isAbsolute(docPath)
      ? docPath
      : path.join(process.cwd(), docPath);
    const raw = JSON.parse(readFileSync(abs, "utf8"));
    const groups = channelGroupsFromDocument(raw);
    if (groups.length === 0) {
      throw new Error(
        `No channel groups with ≥2 colors in ${abs}. Expected channelGroups[].channels[].color.`,
      );
    }
    console.error(
      `[palette-study-wasm] document mode: ${groups.length} group(s) from ${abs}`,
    );
    batches = await runDocumentGroups(psudo, groups, nRows, config);
  } else {
    const channelCounts = parseChannelCounts();
    const nParents = parseEnvInt("PALETTE_STUDY_PARENTS", DEFAULT_PARENTS);
    for (const channels of channelCounts) {
      batches.push(
        await runSyntheticBatch(psudo, channels, nParents, nRows, config),
      );
    }
  }

  const studyMs = performance.now() - studyStart;
  mkdirSync(outDir, { recursive: true });

  const meta = `Nelder–Mead WASM · max_iters=${config.maxIters} restarts=${config.numRestarts} confusion=${config.confusionSamples} spatial=${config.spatial ? "on" : "off"} · rows=${nRows} · total <span class="time">${formatDuration(studyMs)}</span>`;
  const html = [htmlReportHeader(meta)];
  for (const batch of batches) {
    appendSectionHtml(html, batch);
  }
  html.push("</body></html>");

  const reportPath = path.join(outDir, "report.html");
  writeFileSync(reportPath, html.join("\n"));
  console.error(`[palette-study-wasm] wrote ${reportPath}`);
  console.error(
    `[palette-study-wasm] open file://${path.resolve(reportPath)}`,
  );
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
