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

function packNmRestart(result) {
  const oklab = cloneFloat32(result.oklab);
  return {
    result: {
      oklab,
      total: result.total,
      min_display_rgb_distance: result.min_display_rgb_distance,
      context_ms: result.context_ms,
      solver_ms: result.solver_ms,
      polish_ms: result.polish_ms,
      solver_objective_evaluations: result.solver_objective_evaluations,
      polish_objective_evaluations: result.polish_objective_evaluations,
    },
    transfer: [oklab.buffer],
  };
}

function packFinalizeProfile(result) {
  const srgbLinear = cloneFloat32(result.srgb_linear);
  const oklab = cloneFloat32(result.oklab);
  return {
    result: {
      srgb_linear: srgbLinear,
      oklab,
      objective: {
        total: result.total,
        minus_mean_color_name_distance:
          result.minus_mean_color_name_distance,
        minus_min_color_name_distance: result.minus_min_color_name_distance,
        minus_min_perceptual_distance:
          result.minus_min_perceptual_distance,
        perceptual_deficit_penalty: result.perceptual_deficit_penalty,
        min_display_rgb_distance: result.min_display_rgb_distance,
        hue_separation_reward: result.hue_separation_reward,
        hue_separation_deficit: result.hue_separation_deficit,
        min_hue_gap_deg: result.min_hue_gap_deg,
        term_loss: result.term_loss,
        confusion_weighted: result.confusion_weighted,
        minus_min_saturation: result.minus_min_saturation,
        saturation_deficit_penalty: result.saturation_deficit_penalty,
        min_srgb_saturation: result.min_srgb_saturation,
        min_oklab_chroma: result.min_oklab_chroma,
      },
      phases: {
        context_ms: result.context_ms,
        initial_polish_ms: result.initial_polish_ms,
        refine_ms: result.refine_ms,
        initial_polish_objective_evaluations:
          result.initial_polish_objective_evaluations,
        refine_objective_evaluations: result.refine_objective_evaluations,
      },
    },
    transfer: [srgbLinear.buffer, oklab.buffer],
  };
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
      case "nmRestart":
        result = core.run_nm_restart(
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
          args[10],
          args[11],
          args[12],
          args[13],
          args[14]
        );
        break;
      case "finalizePalette":
        result = core.finalize_palette_optimize(
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
      case "finalizePaletteProfiled":
        result = core.finalize_palette_optimize_profiled(
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
          args[10],
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
        result = core.channel_gmm(args[0], args[1], args[2], args[3], args[4]);
        break;
      case "ln":
        result = core.ln(args[0]);
        break;
      default:
        throw new Error(`unknown psudo worker method: ${method}`);
    }
    const packed =
      method === "nmRestart"
        ? packNmRestart(result)
        : method === "finalizePaletteProfiled"
          ? packFinalizeProfile(result)
          : packResult(result);
    self.postMessage({ id, ok: true, result: packed.result }, packed.transfer);
  } catch (err) {
    self.postMessage({
      id,
      ok: false,
      error: err?.message ? String(err.message) : String(err),
    });
  }
};
