/* Async worker-backed API (default package entry). */

export function warmup(): Promise<boolean[]>;

/** Toggle parallel NM multistarts across a worker pool (default on). */
export function setParallelMultistart(enabled: boolean): void;

export function ln(array: Uint16Array): Promise<Float32Array>;

/** Defaults: subsample=40000, tol=1e-6, max_iter=1000 */
export function channel_gmm(
  array: Uint16Array,
  subsample?: number,
  tol?: number,
  max_iter?: number
): Promise<Float32Array>;

/** Defaults match palette_study: max_iters=3000, confusion=32, spatial=false, num_restarts=18 (× n/3, max 40). luminance_values: recommended [50, 92] (OKLab L × 100). */
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
