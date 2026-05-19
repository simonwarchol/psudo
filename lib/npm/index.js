/**
 * Default entry: async API with a pool of module workers for parallel NM multistarts.
 * Sync WASM on the main thread: import from "psudo/sync".
 */

const SIX_CH_MIN_RGB_RESCUE = 190;
const SIX_CH_L_TOT_RESCUE = -3.85;
const SIX_CH_RESCUE_RESTARTS = 4;
const RESCUE_SEED_SALT = 0x8e5ce500;
const DEFAULT_NUM_RESTARTS = 6;
const WASM_RESTART_MAX = 12;

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
  let best = pickBest(await runRestartWave(pool, commonArgs, nRestarts, 0, false));

  if (
    channels >= 6 &&
    (best.min_display_rgb_distance < SIX_CH_MIN_RGB_RESCUE ||
      best.total > SIX_CH_L_TOT_RESCUE)
  ) {
    const rescue = await runRestartWave(
      pool,
      commonArgs,
      SIX_CH_RESCUE_RESTARTS,
      RESCUE_SEED_SALT,
      true
    );
    const rescueBest = pickBest(rescue);
    if (rescueBest.total < best.total) best = rescueBest;
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

export function channel_gmm(array) {
  return callAny("channel_gmm", [array]);
}

export function ln(array) {
  return callAny("ln", [array]);
}
