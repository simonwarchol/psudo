/**
 * Default entry: async API with a pool of module workers for parallel NM multistarts.
 * Sync WASM on the main thread: import from "psudo/sync".
 */

const MIN_DISPLAY_RGB_DISTANCE = 90;
const HIGH_CH_RESCUE_RESTARTS_BASE = 5;
const RESCUE_SEED_SALT_1 = 0x8e5ce500;
const RESCUE_SEED_SALT_2 = 0xa11ce5c0;
const DEFAULT_NUM_RESTARTS = 18;
const WASM_RESTART_MAX = 32;

/** Adaptive rescue gates from a multistart wave (mirrors Rust `adaptive_rescue_band`). */
function adaptiveRescueBand(outcomes) {
  let bestL = Infinity;
  let bestMinRgb = 0;
  let sum = 0;
  let sumSq = 0;
  const n = Math.max(1, outcomes.length);
  for (const o of outcomes) {
    sum += o.total;
    sumSq += o.total * o.total;
    if (o.total < bestL) {
      bestL = o.total;
      bestMinRgb = o.min_display_rgb_distance;
    }
  }
  const soft = MIN_DISPLAY_RGB_DISTANCE * 0.9;
  if (!Number.isFinite(bestL)) return { lTotRescue: 0, rgbRescue: soft };
  const mean = sum / n;
  const varr = Math.max(0, sumSq / n - mean * mean);
  const std = Math.sqrt(varr);
  const margin = Math.min(0.35, Math.max(0.12, 0.5 * std));
  return { lTotRescue: bestL + margin, rgbRescue: Math.min(bestMinRgb, soft) };
}

function outsideAdaptiveBand(total, minRgb, lTotRescue, rgbRescue) {
  return minRgb < rgbRescue || total > lTotRescue;
}

let parallelMultistart = true;
let workerPool = null;

function supportsWorker() {
  return typeof Worker !== "undefined" && typeof window !== "undefined";
}

function defaultPoolSize() {
  if (typeof navigator !== "undefined" && navigator.hardwareConcurrency) {
    return Math.min(4, Math.max(1, navigator.hardwareConcurrency));
  }
  return 2;
}

function scaledBudget(base, channels) {
  return Math.max(1, Math.floor((Number(base) * channels) / 3));
}

function effectiveNumRestarts(num_restarts, channels) {
  const base = num_restarts ?? DEFAULT_NUM_RESTARTS;
  const scaled = scaledBudget(base, channels);
  return Math.min(WASM_RESTART_MAX, Math.max(1, scaled));
}

class WorkerClient {
  constructor(worker) {
    this.worker = worker;
    this.pending = new Map();
    this.nextId = 1;
    worker.onmessage = (event) => {
      const { id, ok, result, error } = event.data;
      const entry = this.pending.get(id);
      if (!entry) return;
      this.pending.delete(id);
      if (ok) entry.resolve(result);
      else entry.reject(new Error(error || "psudo worker error"));
    };
    worker.onerror = (event) => {
      for (const [, entry] of this.pending) {
        entry.reject(new Error(event.message || "psudo worker failed"));
      }
      this.pending.clear();
    };
  }

  call(method, args) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ id, method, args });
    });
  }
}

function getWorkerPool() {
  if (!supportsWorker()) {
    throw new Error(
      "psudo Web Worker is not available in this environment. Use `import * as psudo from \"psudo/sync\"`."
    );
  }
  if (!workerPool) {
    const n = defaultPoolSize();
    workerPool = Array.from({ length: n }, () => {
      const w = new Worker(new URL("./psudo.worker.js", import.meta.url), {
        type: "module",
      });
      return new WorkerClient(w);
    });
  }
  return workerPool;
}

function callAny(method, args) {
  const pool = getWorkerPool();
  return pool[0].call(method, args);
}

function restartArgs(
  colors,
  locked_colors,
  intensities,
  contrast_limits,
  luminance_values,
  excluded_colors,
  color_names,
  max_iters,
  confusion_baseline_samples,
  include_spatial_channel_overlap,
  restartIndex,
  seedSalt,
  rescueRandomInit
) {
  return [
    colors,
    locked_colors,
    intensities,
    contrast_limits,
    luminance_values,
    excluded_colors,
    color_names,
    max_iters ?? undefined,
    confusion_baseline_samples ?? undefined,
    include_spatial_channel_overlap ?? undefined,
    restartIndex,
    seedSalt,
    rescueRandomInit,
  ];
}

function pickBest(outcomes) {
  let best = outcomes[0];
  for (let i = 1; i < outcomes.length; i++) {
    if (outcomes[i].total < best.total) best = outcomes[i];
  }
  return best;
}

async function runRestartWave(pool, commonArgs, count, seedSalt, rescueRandomInit) {
  let next = 0;
  const results = new Array(count);

  async function runOnClient(client) {
    while (true) {
      const i = next;
      next += 1;
      if (i >= count) break;
      const args = restartArgs(
        ...commonArgs,
        i,
        seedSalt,
        rescueRandomInit
      );
      results[i] = await client.call("nmRestart", args);
    }
  }

  await Promise.all(pool.map((client) => runOnClient(client)));
  return results;
}

async function optimizeParallel(
  colors,
  locked_colors,
  intensities,
  contrast_limits,
  luminance_values,
  excluded_colors,
  color_names,
  max_iters,
  confusion_baseline_samples,
  include_spatial_channel_overlap,
  num_restarts
) {
  const channels = colors.length / 3;
  const nRestarts = effectiveNumRestarts(num_restarts, channels);
  const commonArgs = [
    colors,
    locked_colors,
    intensities,
    contrast_limits,
    luminance_values,
    excluded_colors,
    color_names,
    max_iters,
    confusion_baseline_samples,
    include_spatial_channel_overlap,
  ];

  await Promise.all(getWorkerPool().map((c) => c.call("warmup", [])));

  const pool = getWorkerPool();
  const primary = await runRestartWave(pool, commonArgs, nRestarts, 0, false);
  let best = pickBest(primary);

  if (channels >= 6) {
    const { lTotRescue, rgbRescue } = adaptiveRescueBand(primary);
    const softRgb = MIN_DISPLAY_RGB_DISTANCE * 0.9;
    const rescueRestarts = Math.max(
      4,
      scaledBudget(HIGH_CH_RESCUE_RESTARTS_BASE, channels)
    );
    let preferRandom = best.min_display_rgb_distance < softRgb;
    const salts = [RESCUE_SEED_SALT_1, RESCUE_SEED_SALT_2];
    for (let wave = 0; wave < 2; wave++) {
      if (
        !outsideAdaptiveBand(
          best.total,
          best.min_display_rgb_distance,
          lTotRescue,
          rgbRescue
        )
      ) {
        break;
      }
      const rescue = await runRestartWave(
        pool,
        commonArgs,
        rescueRestarts,
        salts[wave],
        preferRandom
      );
      preferRandom = !preferRandom;
      const rescueBest = pickBest(rescue);
      if (rescueBest.total < best.total) best = rescueBest;
    }
  }

  return pool[0].call("finalizePalette", [...commonArgs, best.oklab]);
}

/** Enable/disable parallel NM multistarts across workers (default on). */
export function setParallelMultistart(enabled) {
  parallelMultistart = Boolean(enabled);
}

/** Preload WASM in all pool workers (optional, e.g. on app mount). */
export function warmup() {
  return Promise.all(getWorkerPool().map((c) => c.call("warmup", [])));
}

export function optimize(
  colors,
  locked_colors,
  intensities,
  contrast_limits,
  luminance_values,
  excluded_colors,
  color_names,
  max_iters,
  confusion_baseline_samples,
  include_spatial_channel_overlap,
  num_restarts
) {
  if (parallelMultistart && supportsWorker()) {
    return optimizeParallel(
      colors,
      locked_colors,
      intensities,
      contrast_limits,
      luminance_values,
      excluded_colors,
      color_names,
      max_iters,
      confusion_baseline_samples,
      include_spatial_channel_overlap,
      num_restarts
    ).catch((err) => {
      console.warn(
        "[psudo] parallel optimize failed, falling back to single worker:",
        err
      );
      return callAny("optimize", [
        colors,
        locked_colors,
        intensities,
        contrast_limits,
        luminance_values,
        excluded_colors,
        color_names,
        max_iters,
        confusion_baseline_samples,
        include_spatial_channel_overlap,
        num_restarts,
      ]);
    });
  }
  return callAny("optimize", [
    colors,
    locked_colors,
    intensities,
    contrast_limits,
    luminance_values,
    excluded_colors,
    color_names,
    max_iters,
    confusion_baseline_samples,
    include_spatial_channel_overlap,
    num_restarts,
  ]);
}

export function calculate_palette_loss(
  intensities,
  colors,
  contrast_limits,
  luminance_values,
  excluded_colors,
  color_names,
  include_spatial_channel_overlap
) {
  return callAny("calculate_palette_loss", [
    intensities,
    colors,
    contrast_limits,
    luminance_values,
    excluded_colors,
    color_names,
    include_spatial_channel_overlap,
  ]);
}

export function optimize_in_lens(
  intensities,
  colors,
  contrast_limits,
  luminance_values
) {
  return callAny("optimize_in_lens", [
    intensities,
    colors,
    contrast_limits,
    luminance_values,
  ]);
}

export function channel_gmm(array, subsample, tol, max_iter) {
  return callAny("channel_gmm", [array, subsample, tol, max_iter]);
}

export function ln(array) {
  return callAny("ln", [array]);
}
