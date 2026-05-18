/**
 * Default entry: async API backed by a shared module worker.
 * Sync WASM on the main thread: import from "psudo/sync".
 */

let worker = null;
let nextId = 1;
const pending = new Map();

function supportsWorker() {
  return typeof Worker !== "undefined" && typeof window !== "undefined";
}

function getWorker() {
  if (!supportsWorker()) {
    throw new Error(
      "psudo Web Worker is not available in this environment. Use `import * as psudo from \"psudo/sync\"`."
    );
  }
  if (!worker) {
    worker = new Worker(new URL("./psudo.worker.js", import.meta.url), {
      type: "module",
    });
    worker.onmessage = (event) => {
      const { id, ok, result, error } = event.data;
      const entry = pending.get(id);
      if (!entry) return;
      pending.delete(id);
      if (ok) entry.resolve(result);
      else entry.reject(new Error(error || "psudo worker error"));
    };
    worker.onerror = (event) => {
      for (const [, entry] of pending) {
        entry.reject(new Error(event.message || "psudo worker failed"));
      }
      pending.clear();
    };
  }
  return worker;
}

function call(method, args) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    getWorker().postMessage({ id, method, args });
  });
}

/** Preload WASM in the worker (optional, e.g. on app mount). */
export function warmup() {
  return call("warmup", []);
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
  return call("optimize", [
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
  return call("calculate_palette_loss", [
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
  return call("optimize_in_lens", [
    intensities,
    colors,
    contrast_limits,
    luminance_values,
  ]);
}

export function channel_gmm(array) {
  return call("channel_gmm", [array]);
}

export function ln(array) {
  return call("ln", [array]);
}
