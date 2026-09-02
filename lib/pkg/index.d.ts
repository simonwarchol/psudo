/* Async worker-backed API (default package entry). */

export function warmup(): Promise<boolean[]>;

/** Toggle parallel NM multistarts across a worker pool (default on). */
export function setParallelMultistart(enabled: boolean): void;

/** Set worker count before warmup or the first worker-backed operation. */
export function setWorkerPoolSize(size: number): void;

export function ln(array: Uint16Array): Promise<Float32Array>;

/** Defaults: subsample=40000, tol=1e-6, max_iter=1000 */
export function channel_gmm(
  array: Uint16Array,
  subsample?: number,
  tol?: number,
  max_iter?: number
): Promise<Float32Array>;

/** Returns display-encoded sRGB values in [0, 1]. WASM defaults: max_iters=2700, confusion=32, spatial=false, num_restarts=18 (× n/3, max 40). luminance_values: recommended [50, 92] (OKLab L × 100). */
export function optimize(
  colors: Uint16Array,
  locked_colors: Uint16Array,
  intensities: Uint16Array,
  contrast_limits: Uint16Array,
  luminance_values: Uint16Array,
  excluded_colors: string[],
  color_names: string[],
  max_iters?: number,
  confusion_baseline_samples?: number,
  include_spatial_channel_overlap?: boolean,
  num_restarts?: number
): Promise<Float32Array>;

export interface PaletteObjectiveProfile {
  total: number;
  minus_mean_color_name_distance: number;
  minus_min_color_name_distance: number;
  minus_min_perceptual_distance: number;
  perceptual_deficit_penalty: number;
  min_display_rgb_distance: number;
  hue_separation_reward: number;
  hue_separation_deficit: number;
  min_hue_gap_deg: number;
  term_loss: number;
  confusion_weighted: number;
  minus_min_saturation: number;
  saturation_deficit_penalty: number;
  min_srgb_saturation: number;
  min_oklab_chroma: number;
}

export interface PalettePhaseProfile {
  context_ms: number;
  initial_polish_ms: number;
  refine_ms: number;
  restart_wall_ms: number;
  finalize_wall_ms: number;
  restart_context_worker_ms: number;
  solver_worker_ms: number;
  restart_polish_worker_ms: number;
  solver_objective_evaluations: number;
  restart_polish_objective_evaluations: number;
  initial_polish_objective_evaluations: number;
  refine_objective_evaluations: number;
  restarts_completed: number;
}

export interface PaletteRestartProfile {
  oklab: Float32Array;
  total: number;
  min_display_rgb_distance: number;
  context_ms: number;
  solver_ms: number;
  polish_ms: number;
  solver_objective_evaluations: number;
  polish_objective_evaluations: number;
}

export interface PaletteOptimizeProfile {
  /** Display-encoded sRGB in [0, 1]; the historical field name is retained for compatibility. */
  srgb_linear: Float32Array;
  oklab: Float32Array;
  objective: PaletteObjectiveProfile;
  phases: PalettePhaseProfile;
  restart_metrics: PaletteRestartProfile[];
}

/** @experimental Browser worker-backed benchmark path with full objective and phase telemetry. */
export function optimize_profiled(
  colors: Uint16Array,
  locked_colors: Uint16Array,
  intensities: Uint16Array,
  contrast_limits: Uint16Array,
  luminance_values: Uint16Array,
  excluded_colors: string[],
  color_names: string[],
  max_iters?: number,
  confusion_baseline_samples?: number,
  include_spatial_channel_overlap?: boolean,
  num_restarts?: number,
  polish_each_restart?: boolean
): Promise<PaletteOptimizeProfile>;

export function calculate_palette_loss(
  intensities: Uint16Array,
  colors: Uint16Array,
  contrast_limits: Uint16Array,
  luminance_values: Uint16Array,
  excluded_colors: string[],
  color_names: string[],
  include_spatial_channel_overlap?: boolean
): Promise<Record<string, number>>;

export function optimize_in_lens(
  intensities: Uint16Array,
  colors: Uint16Array,
  contrast_limits: Uint16Array,
  luminance_values: Uint16Array
): Promise<number>;
