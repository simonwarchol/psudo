/**
 * Module worker: loads WASM once and runs psudo exports off the main thread.
 */
import init, * as core from "./psudo.js";

let readyPromise;

function ensureReady() {
  if (!readyPromise) readyPromise = init();
  return readyPromise;
}

function cloneFloat32(src) {
  const out = new Float32Array(src.length);
  out.set(src);
  return out;
}

function packResult(result) {
  if (result instanceof Float32Array) {
    const copy = cloneFloat32(result);
    return { result: copy, transfer: [copy.buffer] };
  }
  return { result, transfer: [] };
}

self.onmessage = async (event) => {
  const { id, method, args } = event.data;
  try {
    await ensureReady();
    let result;
    switch (method) {
      case "warmup":
        result = true;
        break;
      case "optimize":
        result = core.optimize(
          args[0],
          args[1],
          args[2],
          args[3],
          args[4],
          args[5],
          args[6],
          args[7],
          args[8],
          args[9],
          args[10]
        );
        break;
      case "calculate_palette_loss":
        result = core.calculate_palette_loss(
          args[0],
          args[1],
          args[2],
          args[3],
          args[4],
          args[5],
          args[6]
        );
        break;
      case "optimize_in_lens":
        result = core.optimize_in_lens(args[0], args[1], args[2], args[3]);
        break;
      case "channel_gmm":
        result = core.channel_gmm(args[0]);
        break;
      case "ln":
        result = core.ln(args[0]);
        break;
      default:
        throw new Error(`unknown psudo worker method: ${method}`);
    }
    const packed = packResult(result);
    self.postMessage({ id, ok: true, result: packed.result }, packed.transfer);
  } catch (err) {
    self.postMessage({
      id,
      ok: false,
      error: err?.message ? String(err.message) : String(err),
    });
  }
};
