/**
 * Default entry: async API with a pool of module workers for parallel NM multistarts.
 * Sync WASM on the main thread: import from "psudo/sync".
 */

const MIN_DISPLAY_RGB_DISTANCE = 90;
const HIGH_CH_RESCUE_RESTARTS_BASE = 5;
const RESCUE_SEED_SALT_1 = 0x8e5ce500;
const RESCUE_SEED_SALT_2 = 0xa11ce5c0;
const DEFAULT_NUM_RESTARTS = 18;
/** Same cap as native `restart_count_bounds` so 6ch→36 / 8ch→40 match palette_study. */
const WASM_RESTART_MAX = 40;

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
let configuredWorkerPoolSize = null;

function supportsWorker() {
  return typeof Worker !== "undefined" && typeof window !== "undefined";
}

function abortError() {
  if (typeof DOMException !== "undefined") {
    try {
      return new DOMException("Aborted", "AbortError");
    } catch {
      /* Node without DOMException name support */
    }
  }
  const err = new Error("Aborted");
  err.name = "AbortError";
  return err;
}

function defaultPoolSize() {
  if (configuredWorkerPoolSize !== null) {
    return configuredWorkerPoolSize;
  }
  if (typeof navigator !== "undefined" && navigator.hardwareConcurrency) {
    const concurrency = Math.max(1, navigator.hardwareConcurrency);
    return Math.min(
      6,
      concurrency,
      Math.max(4, Math.floor(concurrency * 0.6))
    );
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

/** Locked channels are pinned, not searched, so budgets follow the free count (as in Rust). */
function countFreeChannels(locked_colors, channels) {
  let free = 0;
  for (let i = 0; i < channels; i++) {
    if (!locked_colors?.[i]) free += 1;
  }
  return free;
}

function spawnWorkerClient() {
  const w = new Worker(new URL("./psudo.worker.js", import.meta.url), {
    type: "module",
  });
  return new WorkerClient(w);
}

/** Terminate pool members busy with this scope and put fresh clients in those slots. */
function replaceBusyClients(tracked, error) {
  const pool = workerPool;
  if (!pool) return;
  for (let i = 0; i < pool.length; i++) {
    const client = pool[i];
    if (!tracked.has(client) || client.pending.size === 0) continue;
    client.worker.onmessage = null;
    client.worker.onerror = null;
    client.rejectAll(error);
    client.worker.terminate();
    pool[i] = spawnWorkerClient();
  }
}

function createAbortScope(signal) {
  const tracked = new Set();
  let aborted = false;

  const onAbort = () => {
    if (aborted) return;
    aborted = true;
    replaceBusyClients(tracked, abortError());
  };

  if (signal) {
    signal.addEventListener("abort", onAbort);
  }

  return {
    get aborted() {
      return aborted || Boolean(signal?.aborted);
    },
    track(client) {
      tracked.add(client);
    },
    dispose() {
      if (signal) signal.removeEventListener("abort", onAbort);
    },
  };
}

class WorkerClient {
  constructor(worker) {
    this.worker = worker;
    this.pending = new Map();
    this.nextId = 1;
    this.failed = false;
    worker.onmessage = (event) => {
      const { id, ok, result, error } = event.data;
      const entry = this.pending.get(id);
      if (!entry) return;
      this.pending.delete(id);
      if (ok) entry.resolve(result);
      else entry.reject(new Error(error || "psudo worker error"));
    };
    worker.onerror = (event) => {
      invalidateWorkerPool(
        this,
        new Error(event.message || "psudo worker failed")
      );
    };
  }

  rejectAll(error) {
    this.failed = true;
    for (const [, entry] of this.pending) entry.reject(error);
    this.pending.clear();
  }

  call(method, args, scope) {
    if (this.failed) {
      return Promise.reject(new Error("psudo worker is unavailable"));
    }
    if (scope?.aborted) {
      return Promise.reject(abortError());
    }
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      if (scope) scope.track(this);
      this.worker.postMessage({ id, method, args });
    });
  }
}

function invalidateWorkerPool(source, error) {
  if (!workerPool?.includes(source)) return;
  const failedPool = workerPool;
  workerPool = null;
  for (const client of failedPool) {
    client.rejectAll(error);
    client.worker.terminate();
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
    workerPool = Array.from({ length: n }, () => spawnWorkerClient());
  }
  return workerPool;
}

function callAny(method, args, scope) {
  const pool = getWorkerPool();
  return pool[0].call(method, args, scope);
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
  rescueRandomInit,
  profiled,
  polishEachRestart
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
    profiled,
    polishEachRestart,
  ];
}

function pickBest(outcomes) {
  let best = outcomes[0];
  for (let i = 1; i < outcomes.length; i++) {
    if (outcomes[i].total < best.total) best = outcomes[i];
  }
  return best;
}

async function runRestartWave(
  pool,
  commonArgs,
  count,
  seedSalt,
  rescueRandomInit,
  profiled,
  polishEachRestart,
  scope
) {
  let next = 0;
  const results = new Array(count);

  async function runOnClient(client) {
    while (true) {
      if (scope?.aborted) throw abortError();
      const i = next;
      next += 1;
      if (i >= count) break;
      const args = restartArgs(
        ...commonArgs,
        i,
        seedSalt,
        rescueRandomInit,
        profiled,
        polishEachRestart
      );
      results[i] = await client.call("nmRestart", args, scope);
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
  num_restarts,
  profiled = false,
  polishEachRestart = true,
  signal
) {
  const channels = colors.length / 3;
  const nFree = countFreeChannels(locked_colors, channels);
  if (nFree === 0) {
    // Every channel is pinned: the input palette is the answer, no worker needed.
    return Float32Array.from(colors, (c) => c / 255);
  }
  const nRestarts = effectiveNumRestarts(num_restarts, nFree);
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

  const scope = signal ? createAbortScope(signal) : null;
  try {
    await Promise.all(
      getWorkerPool().map((c) => c.call("warmup", [], scope))
    );

    const pool = getWorkerPool();
    const restartStarted = performance.now();
    const primary = await runRestartWave(
      pool,
      commonArgs,
      nRestarts,
      0,
      false,
      profiled,
      polishEachRestart,
      scope
    );

    const allOutcomes = [...primary];
    let best = pickBest(primary);

    if (nFree >= 6) {
      const { lTotRescue, rgbRescue } = adaptiveRescueBand(primary);
      const softRgb = MIN_DISPLAY_RGB_DISTANCE * 0.9;
      const rescueRestarts = Math.max(
        4,
        scaledBudget(HIGH_CH_RESCUE_RESTARTS_BASE, nFree)
      );
      let preferRandom = best.min_display_rgb_distance < softRgb;
      const salts = [RESCUE_SEED_SALT_1, RESCUE_SEED_SALT_2];
      for (let wave = 0; wave < 2; wave++) {
        if (scope?.aborted) throw abortError();
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
          preferRandom,
          profiled,
          polishEachRestart,
          scope
        );
        allOutcomes.push(...rescue);
        preferRandom = !preferRandom;
        const rescueBest = pickBest(rescue);
        if (rescueBest.total < best.total) best = rescueBest;
      }
    }

    if (scope?.aborted) throw abortError();

    const restartWallMs = performance.now() - restartStarted;
    if (!profiled) {
      return pool[0].call(
        "finalizePalette",
        [...commonArgs, best.oklab],
        scope
      );
    }

    const finalizeStarted = performance.now();
    const finalized = await pool[0].call(
      "finalizePaletteProfiled",
      [...commonArgs, best.oklab],
      scope
    );
    const finalizeWallMs = performance.now() - finalizeStarted;
    return {
      ...finalized,
      phases: {
        ...finalized.phases,
        restart_wall_ms: restartWallMs,
        finalize_wall_ms: finalizeWallMs,
        restart_context_worker_ms: allOutcomes.reduce(
          (sum, outcome) => sum + outcome.context_ms,
          0
        ),
        solver_worker_ms: allOutcomes.reduce(
          (sum, outcome) => sum + outcome.solver_ms,
          0
        ),
        restart_polish_worker_ms: allOutcomes.reduce(
          (sum, outcome) => sum + outcome.polish_ms,
          0
        ),
        solver_objective_evaluations: allOutcomes.reduce(
          (sum, outcome) => sum + outcome.solver_objective_evaluations,
          0
        ),
        restart_polish_objective_evaluations: allOutcomes.reduce(
          (sum, outcome) => sum + outcome.polish_objective_evaluations,
          0
        ),
        restarts_completed: allOutcomes.length,
      },
      restart_metrics: allOutcomes,
    };
  } finally {
    scope?.dispose();
  }
}

/** Enable/disable parallel NM multistarts across workers (default on). */
export function setParallelMultistart(enabled) {
  parallelMultistart = Boolean(enabled);
}

/**
 * Set the number of workers used for parallel multistart optimization.
 * Must be called before warmup or the first worker-backed operation.
 */
export function setWorkerPoolSize(size) {
  if (workerPool) {
    throw new Error(
      "setWorkerPoolSize must be called before warmup or optimization."
    );
  }
  const normalized = Number(size);
  if (!Number.isInteger(normalized) || normalized < 1) {
    throw new TypeError("Worker pool size must be a positive integer.");
  }
  configuredWorkerPoolSize = normalized;
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
  num_restarts,
  signal
) {
  if (signal?.aborted) {
    return Promise.reject(abortError());
  }
  const args = [
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
  ];
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
      num_restarts,
      false,
      true,
      signal
    ).catch((err) => {
      if (err?.name === "AbortError") throw err;
      console.warn(
        "[psudo] parallel optimize failed, falling back to single worker:",
        err
      );
      if (!signal) return callAny("optimize", args);
      const scope = createAbortScope(signal);
      return callAny("optimize", args, scope).finally(() => scope.dispose());
    });
  }
  if (!signal) return callAny("optimize", args);
  const scope = createAbortScope(signal);
  return callAny("optimize", args, scope).finally(() => scope.dispose());
}

/** Benchmark-only optimize path with full objective and phase telemetry. */
export function optimize_profiled(
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
  polish_each_restart = true,
  signal
) {
  if (signal?.aborted) {
    return Promise.reject(abortError());
  }
  if (!supportsWorker()) {
    throw new Error("optimize_profiled requires the browser worker-backed API.");
  }
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
    num_restarts,
    true,
    polish_each_restart,
    signal
  );
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

export function channel_gmm(array, subsample, tol, max_iter, n_runs) {
  return callAny("channel_gmm", [array, subsample, tol, max_iter, n_runs]);
}

export function ln(array) {
  return callAny("ln", [array]);
}
