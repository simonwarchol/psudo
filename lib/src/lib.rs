//! Primary wall-time costs: Monte Carlo confusion baseline, then simulated-annealing `Loss::cost`
//! (C3 name distance + regression confusion each iteration). Profiling: browser Performance panel
//! around `optimize`, or `cargo build --release` timings on native tests.

mod palette_diagnostics;
mod palette_eval;
mod palette_objective;
mod palette_refine;
mod utils;
use argmin::core::{CostFunction, Error, Executor, State};
use argmin::solver::simulatedannealing::{Anneal, SATempFunc, SimulatedAnnealing};
use linfa::prelude::Predict; // Continue to include linfa prelude for other necessary traits and structures
use linfa::traits::Fit; // Import the Fit trait
use linfa::Dataset;
use linfa_clustering::GaussianMixtureModel;
use linfa_linear;
use linfa_linear::LinearRegression;
use ndarray::Axis;
use ndarray::{Array1, Array2};
use ndarray_stats::QuantileExt; // <-- Add this line
use palette::{FromColor, Oklab, Srgb, Xyz};
use rand::seq::SliceRandom; // Import
use rand::thread_rng; // Import the RNG
use rand::Rng;
use rand_xoshiro::Xoshiro256PlusPlus;
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use statrs::distribution::{Continuous, Normal};
use std::sync::{Arc, Mutex};
use web_sys::console;

use rand::distributions::Uniform;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::{HashMap, HashSet};

use wasm_bindgen::prelude::*;

pub mod c3;

#[cfg(test)]
mod c3_migration_tests;

mod palette_solvers;

pub use palette_diagnostics::{
    coarse_name_family, compute_diagnostics, generate_feasible_seed_candidates,
    glasbey_like_seed_palette, select_best_restart, GlasbeyDistanceWeights,
    PaletteCandidateDiagnostics, PaletteInitMode, PaletteSelectionMode, RestartRecord,
    SeedCandidate, EARTH_TONE_TERMS,
};
pub use palette_objective::{
    current_min_name_weight, current_objective_mode, with_min_name_weight, with_objective_mode,
    PaletteObjectiveMode, MIN_NAME_DISTANCE_WEIGHT, MIN_OKLAB_DISTANCE, OKLAB_PERCEPTUAL_SCALE,
};
pub use palette_refine::{
    apply_palette_refine, apply_palette_refine_ex, oklab_to_oklch, oklch_to_oklab,
    refine_oklch_palette, PaletteRefineMode, PolarRefineStats,
};
pub use palette_solvers::{
    objective_total_for_oklab, run_palette_argmin_solver, scaled_solver_iters,
    study_postprocess_oklab, PaletteArgminSolver, PaletteSolverParams, SeedOverrideMode,
};
// `optimize_palette_with_solver` is defined in this crate root (benchmark tooling).

/// Nominal global-search budget before per-channel scaling (NM uses half as argmin iters).
const DEFAULT_MAX_ITERS: u32 = 3000;
#[cfg(target_arch = "wasm32")]
const DEFAULT_MAX_ITERS_WASM: u32 = 3000;
/// Independent global-search runs per `optimize` (Nelder–Mead multistart).
const DEFAULT_NUM_RESTARTS: u32 = 18;
#[cfg(target_arch = "wasm32")]
const DEFAULT_NUM_RESTARTS_WASM: u32 = 18;
/// Initial SA temperature (must match [`build_simulated_annealing_solver`] and [`Loss::anneal`] scaling).
const SA_INITIAL_TEMP: f32 = 12.0;
/// Exponential cooling factor per SA iteration: `T_i = T_0 * factor^i`.
const SA_TEMP_DECAY: f32 = 0.997;
/// Baseline confusion uses fewer MC draws than legacy 100 for speed; override via `optimize` options.
const DEFAULT_CONFUSION_BASELINE_SAMPLES: u32 = 32;
#[cfg(target_arch = "wasm32")]
const DEFAULT_CONFUSION_BASELINE_SAMPLES_WASM: u32 = 32;

#[inline]
fn default_max_iters() -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        return DEFAULT_MAX_ITERS_WASM;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        DEFAULT_MAX_ITERS
    }
}

#[inline]
fn default_num_restarts() -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        return DEFAULT_NUM_RESTARTS_WASM;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        DEFAULT_NUM_RESTARTS
    }
}

#[inline]
fn default_confusion_baseline_samples() -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        return DEFAULT_CONFUSION_BASELINE_SAMPLES_WASM;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        DEFAULT_CONFUSION_BASELINE_SAMPLES
    }
}

#[inline]
fn restart_count_bounds() -> (u32, u32) {
    #[cfg(target_arch = "wasm32")]
    {
        // Same max as native so default 18 × n/3 matches palette_study (6ch→36, 8ch→40).
        (1, 40)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        (4, 40)
    }
}

/// WASM defaults to color-only objective (spatial term is row-heavy).
#[inline]
fn default_include_spatial_overlap() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}

#[inline]
fn pipeline_use_full_postprocess() -> bool {
    true
}
/// Post-search polish / refine depth (see [`optimize_palette_pipeline`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OptimizePostprocess {
    #[default]
    /// Polish after every restart, quench pass, refine — best quality (app default).
    Full,
    /// One polish + refine on the best restart only; skips quench (batch studies).
    Study,
}

/// Golden-ratio stride for per-restart RNG seeds derived from [`problem_seed`].
const RESTART_SEED_STRIDE: u64 = 0x9E3779B97F4A7C15;
/// Base rescue multistarts (scaled by channel count).
const HIGH_CH_RESCUE_RESTARTS_BASE: u32 = 5;
const RESCUE_SEED_SALT_1: u64 = 0x8E5C_E500;
const RESCUE_SEED_SALT_2: u64 = 0xA11C_E5C0;
/// Weight on the multi-channel spatial confusion term (`compute_confusion_loss`).
pub(crate) const SPATIAL_CONFUSION_WEIGHT: f32 = 0.1;

/// NM simplex perturbation scale: grows gently with channel count.
#[inline]
fn nm_perturb_scale_for_channels(n_channels: usize) -> f32 {
    1.0 + 0.06 * ((n_channels as f32) - 3.0).clamp(0.0, 8.0)
}

/// Adaptive rescue gates from a multistart wave: stay near that problem's best restart.
/// Returns `(l_tot_rescue, rgb_rescue)` thresholds for [`outside_adaptive_band`].
fn adaptive_rescue_band(outcomes: &[NmRestartOutcome]) -> (f32, f32) {
    let mut best_l = f32::INFINITY;
    let mut best_min_rgb = 0.0f32;
    let mut sum = 0.0f32;
    let mut sum_sq = 0.0f32;
    let n = outcomes.len().max(1) as f32;
    for o in outcomes {
        sum += o.total;
        sum_sq += o.total * o.total;
        if o.total < best_l {
            best_l = o.total;
            best_min_rgb = o.min_display_rgb_distance;
        }
    }
    let soft = MIN_DISPLAY_RGB_DISTANCE as f32 * 0.9;
    if !best_l.is_finite() {
        return (0.0, soft);
    }
    let mean = sum / n;
    let var = (sum_sq / n - mean * mean).max(0.0);
    let std = var.sqrt();
    let margin = (0.5 * std).clamp(0.12, 0.35);
    (best_l + margin, best_min_rgb.min(soft))
}

#[inline]
fn outside_adaptive_band(total: f32, min_rgb: f32, l_tot_rescue: f32, rgb_rescue: f32) -> bool {
    min_rgb < rgb_rescue || total > l_tot_rescue
}

// Palette objective (minimize total):
//   −mean C3 name distance  −min display-sRGB Δ  + perc_deficit
//   + hue_reward + hue_deficit  + term_loss  + w·confusion
//   −w_sat·min(chroma,sat)  + w_def·Σ(chroma/sat deficits)²
// C3 naming + hue gaps use display-projected (gamut-clipped) colors so they match swatches.
// Hard floors in SA `anneal`: OKLab chroma ≥ DEFAULT_MIN_OKLAB_CHROMA, sRGB sat ≥ MIN_SRGB_SATURATION.
/// Minimum OKLab chroma √(a²+b²) per channel (optimization space).
pub(crate) const DEFAULT_MIN_OKLAB_CHROMA: f32 = 0.16;
/// Minimum linear-sRGB saturation (max−min)/max per channel — matches “pale” in the viewer.
pub(crate) const MIN_SRGB_SATURATION: f32 = 0.42;
/// Penalty when any channel violates OKLab chroma or sRGB saturation floors.
const SATURATION_DEFICIT_WEIGHT: f32 = 10.0;
/// Reward on the *dullest* channel: `-weight * min(min_oklab_chroma, min_srgb_sat)`.
const MIN_SAT_REWARD_WEIGHT: f32 = 2.5;
/// Target minimum display-sRGB distance (0–255 scale) between channel colors.
pub(crate) const MIN_DISPLAY_RGB_DISTANCE: f64 = 90.0;
pub(crate) const PERCEPTUAL_SCALE: f64 = 255.0;
pub(crate) const PERCEPTUAL_DEFICIT_WEIGHT: f32 = 6.0;
/// Weight on circular hue separation of display-projected OKLab (`0` disables).
/// Prevents light+dark versions of the same warm hue from padding ΔE / RGB distance.
pub(crate) const HUE_SEPARATION_WEIGHT: f32 = 3.0;
/// Chroma below which hue is treated as undefined (skip that channel in the min-gap).
const HUE_MIN_CHROMA: f32 = 0.04;
// OKLab separation floors/scales live in `palette_objective` (`MIN_OKLAB_DISTANCE`,
// `OKLAB_PERCEPTUAL_SCALE`) for the study-only `oklab_sep` objective mode.
/// C3 color-name terms always penalized unless the user explicitly allows them.
const DEFAULT_EXCLUDED_COLOR_NAMES: &[&str] = &[
    "grey",
    "white",
    "lightgrey",
    "darkgrey",
    "offwhite",
    "greyblue",
    "greygreen",
    "bluegrey",
    "lightbluegrey",
];

pub(crate) fn oklab_chroma(a: f32, b: f32) -> f32 {
    (a * a + b * b).sqrt()
}

pub(crate) fn channel_srgb_saturation(l: f32, a: f32, b: f32) -> f32 {
    let okl = Oklab::new(l, a, b);
    let rgb: Srgb = Srgb::from_color(okl);
    let mx = rgb.red.max(rgb.green).max(rgb.blue);
    let mn = rgb.red.min(rgb.green).min(rgb.blue);
    if mx < 1e-5 {
        0.0
    } else {
        (mx - mn) / mx
    }
}

fn min_channel_oklab_chroma(oklab_flat: &[f32]) -> f32 {
    oklab_flat
        .chunks(3)
        .map(|c| oklab_chroma(c[1], c[2]))
        .fold(f32::INFINITY, f32::min)
}

fn min_channel_srgb_saturation(oklab_flat: &[f32]) -> f32 {
    oklab_flat
        .chunks(3)
        .map(|c| channel_srgb_saturation(c[0], c[1], c[2]))
        .fold(f32::INFINITY, f32::min)
}

/// OKLab chroma + display saturation: one deficit term, one min-channel reward.
pub(crate) fn saturation_objective_terms(oklab_flat: &[f32]) -> (f32, f32, f32, f32) {
    let min_ch = min_channel_oklab_chroma(oklab_flat);
    let min_sat = min_channel_srgb_saturation(oklab_flat);
    let mut deficit_sq = 0.0f32;
    for c in oklab_flat.chunks(3) {
        let ch = oklab_chroma(c[1], c[2]);
        let d_ch = DEFAULT_MIN_OKLAB_CHROMA - ch;
        if d_ch > 0.0 {
            deficit_sq += d_ch * d_ch;
        }
        let sat = channel_srgb_saturation(c[0], c[1], c[2]);
        let d_sat = MIN_SRGB_SATURATION - sat;
        if d_sat > 0.0 {
            deficit_sq += d_sat * d_sat;
        }
    }
    let saturation_deficit_penalty = deficit_sq * SATURATION_DEFICIT_WEIGHT;
    let min_bottleneck = min_ch.min(min_sat);
    let minus_min_saturation = -min_bottleneck * MIN_SAT_REWARD_WEIGHT;
    (
        minus_min_saturation,
        saturation_deficit_penalty,
        min_sat,
        min_ch,
    )
}

/// Smallest circular gap (degrees) between two hue angles.
#[inline]
pub(crate) fn circular_hue_gap_deg(h0: f32, h1: f32) -> f32 {
    let mut d = (h0 - h1).abs() % 360.0;
    if d > 180.0 {
        d = 360.0 - d;
    }
    d
}

/// Target minimum hue separation (degrees) for `n` channels: equal spacing `360/n`.
#[inline]
pub(crate) fn min_hue_separation_target_deg(n_channels: usize) -> f32 {
    let equal = 360.0 / n_channels.max(2) as f32;
    equal.clamp(30.0, 175.0)
}

/// Hue separation on **display-projected** OKLab: `(reward, deficit, min_gap_deg)`.
/// Using the gamut-clipped preimage stops yellow+brown from looking well-spread in
/// unbounded OKLab while collapsing to the same warm display hue.
pub(crate) fn hue_separation_terms(oklab_flat: &[f32], weight: f32) -> (f32, f32, f32) {
    if weight <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let n = oklab_flat.len() / 3;
    if n < 2 {
        return (0.0, 0.0, 0.0);
    }
    let mut projected = Vec::with_capacity(oklab_flat.len());
    palette_eval::project_oklab_through_display(oklab_flat, &mut projected);

    let mut hues = Vec::with_capacity(n);
    let mut chromas = Vec::with_capacity(n);
    for c in projected.chunks(3) {
        chromas.push(oklab_chroma(c[1], c[2]));
        hues.push(c[2].atan2(c[1]).to_degrees());
    }
    let mut min_gap = f32::INFINITY;
    let mut any = false;
    for i in 0..n {
        if chromas[i] < HUE_MIN_CHROMA {
            continue;
        }
        for j in i + 1..n {
            if chromas[j] < HUE_MIN_CHROMA {
                continue;
            }
            min_gap = min_gap.min(circular_hue_gap_deg(hues[i], hues[j]));
            any = true;
        }
    }
    if !any || !min_gap.is_finite() {
        return (0.0, 0.0, 0.0);
    }
    let scale = 180.0f32;
    let target = min_hue_separation_target_deg(n);
    let base = min_gap.min(target);
    let excess = (min_gap - target).max(0.0);
    let reward = -weight * (base + 0.25 * excess) / scale;
    let deficit = (target - min_gap).max(0.0) / scale;
    let deficit_penalty = weight * 16.0 * deficit * deficit;
    (
        if reward.is_finite() { reward } else { 0.0 },
        if deficit_penalty.is_finite() {
            deficit_penalty
        } else {
            0.0
        },
        min_gap,
    )
}

/// Per-channel C3 label and hue for debugging palettes (e.g. red+pink vs R/G/B spread).
#[derive(Clone, Debug)]
pub struct PaletteChannelDebug {
    pub name: String,
    pub chroma: f32,
    pub srgb_saturation: f32,
    pub hue_deg: f32,
}

pub fn debug_palette_channels(c3: &c3::C3, oklab_flat: &[f32]) -> Vec<PaletteChannelDebug> {
    oklab_flat
        .chunks(3)
        .map(|c| {
            // Name the gamut-clipped display color so labels match swatches.
            let lab = palette_eval::lab_from_display_clipped_oklab(c[0], c[1], c[2]);
            let name = c3.dominant_term_name(lab);
            let ch = oklab_chroma(c[1], c[2]);
            let sat = channel_srgb_saturation(c[0], c[1], c[2]);
            let hue_deg = c[2].atan2(c[1]).to_degrees();
            PaletteChannelDebug {
                name,
                chroma: ch,
                srgb_saturation: sat,
                hue_deg,
            }
        })
        .collect()
}

fn random_saturated_oklab_ab(rng: &mut impl Rng, min_chroma: f32) -> (f32, f32) {
    let angle = rng.gen_range(0.0f32..std::f32::consts::TAU);
    let lo = min_chroma.max(DEFAULT_MIN_OKLAB_CHROMA);
    let chroma = rng.gen_range(lo..0.38f32);
    (chroma * angle.cos(), chroma * angle.sin())
}

/// Hard floor on saturation in OKLab and in linear sRGB (post-gamma appearance).
pub(crate) fn enforce_channel_saturation(
    oklab_flat: &mut [f32],
    color_idx: usize,
    rng: &mut impl Rng,
) {
    let base = color_idx * 3;
    for _ in 0..32 {
        let ch = oklab_chroma(oklab_flat[base + 1], oklab_flat[base + 2]);
        let sat =
            channel_srgb_saturation(oklab_flat[base], oklab_flat[base + 1], oklab_flat[base + 2]);
        if ch >= DEFAULT_MIN_OKLAB_CHROMA && sat >= MIN_SRGB_SATURATION {
            return;
        }
        if ch < DEFAULT_MIN_OKLAB_CHROMA {
            let (a, b) = random_saturated_oklab_ab(rng, DEFAULT_MIN_OKLAB_CHROMA);
            oklab_flat[base + 1] = a;
            oklab_flat[base + 2] = b;
        } else if sat < MIN_SRGB_SATURATION {
            let scale = (MIN_SRGB_SATURATION / sat.max(0.08)).min(2.5);
            oklab_flat[base + 1] = (oklab_flat[base + 1] * scale).clamp(-0.4, 0.4);
            oklab_flat[base + 2] = (oklab_flat[base + 2] * scale).clamp(-0.4, 0.4);
        }
    }
    let (a, b) = random_saturated_oklab_ab(rng, DEFAULT_MIN_OKLAB_CHROMA);
    oklab_flat[base + 1] = a;
    oklab_flat[base + 2] = b;
}

/// User exclusions plus default achromatic C3 terms (grey/white family).
pub fn merge_excluded_color_names(user: Vec<String>) -> Vec<String> {
    let mut names: HashSet<String> = DEFAULT_EXCLUDED_COLOR_NAMES
        .iter()
        .map(|s| s.to_string())
        .collect();
    names.extend(user);
    names.into_iter().collect()
}

/// Stable hash of problem inputs so the same image + settings get the same MC baseline and restart seeds.
pub fn problem_seed(
    colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &x in colors
        .iter()
        .chain(intensities.iter())
        .chain(contrast_limits.iter())
        .chain(luminance_values.iter())
    {
        h ^= x as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn build_simulated_annealing_solver(
    max_iters: u64,
    initial_temp: f32,
) -> Result<SimulatedAnnealing<f32, Xoshiro256PlusPlus>, Error> {
    let reanneal = (max_iters / 3).clamp(200, 800);
    Ok(SimulatedAnnealing::new(initial_temp)?
        .with_temp_func(SATempFunc::Exponential(SA_TEMP_DECAY))
        .with_reannealing_best(reanneal))
}

/// Scale iteration / restart budgets with channel count (6 ch ≈ 2× cost of 3 ch).
fn scale_budget_for_channels(base: u32, n_channels: usize) -> u32 {
    let n = n_channels.max(1) as u64;
    ((base as u64 * n) / 3).max(1) as u32
}

const QUENCH_TEMP: f32 = 2.5;

fn enforce_all_channel_saturation(oklab: &mut [f32], rng: &mut impl Rng) {
    let n = oklab.len() / 3;
    for color_idx in 0..n {
        enforce_channel_saturation(oklab, color_idx, rng);
    }
}

/// Deterministic coordinate polish after SA (steepest-descent sweeps, then fine pass).
pub(crate) fn polish_oklab_palette(
    oklab: &mut Vec<f32>,
    locked_colors: &[bool],
    luminance_values: &[f32],
    c3: &c3::C3,
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32],
    fast: bool,
) {
    let cost = |p: &[f32]| {
        evaluate_palette_objective_breakdown_with_excluded_set(
            c3,
            p,
            intensity_arc,
            avg_confusion,
            spatial_w,
            excluded_set,
            color_name_indices,
        )
        .total
    };
    let n_colors = oklab.len() / 3;
    if n_colors == 0 {
        return;
    }
    crate::palette_solvers::clamp_oklab_to_luminance_bounds(oklab, luminance_values);
    let schedules: &[([f32; 3], [f32; 3])] = if fast {
        &[([0.022, 0.009, 0.004], [0.032, 0.014, 0.006])]
    } else {
        &[
            ([0.035, 0.015, 0.006], [0.05, 0.02, 0.008]),
            ([0.012, 0.005, 0.002], [0.018, 0.007, 0.003]),
        ]
    };
    for (step_l, step_ab) in schedules {
        for &sl in step_l {
            for &sab in step_ab {
                let mut rounds = 0u32;
                loop {
                    if fast {
                        rounds += 1;
                        if rounds > 1 {
                            break;
                        }
                    }
                    let base_cost = cost(oklab);
                    let mut best: Option<(Vec<f32>, f32)> = None;
                    for color_idx in 0..n_colors {
                        if locked_colors[color_idx] {
                            continue;
                        }
                        let base = color_idx * 3;
                        for (comp, step) in [(0usize, sl), (1, sab), (2, sab)] {
                            for sign in [-1.0f32, 1.0f32] {
                                let mut trial = oklab.clone();
                                trial[base + comp] += sign * step;
                                if comp == 0 {
                                    trial[base] =
                                        trial[base].clamp(luminance_values[0], luminance_values[1]);
                                } else {
                                    trial[base + comp] = trial[base + comp].clamp(-0.4, 0.4);
                                }
                                let mut rng = StdRng::seed_from_u64(
                                    (base as u64)
                                        .wrapping_add(comp as u64)
                                        .wrapping_add(trial[base + 1].to_bits() as u64),
                                );
                                enforce_channel_saturation(&mut trial, color_idx, &mut rng);
                                let trial_cost = cost(&trial);
                                if trial_cost + 1e-7 < base_cost {
                                    if best.as_ref().map_or(true, |(_, c)| trial_cost < *c) {
                                        best = Some((trial, trial_cost));
                                    }
                                }
                            }
                        }
                    }
                    match best {
                        Some((trial, _)) => *oklab = trial,
                        None => break,
                    }
                }
            }
        }
    }
    let mut polish_rng = StdRng::seed_from_u64(0x50C1_4E1A_DEAD);
    enforce_all_channel_saturation(oklab, &mut polish_rng);
}

/// Random single-channel jitters + polish; escapes shallow SA local minima.
pub(crate) fn refine_oklab_palette(
    oklab: &mut Vec<f32>,
    locked_colors: &[bool],
    luminance_values: &[f32],
    c3: &c3::C3,
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32],
    rng_seed: u64,
) {
    let n_colors = oklab.len() / 3;
    if n_colors == 0 {
        return;
    }
    let n_trials = scale_budget_for_channels(24, n_colors);
    let mut rng = StdRng::seed_from_u64(rng_seed);
    let mut best_cost = evaluate_palette_objective_breakdown_with_excluded_set(
        c3,
        oklab,
        intensity_arc,
        avg_confusion,
        spatial_w,
        excluded_set,
        color_name_indices,
    )
    .total;
    for _ in 0..n_trials {
        let color_idx = rng.gen_range(0..n_colors);
        if locked_colors[color_idx] {
            continue;
        }
        let base = color_idx * 3;
        let mut trial = oklab.clone();
        trial[base] += rng.gen_range(-0.05f32..0.05f32);
        trial[base] = trial[base].clamp(luminance_values[0], luminance_values[1]);
        trial[base + 1] += rng.gen_range(-0.08f32..0.08f32);
        trial[base + 2] += rng.gen_range(-0.08f32..0.08f32);
        trial[base + 1] = trial[base + 1].clamp(-0.4, 0.4);
        trial[base + 2] = trial[base + 2].clamp(-0.4, 0.4);
        enforce_channel_saturation(&mut trial, color_idx, &mut rng);
        let trial_cost = evaluate_palette_objective_breakdown_with_excluded_set(
            c3,
            &trial,
            intensity_arc,
            avg_confusion,
            spatial_w,
            excluded_set,
            color_name_indices,
        )
        .total;
        if trial_cost + 1e-7 < best_cost {
            *oklab = trial;
            best_cost = trial_cost;
        }
    }
    polish_oklab_palette(
        oklab,
        locked_colors,
        luminance_values,
        c3,
        intensity_arc,
        avg_confusion,
        spatial_w,
        excluded_set,
        color_name_indices,
        false,
    );
}

fn random_palette_mc_sample(
    colors: &[f32],
    luminance_values: &[f32],
    rng: &mut impl Rng,
) -> Vec<f32> {
    let mut random_colors = Vec::new();
    for _color in colors.chunks(3) {
        for (i, _) in _color.iter().enumerate() {
            let val = if i == 0 {
                rng.gen_range(luminance_values[0]..luminance_values[1])
            } else {
                rng.gen_range(-0.4..0.4)
            };
            random_colors.push(val);
        }
    }
    random_colors
}

fn random_initial_oklab(
    oklab_flat: &[f32],
    locked_colors: &[bool],
    luminance_values: &[f32],
    rng: &mut impl Rng,
) -> Vec<f32> {
    let n_colors = oklab_flat.len() / 3;
    let mut out = oklab_flat.to_vec();
    for color_idx in 0..n_colors {
        if locked_colors[color_idx] {
            continue;
        }
        let base = color_idx * 3;
        out[base] = rng.gen_range(luminance_values[0]..luminance_values[1]);
        let (a, b) = random_saturated_oklab_ab(rng, DEFAULT_MIN_OKLAB_CHROMA);
        out[base + 1] = a;
        out[base + 2] = b;
    }
    out
}

/// OKLab a/b (and L) of an sRGB primary (R, G, or B), chroma-clamped to the search box.
fn srgb_primary_oklab(which: usize) -> (f32, f32, f32) {
    let rgb = match which % 3 {
        0 => Srgb::new(1.0, 0.0, 0.0),
        1 => Srgb::new(0.0, 1.0, 0.0),
        _ => Srgb::new(0.0, 0.0, 1.0),
    };
    let okl: Oklab = Oklab::from_color(rgb);
    (okl.l, okl.a.clamp(-0.4, 0.4), okl.b.clamp(-0.4, 0.4))
}

/// RGB primaries first (R, G, B), then evenly spaced hues for any remaining channels.
/// Leading with saturated primaries gives Nelder–Mead a strong, distinct basin to refine.
fn spread_initial_oklab(
    fallback: &[f32],
    locked_colors: &[bool],
    luminance_values: &[f32],
    n_colors: usize,
    rng: &mut impl Rng,
) -> Vec<f32> {
    let mut out = fallback.to_vec();
    let n_primaries = n_colors.min(3);
    for color_idx in 0..n_primaries {
        if locked_colors[color_idx] {
            continue;
        }
        let base = color_idx * 3;
        let (l, a, b) = srgb_primary_oklab(color_idx);
        out[base] = l.clamp(luminance_values[0], luminance_values[1]);
        out[base + 1] = a;
        out[base + 2] = b;
        enforce_channel_saturation(&mut out, color_idx, rng);
    }
    if n_colors <= 3 {
        return out;
    }
    let l = 0.58f32;
    let extra = n_colors - 3;
    for i in 0..extra {
        let color_idx = 3 + i;
        if locked_colors[color_idx] {
            continue;
        }
        let base = color_idx * 3;
        let angle = std::f32::consts::TAU * ((i as f32) + 0.5) / (extra as f32)
            + rng.gen_range(-0.08f32..0.08f32);
        let chroma = rng.gen_range(0.18f32..0.34f32);
        out[base] = l.clamp(luminance_values[0], luminance_values[1]);
        out[base + 1] = chroma * angle.cos();
        out[base + 2] = chroma * angle.sin();
        enforce_channel_saturation(&mut out, color_idx, rng);
    }
    out
}

/// Per-restart NM/SA start (restart 0). See [`sa_initial_oklab_for_restart`].
#[inline]
pub(crate) fn sa_initial_oklab(
    oklab_flat: &[f32],
    locked_colors: &[bool],
    luminance_values: &[f32],
    init_seed: u64,
    rng: &mut impl Rng,
) -> Vec<f32> {
    sa_initial_oklab_for_restart(
        oklab_flat,
        locked_colors,
        luminance_values,
        init_seed,
        0,
        rng,
    )
}

/// Per-restart NM/SA start. Restart 0 leads with the RGB-primary spread; most later
/// restarts use random saturated OKLab for basin diversity under the harder objective.
#[inline]
pub(crate) fn sa_initial_oklab_for_restart(
    oklab_flat: &[f32],
    locked_colors: &[bool],
    luminance_values: &[f32],
    init_seed: u64,
    restart: u32,
    rng: &mut impl Rng,
) -> Vec<f32> {
    let n_colors = oklab_flat.len() / 3;
    let _ = init_seed;
    // After restart 0: ~2/3 random, ~1/3 jittered spread (keeps primary-basin coverage).
    if restart > 0 && n_colors >= 2 && restart % 3 != 0 {
        return random_initial_oklab(oklab_flat, locked_colors, luminance_values, rng);
    }
    if n_colors >= 1 {
        spread_initial_oklab(oklab_flat, locked_colors, luminance_values, n_colors, rng)
    } else {
        random_initial_oklab(oklab_flat, locked_colors, luminance_values, rng)
    }
}

fn preprocess_data(colors: &[u16], intensities: &[u16], contrast_limits: &[u16]) -> Array2<f32> {
    let num_channels = colors.len() / 3;
    let num_rows = intensities.len() / num_channels;
    let mut intensities_array = Array2::zeros((num_rows, num_channels));
    for channel in 0..num_channels {
        for row in 0..num_rows {
            let index = channel * num_rows + row;
            intensities_array[[row, channel]] = intensities[index];
        }
    }
    let mut intensities_array_float: Array2<f32> = Array2::zeros((num_rows, num_channels));

    for channel in 0..num_channels {
        for row in 0..num_rows {
            let index = channel * num_rows + row;
            if intensities[index] < contrast_limits[channel * 2] {
                intensities_array[[row, channel]] = contrast_limits[channel * 2];
            } else if intensities[index] > contrast_limits[channel * 2 + 1] {
                intensities_array[[row, channel]] = contrast_limits[channel * 2 + 1];
            }
            // Subtract the lower limit from the value
            intensities_array[[row, channel]] -= contrast_limits[channel * 2];
            intensities_array_float[[row, channel]] = (intensities_array[[row, channel]] as f32)
                / ((contrast_limits[channel * 2 + 1] - contrast_limits[channel * 2]) as f32);
        }
    }
    // Compute the sum of each row
    // let mut row_sums = Array1::zeros(num_rows);
    let mut indexes = Vec::new();
    for row in 0..num_rows {
        let mut row_sum = 0.0;
        for channel in 0..num_channels {
            row_sum += intensities_array_float[[row, channel]];
        }
        if row_sum > 0.3 {
            // print row index
            indexes.push(row);
        }
    }
    // println!("indexes: {:?}", indexes);
    // Shuffle the indexes
    let mut rng = thread_rng();
    indexes.shuffle(&mut rng);
    #[cfg(debug_assertions)]
    println!("indexes length: {:?}", indexes.len());
    indexes = indexes
        .iter()
        .take(5000)
        .map(|&x| x)
        .collect::<Vec<usize>>();

    // subsample intensities_array_float to the first 5000 indices
    let mut subsampled_array = Array2::zeros((indexes.len(), num_channels));
    for (i, &index) in indexes.iter().enumerate() {
        for channel in 0..num_channels {
            subsampled_array[[i, channel]] = intensities_array_float[[index, channel]];
        }
    }
    #[cfg(debug_assertions)]
    println!("subsampled_array shape: {:?}", subsampled_array.shape());
    subsampled_array
}

#[wasm_bindgen]
pub fn ln(array: &[u16]) -> Vec<f32> {
    let array_vec = array.to_vec();
    #[cfg(target_arch = "wasm32")]
    let vals: Vec<f32> = array_vec.iter().map(|&x| x as f32).collect();
    #[cfg(not(target_arch = "wasm32"))]
    let vals = array_vec
        .par_iter()
        .map(|&x| x as f32)
        .collect::<Vec<f32>>();
    // take a random sample of 1000 values
    // Iterate over vals, if value is 0 or nan, make it 0, otherwise take the log

    let vals_log = vals
        .iter()
        .map(|&x| if x <= 0.0 || x.is_nan() { 0.0 } else { x.ln() })
        .collect::<Array1<f32>>();
    return vals_log.to_vec();
}

const DEFAULT_GMM_SUBSAMPLE: usize = 40_000;
const DEFAULT_GMM_TOLERANCE: f32 = 1e-6;
const DEFAULT_GMM_MAX_ITER: u64 = 1000;

#[wasm_bindgen]
pub fn channel_gmm(
    array: &[u16],
    subsample: Option<u32>,
    tol: Option<f32>,
    max_iter: Option<u32>,
) -> Vec<f32> {
    // console::log_1(&"Starting GMM".into());
    let subsample = subsample
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_GMM_SUBSAMPLE)
        .max(1);
    let tol = tol.unwrap_or(DEFAULT_GMM_TOLERANCE);
    let max_iter = max_iter
        .map(|n| n as u64)
        .unwrap_or(DEFAULT_GMM_MAX_ITER)
        .max(1);

    let sampled_array = if array.len() > subsample {
        let mut rng = rand::thread_rng();
        array
            .choose_multiple(&mut rng, subsample)
            .cloned()
            .collect::<Vec<_>>()
    } else {
        array.to_vec()
    };

    // Match Python: np.log(image_data[image_data > 0])
    #[cfg(target_arch = "wasm32")]
    let vals: Vec<f32> = sampled_array
        .iter()
        .filter_map(|&x| {
            let v = x as f32;
            if v > 0.0 && !v.is_nan() {
                Some(v)
            } else {
                None
            }
        })
        .collect();
    #[cfg(not(target_arch = "wasm32"))]
    let vals = sampled_array
        .par_iter()
        .filter_map(|&x| {
            let v = x as f32;
            if v > 0.0 && !v.is_nan() {
                Some(v)
            } else {
                None
            }
        })
        .collect::<Vec<f32>>();

    if vals.is_empty() {
        panic!("GMM fitting failed: no positive intensities");
    }

    let vals_log = vals.iter().map(|&x| x.ln()).collect::<Array1<f32>>();

    let dataset = Dataset::from(vals_log.insert_axis(Axis(1)));

    // console::log_1(&"Created Dataset!".into());
    let gmm_result = GaussianMixtureModel::params(3)
        .n_runs(10)
        .tolerance(tol)
        .max_n_iterations(max_iter)
        .init_method(linfa_clustering::GmmInitMethod::Random)
        .fit(&dataset);

    let gmm = match gmm_result {
        Ok(g) => g,
        Err(e) => {
            let error_message = format!("GMM fitting error: {:?}", e);
            console::log_1(&error_message.into());
            panic!("GMM fitting failed!");
        }
    };
    let means = gmm.means();
    let covariances = gmm.covariances();
    let weights = gmm.weights();

    let flattend_means = means
        .view()
        .into_shape((means.len(),))
        .unwrap()
        .to_owned()
        .into_raw_vec();
    let mut indexed_values: Vec<(usize, f32)> = flattend_means
        .iter()
        .enumerate()
        .map(|(i, &val)| (i, val))
        .collect();
    indexed_values.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    // Extract indices from the sorted pairs.
    let (_i0, i1, i2) = (
        indexed_values[0].0,
        indexed_values[1].0,
        indexed_values[2].0,
    );
    let (mean1, mean2) = (flattend_means[i1], flattend_means[i2]);
    let (std1, std2) = (
        covariances[[i1, 0, 0]].sqrt(),
        covariances[[i2, 0, 0]].sqrt(),
    );
    // Python code to implement
    let x = Array1::linspace(mean1, mean2, 50);
    let norm1 = Normal::new(mean1 as f64, std1 as f64).unwrap();
    let norm2 = Normal::new(mean2 as f64, std2 as f64).unwrap();
    let y1 = x.mapv(|v| norm1.pdf(v as f64) * (weights[i1] as f64));
    let y2 = x.mapv(|v| norm2.pdf(v as f64) * (weights[i2] as f64));
    let lmax = mean2 + 2.0 * std2;
    // Calculate the differences between y1 and y2, take their absolute values, and get the index of the minimum value
    let differences = (&y1 - &y2).mapv(|val| val.abs());
    let mut min_diff_index: usize = 0;
    let mut min_diff_value = f64::MAX;
    for (i, &diff) in differences.iter().enumerate() {
        if diff < min_diff_value {
            min_diff_index = i;
            min_diff_value = diff;
        }
    }
    let mut lmin = x[min_diff_index];
    // Apply the given condition
    if lmin >= mean2 {
        lmin = mean2 - 2.0 * std2;
    }
    let vals_array = Array1::from(vals);

    let vmin = f32::max(lmin.exp(), f32::max(*vals_array.min().unwrap(), 0.0));
    let vmax = f32::min(lmax.exp(), *vals_array.max().unwrap());
    return vec![vmin, vmax];
}

/// Euclidean distance in gamma-encoded sRGB (0–255), matching what the viewer displays.
pub(crate) fn display_srgb_distance(oklab_flat: &[f32], i: usize, j: usize) -> f64 {
    let to8 = |idx: usize| -> [f64; 3] {
        let okl = Oklab::new(
            oklab_flat[idx * 3],
            oklab_flat[idx * 3 + 1],
            oklab_flat[idx * 3 + 2],
        );
        let rgb: Srgb = Srgb::from_color(okl);
        [
            (rgb.red.clamp(0.0, 1.0) * 255.0) as f64,
            (rgb.green.clamp(0.0, 1.0) * 255.0) as f64,
            (rgb.blue.clamp(0.0, 1.0) * 255.0) as f64,
        ]
    };
    let a = to8(i);
    let b = to8(j);
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    (dr * dr + dg * dg + db * db).sqrt()
}

fn perceptual_objective_terms(oklab_flat: &[f32]) -> (f32, f32, f32) {
    let n = oklab_flat.len() / 3;
    if n < 2 {
        return (0.0, 0.0, 0.0);
    }
    let mut min_dist = f64::MAX;
    let mut deficit_sq = 0.0f64;
    for i in 0..n {
        for j in i + 1..n {
            let d = display_srgb_distance(oklab_flat, i, j);
            min_dist = min_dist.min(d);
            let deficit = (MIN_DISPLAY_RGB_DISTANCE - d).max(0.0);
            deficit_sq += deficit * deficit;
        }
    }
    let scale = PERCEPTUAL_SCALE;
    let minus_min = -(min_dist / scale) as f32;
    let deficit_penalty = (deficit_sq / (scale * scale) * PERCEPTUAL_DEFICIT_WEIGHT as f64) as f32;
    (minus_min, deficit_penalty, min_dist as f32)
}

// ///////////////////////////////////////// Optimization /////////////////////////////////////
struct Loss {
    rng: Arc<Mutex<Xoshiro256PlusPlus>>,
    pub(crate) locked_colors: Vec<bool>,
    intensity_array: Arc<Array2<f32>>,
    pub(crate) luminance_values: Vec<f32>,
    avg_confusion: f32,
    /// When `0.0`, skip `compute_confusion_loss` (no spatial multi-channel overlap objective).
    spatial_confusion_weight: f32,
    excluded_colors_set: HashSet<usize>,
    color_name_indices: Vec<f32>,
    c3_instance: Arc<c3::C3>,
}

impl Loss {
    pub fn new(
        locked_colors: Vec<bool>,
        intensity_array: Arc<Array2<f32>>,
        luminance_values: Vec<f32>,
        avg_confusion: f32,
        spatial_confusion_weight: f32,
        excluded_colors_indices: Vec<f32>,
        color_name_indices: Vec<f32>,
        c3_instance: Arc<c3::C3>,
        anneal_rng_seed: Option<u64>,
    ) -> Self {
        let excluded_colors_set: HashSet<usize> = excluded_colors_indices
            .iter()
            .map(|&x| x as usize)
            .collect();
        let rng_inner = match anneal_rng_seed {
            Some(s) => Xoshiro256PlusPlus::seed_from_u64(s),
            None => Xoshiro256PlusPlus::from_entropy(),
        };
        Self {
            rng: Arc::new(Mutex::new(rng_inner)),
            locked_colors: locked_colors,
            c3_instance: c3_instance,
            intensity_array: intensity_array,
            luminance_values: luminance_values,
            avg_confusion: avg_confusion,
            spatial_confusion_weight: spatial_confusion_weight,
            excluded_colors_set: excluded_colors_set,
            color_name_indices: color_name_indices,
        }
    }
}

pub(crate) fn term_loss(
    palette_terms: &[Vec<c3::RelatedTerm>],
    excluded_colors: &HashSet<usize>,
    color_name_indices: &[f32],
) -> f32 {
    let mut loss = 0.0;

    for term in palette_terms {
        for rt in term {
            if excluded_colors.contains(&rt.index) {
                loss += rt.score;
            }
        }
    }

    let any_named = color_name_indices.iter().any(|&x| x >= 0.0);
    if any_named {
        let mut iter = 0;
        for term in palette_terms {
            if iter >= color_name_indices.len() || color_name_indices[iter] < 0.0 {
                iter += 1;
                continue;
            }
            let want = color_name_indices[iter] as usize;
            for rt in term {
                if rt.index == want {
                    loss -= rt.score;
                }
            }
            iter += 1;
        }
    }

    loss as f32
}

/// Additive pieces of the palette objective (same decomposition as [`Loss::cost`]).
#[derive(Clone, Debug)]
pub struct PaletteObjectiveBreakdown {
    /// Value simulated annealing minimizes (sum of all components below).
    pub total: f32,
    /// Negative mean pairwise C3 color-name distance.
    pub minus_mean_color_name_distance: f32,
    /// `−w·min` pairwise C3 name distance (0 unless [`PaletteObjectiveMode::MinName`]).
    pub minus_min_color_name_distance: f32,
    /// Negative minimum display-sRGB distance (÷ [`PERCEPTUAL_SCALE`]).
    pub minus_min_perceptual_distance: f32,
    /// Penalty when any pair is too close in display sRGB.
    pub perceptual_deficit_penalty: f32,
    /// Minimum display-sRGB distance 0–255 (diagnostic).
    pub min_display_rgb_distance: f32,
    /// `-w * min_hue_gap/180` on display-projected OKLab hue.
    pub hue_separation_reward: f32,
    /// Penalty when min hue gap is below the n-dependent target.
    pub hue_separation_deficit: f32,
    /// Minimum circular hue gap in degrees among chromatic display-projected pairs.
    pub min_hue_gap_deg: f32,
    /// C3 term penalties / preferred-name rewards.
    pub term_loss: f32,
    /// `spatial_confusion_weight * confusion_loss` (zero when overlap objective is off).
    pub confusion_weighted: f32,
    /// `-MIN_SAT_REWARD_WEIGHT * min(min OKLab chroma, min sRGB sat)` per channel.
    pub minus_min_saturation: f32,
    /// Penalty when any channel is below OKLab chroma or sRGB saturation floors.
    pub saturation_deficit_penalty: f32,
    /// Minimum linear-sRGB saturation across channels (diagnostic, 0–1).
    pub min_srgb_saturation: f32,
    /// Minimum OKLab chroma across channels (diagnostic).
    pub min_oklab_chroma: f32,
}

/// Recompute the objective for a fixed OKLab palette (same functional as an SA cost evaluation).
pub fn evaluate_palette_objective_breakdown(
    c3: &c3::C3,
    oklab_flat: &[f32],
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_confusion_weight: f32,
    excluded_colors_indices: &[f32],
    color_name_indices: &[f32],
) -> PaletteObjectiveBreakdown {
    let excluded_set: HashSet<usize> = excluded_colors_indices
        .iter()
        .map(|&x| x as usize)
        .collect();
    evaluate_palette_objective_breakdown_with_excluded_set(
        c3,
        oklab_flat,
        intensity_arc,
        avg_confusion,
        spatial_confusion_weight,
        &excluded_set,
        color_name_indices,
    )
}

pub(crate) fn evaluate_palette_objective_breakdown_with_excluded_set(
    c3: &c3::C3,
    oklab_flat: &[f32],
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_confusion_weight: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32],
) -> PaletteObjectiveBreakdown {
    palette_eval::with_eval_scratch(|scratch| {
        palette_eval::evaluate_objective_fast(
            c3,
            oklab_flat,
            intensity_arc,
            avg_confusion,
            spatial_confusion_weight,
            excluded_set,
            color_name_indices,
            scratch,
        )
    })
}

impl CostFunction for Loss {
    type Param = Vec<f32>;
    type Output = f32;

    fn cost(&self, param: &Self::Param) -> Result<Self::Output, Error> {
        let mut clamped = param.clone();
        crate::palette_solvers::clamp_oklab_to_luminance_bounds(
            &mut clamped,
            &self.luminance_values,
        );
        let b = evaluate_palette_objective_breakdown_with_excluded_set(
            &self.c3_instance,
            &clamped,
            &self.intensity_array,
            self.avg_confusion,
            self.spatial_confusion_weight,
            &self.excluded_colors_set,
            &self.color_name_indices,
        );
        Ok(b.total)
    }
}
impl Anneal for Loss {
    type Param = Vec<f32>;
    type Output = Vec<f32>;
    type Float = f32;

    /// Anneal a parameter vector; `extent` is the current SA temperature.
    fn anneal(&self, param: &Vec<f32>, extent: f32) -> Result<Vec<f32>, Error> {
        let mut param_n = param.clone();
        let mut rng = self.rng.lock().unwrap();
        let t_frac = (extent / SA_INITIAL_TEMP).clamp(0.04, 1.0);
        let step_l = 0.08 * t_frac;
        let step_ab = 0.14 * t_frac;
        let n_moves = {
            #[cfg(target_arch = "wasm32")]
            {
                ((extent * 1.0).floor() as u64).clamp(1, 12)
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                ((extent * 1.5).floor() as u64).clamp(1, 24)
            }
        };
        for _ in 0..n_moves {
            for color_idx in 0..param.len() / 3 {
                if self.locked_colors[color_idx] {
                    continue;
                }
                for i in 0..3 {
                    let idx = color_idx * 3 + i;
                    let step = if i == 0 { step_l } else { step_ab };
                    let val = rng.sample(Uniform::new_inclusive(-step, step));
                    param_n[idx] += val;
                    if i == 0 {
                        param_n[idx] =
                            param_n[idx].clamp(self.luminance_values[0], self.luminance_values[1]);
                    } else {
                        param_n[idx] = param_n[idx].clamp(-0.4, 0.4);
                    }
                }
                enforce_channel_saturation(&mut param_n, color_idx, &mut *rng);
            }
        }
        Ok(param_n)
    }
}
pub(crate) fn annealing(
    colors: &[f32],
    locked_colors: &[bool],
    intensity_array: Arc<Array2<f32>>,
    luminance_values: &[f32],
    excluded_colors_indices: &[f32],
    color_name_indices: &[f32],
    c3: Arc<c3::C3>,
    max_iters: u32,
    confusion_baseline_samples: u32,
    init_rng_seed: Option<u64>,
    anneal_rng_seed: Option<u64>,
    mc_rng_seed: Option<u64>,
    include_spatial_channel_overlap: bool,
    precomputed_avg_confusion: Option<f32>,
    start_oklab: Option<&[f32]>,
    sa_initial_temp: Option<f32>,
) -> Result<(Vec<f32>, f32), Error> {
    let temp = sa_initial_temp.unwrap_or(SA_INITIAL_TEMP);
    let solver = build_simulated_annealing_solver(max_iters as u64, temp)?;

    let spatial_w = if include_spatial_channel_overlap {
        SPATIAL_CONFUSION_WEIGHT
    } else {
        0.0
    };

    let average_confusion = match precomputed_avg_confusion {
        Some(v) => v,
        None if spatial_w > 0.0 => calculate_average_confusion(
            luminance_values,
            colors,
            &intensity_array,
            confusion_baseline_samples,
            mc_rng_seed,
        ),
        None => 1.0,
    };
    let start_param = if let Some(start) = start_oklab {
        start.to_vec()
    } else {
        match init_rng_seed {
            Some(seed) => {
                let mut rng = StdRng::seed_from_u64(seed);
                sa_initial_oklab(colors, locked_colors, luminance_values, seed, &mut rng)
            }
            None => {
                let mut rng = thread_rng();
                sa_initial_oklab(colors, locked_colors, luminance_values, 0, &mut rng)
            }
        }
    };

    let cost_function = Loss::new(
        locked_colors.to_vec(),
        Arc::clone(&intensity_array),
        luminance_values.to_vec(),
        average_confusion,
        spatial_w,
        excluded_colors_indices.to_vec(),
        color_name_indices.to_vec(),
        Arc::clone(&c3),
        anneal_rng_seed,
    );
    // Optional: Define temperature function (defaults to `SATempFunc::TemperatureFast`)
    let res = Executor::new(cost_function, solver)
        .configure(|state| state.param(start_param).max_iters(max_iters as u64))
        // Optional: Attach an observer
        .run()?;
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    {
        console::log_1(&format!("loss: {:?}", res.state().best_cost).into());
        let best_param_dbg = res.state().get_best_param().unwrap().clone();
        console::log_1(&format!("best_param: {:?}", best_param_dbg).into());
    }
    let mut best_param = res.state().get_best_param().unwrap().clone();
    crate::palette_solvers::clamp_oklab_to_luminance_bounds(&mut best_param, luminance_values);
    let excluded_set: HashSet<usize> = excluded_colors_indices
        .iter()
        .map(|&x| x as usize)
        .collect();
    let best_cost = evaluate_palette_objective_breakdown_with_excluded_set(
        c3.as_ref(),
        &best_param,
        &intensity_array,
        average_confusion,
        spatial_w,
        &excluded_set,
        color_name_indices,
    )
    .total;

    Ok((best_param, best_cost)) // Return the best parameters + final cost
}

/// Full optimize path (native tooling + examples). WASM [`optimize`] wraps this.
pub struct OptimizePipelineResult {
    pub srgb_linear: Vec<f32>,
    /// Last SA state cost (Argmin); compare to [`PaletteObjectiveBreakdown::total`] from
    /// [`evaluate_palette_objective_breakdown`] on `oklab_best` — large gaps suggest optimizer issues.
    pub sa_best_cost: f32,
    pub oklab_best: Vec<f32>,
    pub intensity_arc: Arc<Array2<f32>>,
    pub excluded_colors_indices: Vec<f32>,
    pub color_name_indices: Vec<f32>,
    /// All multistart (+ rescue) outcomes with diagnostics for study selection.
    /// Empty when pool capture is skipped. Production selection still uses total-loss fold.
    pub restart_pool: Vec<RestartRecord>,
}

struct NmRestartOutcome {
    oklab: Vec<f32>,
    total: f32,
    min_display_rgb_distance: f32,
    solver_cost: f32,
}

/// Shared inputs for NM multistart / finalize (native + WASM worker orchestration).
struct PaletteOptContext {
    n_channels: usize,
    locked_colors_vec: Vec<bool>,
    float_luminance_values: Vec<f32>,
    oklab_color_map: Vec<f32>,
    intensity_arc: Arc<Array2<f32>>,
    excluded_colors_indices: Vec<f32>,
    color_name_indices: Vec<f32>,
    c3_eval: Arc<c3::C3>,
    base_seed: u64,
    spatial_w: f32,
    avg_confusion: f32,
    excluded_set: HashSet<usize>,
    max_iters: u32,
    confusion_baseline_samples: u32,
    include_overlap: bool,
    nm_params: PaletteSolverParams,
    polish_each_restart: bool,
}

#[allow(clippy::too_many_arguments)]
fn build_palette_opt_context(
    colors: &[u16],
    locked_colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
    max_iters: Option<u32>,
    confusion_baseline_samples: Option<u32>,
    include_spatial_channel_overlap: Option<bool>,
) -> PaletteOptContext {
    let n_channels = colors.len() / 3;
    let max_iters = scale_budget_for_channels(
        max_iters.unwrap_or_else(default_max_iters).max(1),
        n_channels,
    );
    let confusion_baseline_samples = confusion_baseline_samples
        .unwrap_or_else(default_confusion_baseline_samples)
        .max(1);
    let include_overlap =
        include_spatial_channel_overlap.unwrap_or_else(default_include_spatial_overlap);

    let float_luminance_values: Vec<f32> = luminance_values
        .iter()
        .map(|&x| (x as f32) / 100.0)
        .collect();
    let float_color_map: Vec<f32> = colors.iter().map(|&x| (x as f32) / 255.0).collect();
    let locked_colors_vec: Vec<bool> = locked_colors.iter().map(|&x| x == 1).collect();
    let oklab_color_map: Vec<f32> = float_color_map
        .chunks(3)
        .map(|color| {
            let rgb = Srgb::new(color[0], color[1], color[2]);
            let oklab: Oklab = Oklab::from_color(rgb);
            vec![oklab.l, oklab.a, oklab.b]
        })
        .flatten()
        .collect();
    let intensity_array = preprocess_data(colors, intensities, contrast_limits);
    let excluded_colors = merge_excluded_color_names(excluded_colors);
    let c3_eval = Arc::new(c3::C3::new());
    let mut excluded_colors_indices = Vec::new();
    for color in excluded_colors {
        if let Some(index) = c3_eval.get_term_index(&color) {
            excluded_colors_indices.push(index as f32);
        }
    }
    let mut color_name_indices = Vec::new();
    for color in color_names {
        if color.is_empty() {
            color_name_indices.push(-1.0f32);
            continue;
        }
        if let Some(index) = c3_eval.get_term_index(&color) {
            color_name_indices.push(index as f32);
        }
    }
    let intensity_arc = Arc::new(intensity_array);
    let base_seed = problem_seed(colors, intensities, contrast_limits, luminance_values);
    let spatial_w = if include_overlap {
        SPATIAL_CONFUSION_WEIGHT
    } else {
        0.0
    };
    let avg_confusion = if spatial_w > 0.0 {
        calculate_average_confusion(
            &float_luminance_values,
            &oklab_color_map,
            &intensity_arc,
            confusion_baseline_samples,
            Some(base_seed.wrapping_add(0xA5A5_5A5A_5A5A_5A5A)),
        )
    } else {
        1.0
    };
    let excluded_set: HashSet<usize> = excluded_colors_indices
        .iter()
        .map(|&x| x as usize)
        .collect();
    let nm_perturb_scale = nm_perturb_scale_for_channels(n_channels);
    let nm_params = PaletteSolverParams {
        argmin_max_iters: Some(scaled_solver_iters(
            PaletteArgminSolver::NelderMead,
            max_iters,
        )),
        nm_perturb_scale,
        ..PaletteSolverParams::default()
    };
    PaletteOptContext {
        n_channels,
        locked_colors_vec,
        float_luminance_values,
        oklab_color_map,
        intensity_arc,
        excluded_colors_indices,
        color_name_indices,
        c3_eval,
        base_seed,
        spatial_w,
        avg_confusion,
        excluded_set,
        max_iters,
        confusion_baseline_samples,
        include_overlap,
        nm_params,
        polish_each_restart: true,
    }
}

/// One NM restart result for WASM worker pools (`run_nm_restart`).
#[wasm_bindgen]
pub struct NmRestartWasmResult {
    oklab: Vec<f32>,
    total: f32,
    min_display_rgb_distance: f32,
}

#[wasm_bindgen]
impl NmRestartWasmResult {
    #[wasm_bindgen(getter)]
    pub fn oklab(&self) -> Vec<f32> {
        self.oklab.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn total(&self) -> f32 {
        self.total
    }

    #[wasm_bindgen(getter)]
    pub fn min_display_rgb_distance(&self) -> f32 {
        self.min_display_rgb_distance
    }
}

/// Run a single Nelder–Mead multistart (for parallel Web Workers). Matches one leg of [`optimize_palette_pipeline`].
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run_nm_restart(
    colors: &[u16],
    locked_colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
    max_iters: Option<u32>,
    confusion_baseline_samples: Option<u32>,
    include_spatial_channel_overlap: Option<bool>,
    restart_index: u32,
    seed_salt: u32,
    rescue_random_init: bool,
) -> NmRestartWasmResult {
    utils::set_panic_hook();
    let mut ctx = build_palette_opt_context(
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
    );
    if rescue_random_init {
        ctx.nm_params.force_random_nm_init = true;
        ctx.nm_params.nm_perturb_scale = ctx.nm_params.nm_perturb_scale.max(1.35);
    }
    let o = run_nm_multistart_attempt(
        restart_index,
        ctx.base_seed,
        seed_salt as u64,
        &ctx.oklab_color_map,
        &ctx.locked_colors_vec,
        &ctx.intensity_arc,
        &ctx.float_luminance_values,
        &ctx.excluded_colors_indices,
        &ctx.color_name_indices,
        &ctx.c3_eval,
        ctx.max_iters,
        ctx.confusion_baseline_samples,
        ctx.include_overlap,
        ctx.avg_confusion,
        ctx.spatial_w,
        &ctx.excluded_set,
        &ctx.nm_params,
        ctx.polish_each_restart,
    );
    NmRestartWasmResult {
        oklab: o.oklab,
        total: o.total,
        min_display_rgb_distance: o.min_display_rgb_distance,
    }
}

/// Polish + refine best OKLab and return linear sRGB (after parallel restarts).
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn finalize_palette_optimize(
    colors: &[u16],
    locked_colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
    max_iters: Option<u32>,
    confusion_baseline_samples: Option<u32>,
    include_spatial_channel_overlap: Option<bool>,
    oklab_best: Vec<f32>,
) -> Vec<f32> {
    utils::set_panic_hook();
    let mut ctx = build_palette_opt_context(
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
    );
    let mut best_oklab = oklab_best;
    polish_oklab_palette(
        &mut best_oklab,
        &ctx.locked_colors_vec,
        &ctx.float_luminance_values,
        &ctx.c3_eval,
        &ctx.intensity_arc,
        ctx.avg_confusion,
        ctx.spatial_w,
        &ctx.excluded_set,
        &ctx.color_name_indices,
        false,
    );
    refine_oklab_palette(
        &mut best_oklab,
        &ctx.locked_colors_vec,
        &ctx.float_luminance_values,
        &ctx.c3_eval,
        &ctx.intensity_arc,
        ctx.avg_confusion,
        ctx.spatial_w,
        &ctx.excluded_set,
        &ctx.color_name_indices,
        ctx.base_seed.wrapping_add(0xA11CE),
    );
    best_oklab
        .chunks(3)
        .map(|color| {
            let okl = Oklab::new(color[0], color[1], color[2]);
            let rgb: Srgb = Srgb::from_color(okl);
            vec![
                rgb.red.clamp(0.0, 1.0),
                rgb.green.clamp(0.0, 1.0),
                rgb.blue.clamp(0.0, 1.0),
            ]
        })
        .flatten()
        .collect()
}

/// One Nelder–Mead multistart (+ optional per-restart polish and total re-eval).
fn run_nm_multistart_attempt(
    restart: u32,
    base_seed: u64,
    seed_salt: u64,
    start_oklab: &[f32],
    locked_colors: &[bool],
    intensity_arc: &Arc<Array2<f32>>,
    luminance_values: &[f32],
    excluded_colors_indices: &[f32],
    color_name_indices: &[f32],
    c3_eval: &Arc<c3::C3>,
    max_iters: u32,
    confusion_baseline_samples: u32,
    include_overlap: bool,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: &HashSet<usize>,
    nm_params: &PaletteSolverParams,
    polish_each_restart: bool,
) -> NmRestartOutcome {
    let init_seed = base_seed
        .wrapping_add(seed_salt)
        .wrapping_add((restart as u64).wrapping_mul(RESTART_SEED_STRIDE));
    let solver_seed = init_seed.wrapping_add(0x517C_C1B0_2722_0A95);
    let (mut candidate, solver_cost) = match run_palette_argmin_solver(
        PaletteArgminSolver::NelderMead,
        start_oklab,
        locked_colors,
        Arc::clone(intensity_arc),
        luminance_values,
        excluded_colors_indices,
        color_name_indices,
        Arc::clone(c3_eval),
        max_iters,
        init_seed,
        solver_seed,
        confusion_baseline_samples,
        include_overlap,
        Some(avg_confusion),
        nm_params,
        restart,
    ) {
        Ok(v) => v,
        Err(err) => {
            // Argmin NelderMead can error on degenerate simplex / NaN costs; keep searching.
            eprintln!("[psudo] nelder-mead restart {restart} failed: {err}; using init");
            let mut rng = StdRng::seed_from_u64(init_seed);
            let fallback = if nm_params.force_random_nm_init {
                random_initial_oklab(start_oklab, locked_colors, luminance_values, &mut rng)
            } else {
                sa_initial_oklab_for_restart(
                    start_oklab,
                    locked_colors,
                    luminance_values,
                    init_seed,
                    restart,
                    &mut rng,
                )
            };
            let cost = evaluate_palette_objective_breakdown_with_excluded_set(
                c3_eval,
                &fallback,
                intensity_arc,
                avg_confusion,
                spatial_w,
                excluded_set,
                color_name_indices,
            )
            .total;
            (fallback, cost)
        }
    };

    if polish_each_restart {
        polish_oklab_palette(
            &mut candidate,
            locked_colors,
            luminance_values,
            c3_eval,
            intensity_arc,
            avg_confusion,
            spatial_w,
            excluded_set,
            color_name_indices,
            false,
        );
    }
    let bd = evaluate_palette_objective_breakdown_with_excluded_set(
        c3_eval,
        &candidate,
        intensity_arc,
        avg_confusion,
        spatial_w,
        excluded_set,
        color_name_indices,
    );
    NmRestartOutcome {
        oklab: candidate,
        total: bd.total,
        min_display_rgb_distance: bd.min_display_rgb_distance,
        solver_cost,
    }
}

/// Native: parallel multistarts via rayon. WASM: sequential (single worker thread).
fn nm_multistart_outcomes(
    num_restarts: u32,
    base_seed: u64,
    seed_salt: u64,
    start_oklab: &[f32],
    locked_colors: &[bool],
    intensity_arc: Arc<Array2<f32>>,
    luminance_values: &[f32],
    excluded_colors_indices: &[f32],
    color_name_indices: &[f32],
    c3_eval: Arc<c3::C3>,
    max_iters: u32,
    confusion_baseline_samples: u32,
    include_overlap: bool,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: HashSet<usize>,
    nm_params: PaletteSolverParams,
    polish_each_restart: bool,
) -> Vec<NmRestartOutcome> {
    let obj_mode = crate::current_objective_mode();
    let run = |restart: u32| {
        // Rayon workers do not inherit the parent's thread-local objective mode.
        crate::with_objective_mode(obj_mode, || {
            run_nm_multistart_attempt(
                restart,
                base_seed,
                seed_salt,
                start_oklab,
                locked_colors,
                &intensity_arc,
                luminance_values,
                excluded_colors_indices,
                color_name_indices,
                &c3_eval,
                max_iters,
                confusion_baseline_samples,
                include_overlap,
                avg_confusion,
                spatial_w,
                &excluded_set,
                &nm_params,
                polish_each_restart,
            )
        })
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        return (0..num_restarts).into_par_iter().map(run).collect();
    }
    #[cfg(target_arch = "wasm32")]
    {
        return (0..num_restarts).map(run).collect();
    }
}

fn fold_best_nm_restart(outcomes: &[NmRestartOutcome]) -> (Vec<f32>, f32, f32) {
    let mut best_oklab = Vec::new();
    let mut best_total = f32::INFINITY;
    let mut best_solver_cost = f32::INFINITY;
    for o in outcomes {
        if o.total < best_total {
            best_total = o.total;
            best_oklab = o.oklab.clone();
            best_solver_cost = o.solver_cost;
        }
    }
    (best_oklab, best_total, best_solver_cost)
}

fn build_restart_pool(
    c3: &c3::C3,
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32],
    waves: &[(u32, &[NmRestartOutcome])],
) -> Vec<RestartRecord> {
    let mut pool = Vec::new();
    let mut next_id = 0u32;
    for &(salt_tag, outcomes) in waves {
        let _ = salt_tag;
        for o in outcomes {
            let bd = evaluate_palette_objective_breakdown_with_excluded_set(
                c3,
                &o.oklab,
                intensity_arc,
                avg_confusion,
                spatial_w,
                excluded_set,
                color_name_indices,
            );
            let diagnostics = compute_diagnostics(c3, &o.oklab, &bd);
            pool.push(RestartRecord {
                restart_id: next_id,
                oklab: o.oklab.clone(),
                diagnostics,
            });
            next_id = next_id.wrapping_add(1);
        }
    }
    pool
}

pub fn optimize_palette_pipeline(
    colors: &[u16],
    locked_colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
    max_iters: Option<u32>,
    confusion_baseline_samples: Option<u32>,
    include_spatial_channel_overlap: Option<bool>,
    num_restarts: Option<u32>,
    postprocess: Option<OptimizePostprocess>,
) -> OptimizePipelineResult {
    optimize_palette_pipeline_with_init(
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
        postprocess,
        PaletteInitMode::Current,
        PaletteRefineMode::Cartesian,
        PaletteObjectiveMode::MeanOnly,
    )
}

/// Like [`optimize_palette_pipeline`], with optional Glasbey init, refine mode, and objective mode.
pub fn optimize_palette_pipeline_with_init(
    colors: &[u16],
    locked_colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
    max_iters: Option<u32>,
    confusion_baseline_samples: Option<u32>,
    include_spatial_channel_overlap: Option<bool>,
    num_restarts: Option<u32>,
    postprocess: Option<OptimizePostprocess>,
    init_mode: PaletteInitMode,
    refine_mode: PaletteRefineMode,
    objective_mode: PaletteObjectiveMode,
) -> OptimizePipelineResult {
    with_objective_mode(objective_mode, || {
        optimize_palette_pipeline_with_init_inner(
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
            postprocess,
            init_mode,
            refine_mode,
        )
    })
}

fn optimize_palette_pipeline_with_init_inner(
    colors: &[u16],
    locked_colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
    max_iters: Option<u32>,
    confusion_baseline_samples: Option<u32>,
    include_spatial_channel_overlap: Option<bool>,
    num_restarts: Option<u32>,
    postprocess: Option<OptimizePostprocess>,
    init_mode: PaletteInitMode,
    refine_mode: PaletteRefineMode,
) -> OptimizePipelineResult {
    let postprocess = postprocess.unwrap_or(OptimizePostprocess::Full);
    let (restart_min, restart_max) = restart_count_bounds();
    let mut ctx = build_palette_opt_context(
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
    );
    let num_restarts = scale_budget_for_channels(
        num_restarts.unwrap_or_else(default_num_restarts).max(1),
        ctx.n_channels,
    )
    .clamp(restart_min, restart_max);
    let full_post = pipeline_use_full_postprocess();
    let polish_fast = !full_post;
    // Study path: still polish each restart so restart_pool candidates are comparable.
    let polish_each_restart = full_post
        && matches!(
            postprocess,
            OptimizePostprocess::Full | OptimizePostprocess::Study
        );
    ctx.polish_each_restart = polish_each_restart;

    apply_init_mode_to_nm_params(&mut ctx, init_mode);

    let outcomes = nm_multistart_outcomes(
        num_restarts,
        ctx.base_seed,
        0,
        &ctx.oklab_color_map,
        &ctx.locked_colors_vec,
        Arc::clone(&ctx.intensity_arc),
        &ctx.float_luminance_values,
        &ctx.excluded_colors_indices,
        &ctx.color_name_indices,
        Arc::clone(&ctx.c3_eval),
        ctx.max_iters,
        ctx.confusion_baseline_samples,
        ctx.include_overlap,
        ctx.avg_confusion,
        ctx.spatial_w,
        ctx.excluded_set.clone(),
        ctx.nm_params.clone(),
        polish_each_restart,
    );
    let (l_tot_rescue, rgb_rescue) = adaptive_rescue_band(&outcomes);
    let soft_rgb = MIN_DISPLAY_RGB_DISTANCE as f32 * 0.9;
    let wave_best_min_rgb = outcomes
        .iter()
        .min_by(|a, b| {
            a.total
                .partial_cmp(&b.total)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|o| o.min_display_rgb_distance)
        .unwrap_or(0.0);
    let (mut best_oklab, mut best_total, mut best_solver_cost) = fold_best_nm_restart(&outcomes);

    let mut rescue_waves: Vec<Vec<NmRestartOutcome>> = Vec::new();
    if ctx.n_channels >= 6 {
        let rescue_restarts =
            scale_budget_for_channels(HIGH_CH_RESCUE_RESTARTS_BASE, ctx.n_channels).max(4);
        let mut prefer_random = wave_best_min_rgb < soft_rgb;
        for wave in 0..2u32 {
            let check = evaluate_palette_objective_breakdown_with_excluded_set(
                &ctx.c3_eval,
                &best_oklab,
                &ctx.intensity_arc,
                ctx.avg_confusion,
                ctx.spatial_w,
                &ctx.excluded_set,
                &ctx.color_name_indices,
            );
            if !outside_adaptive_band(
                check.total,
                check.min_display_rgb_distance,
                l_tot_rescue,
                rgb_rescue,
            ) {
                break;
            }
            let mut rescue_params = ctx.nm_params.clone();
            rescue_params.force_random_nm_init = prefer_random;
            // Rescue keeps exploring; drop seed override so waves stay diverse.
            rescue_params.seed_oklab_override = None;
            rescue_params.seed_override_mode = SeedOverrideMode::None;
            prefer_random = !prefer_random;
            rescue_params.nm_perturb_scale = rescue_params
                .nm_perturb_scale
                .max(nm_perturb_scale_for_channels(ctx.n_channels) + 0.15);
            let salt = if wave == 0 {
                RESCUE_SEED_SALT_1
            } else {
                RESCUE_SEED_SALT_2
            };
            let rescue_outcomes = nm_multistart_outcomes(
                rescue_restarts,
                ctx.base_seed,
                salt,
                &ctx.oklab_color_map,
                &ctx.locked_colors_vec,
                Arc::clone(&ctx.intensity_arc),
                &ctx.float_luminance_values,
                &ctx.excluded_colors_indices,
                &ctx.color_name_indices,
                Arc::clone(&ctx.c3_eval),
                ctx.max_iters,
                ctx.confusion_baseline_samples,
                ctx.include_overlap,
                ctx.avg_confusion,
                ctx.spatial_w,
                ctx.excluded_set.clone(),
                rescue_params,
                polish_each_restart,
            );
            let (r_oklab, r_total, r_cost) = fold_best_nm_restart(&rescue_outcomes);
            if r_total < best_total {
                best_oklab = r_oklab;
                best_total = r_total;
                best_solver_cost = r_cost;
            }
            rescue_waves.push(rescue_outcomes);
        }
    }

    let mut wave_refs: Vec<(u32, &[NmRestartOutcome])> = vec![(0, outcomes.as_slice())];
    for (i, w) in rescue_waves.iter().enumerate() {
        wave_refs.push(((i as u32) + 1, w.as_slice()));
    }
    let restart_pool = build_restart_pool(
        &ctx.c3_eval,
        &ctx.intensity_arc,
        ctx.avg_confusion,
        ctx.spatial_w,
        &ctx.excluded_set,
        &ctx.color_name_indices,
        &wave_refs,
    );

    if full_post && postprocess == OptimizePostprocess::Study {
        // Study path: polish + selected refine mode (cartesian default).
        apply_palette_refine(
            &mut best_oklab,
            &ctx.locked_colors_vec,
            &ctx.float_luminance_values,
            &ctx.c3_eval,
            &ctx.intensity_arc,
            ctx.avg_confusion,
            ctx.spatial_w,
            &ctx.excluded_set,
            &ctx.color_name_indices,
            ctx.base_seed.wrapping_add(0xA11CE),
            refine_mode,
            true,
            false,
        );
    } else {
        let do_full_refine = full_post;
        if do_full_refine {
            apply_palette_refine(
                &mut best_oklab,
                &ctx.locked_colors_vec,
                &ctx.float_luminance_values,
                &ctx.c3_eval,
                &ctx.intensity_arc,
                ctx.avg_confusion,
                ctx.spatial_w,
                &ctx.excluded_set,
                &ctx.color_name_indices,
                ctx.base_seed.wrapping_add(0xA11CE),
                refine_mode,
                true,
                polish_fast,
            );
        } else {
            polish_oklab_palette(
                &mut best_oklab,
                &ctx.locked_colors_vec,
                &ctx.float_luminance_values,
                &ctx.c3_eval,
                &ctx.intensity_arc,
                ctx.avg_confusion,
                ctx.spatial_w,
                &ctx.excluded_set,
                &ctx.color_name_indices,
                polish_fast,
            );
        }
    }
    best_total = evaluate_palette_objective_breakdown_with_excluded_set(
        &ctx.c3_eval,
        &best_oklab,
        &ctx.intensity_arc,
        ctx.avg_confusion,
        ctx.spatial_w,
        &ctx.excluded_set,
        &ctx.color_name_indices,
    )
    .total;
    let _ = best_total;

    // Final hard projection: NM reflections can leave L below the API luminance floor.
    crate::palette_solvers::clamp_oklab_to_luminance_bounds(
        &mut best_oklab,
        &ctx.float_luminance_values,
    );

    let optimized_oklab = best_oklab;
    let sa_best_cost = best_solver_cost;

    let srgb_linear = optimized_oklab
        .chunks(3)
        .map(|color| {
            let okl = Oklab::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let rgb: Srgb = Srgb::from_color(okl);
            vec![
                rgb.red.clamp(0.0, 1.0),
                rgb.green.clamp(0.0, 1.0),
                rgb.blue.clamp(0.0, 1.0),
            ]
        })
        .flatten()
        .collect::<Vec<f32>>();

    OptimizePipelineResult {
        srgb_linear,
        sa_best_cost,
        oklab_best: optimized_oklab,
        intensity_arc: ctx.intensity_arc,
        excluded_colors_indices: ctx.excluded_colors_indices,
        color_name_indices: ctx.color_name_indices,
        restart_pool,
    }
}

fn apply_init_mode_to_nm_params(ctx: &mut PaletteOptContext, init_mode: PaletteInitMode) {
    match init_mode {
        PaletteInitMode::Current => {}
        PaletteInitMode::GlasbeyV1 | PaletteInitMode::Mixed => {
            let cands =
                generate_feasible_seed_candidates(&ctx.c3_eval, &ctx.float_luminance_values);
            let seed = glasbey_like_seed_palette(
                ctx.n_channels,
                &cands,
                &ctx.c3_eval,
                &GlasbeyDistanceWeights::default(),
            );
            ctx.nm_params.seed_oklab_override = Some(seed);
            ctx.nm_params.seed_override_mode = match init_mode {
                PaletteInitMode::GlasbeyV1 => SeedOverrideMode::Restart0AndJitter,
                PaletteInitMode::Mixed => SeedOverrideMode::ExactRestart(1),
                PaletteInitMode::Current => SeedOverrideMode::None,
            };
        }
    }
}

/// Alternate global-search method + Study finish (benchmarks only; app uses [`optimize_palette_pipeline`]).
#[allow(clippy::too_many_arguments)]
pub fn optimize_palette_with_solver(
    colors: &[u16],
    locked_colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
    max_iters: Option<u32>,
    confusion_baseline_samples: Option<u32>,
    include_spatial_channel_overlap: Option<bool>,
    num_restarts: Option<u32>,
    solver: PaletteArgminSolver,
    solver_params: Option<PaletteSolverParams>,
) -> OptimizePipelineResult {
    use palette_solvers::{run_palette_argmin_solver, study_postprocess_oklab};

    let params = solver_params.unwrap_or_default();
    let n_channels = colors.len() / 3;
    let max_iters = scale_budget_for_channels(
        max_iters.unwrap_or_else(default_max_iters).max(1),
        n_channels,
    );
    let confusion_baseline_samples = confusion_baseline_samples
        .unwrap_or_else(default_confusion_baseline_samples)
        .max(1);
    let include_overlap =
        include_spatial_channel_overlap.unwrap_or_else(default_include_spatial_overlap);
    let num_restarts = scale_budget_for_channels(
        num_restarts.unwrap_or_else(default_num_restarts).max(1),
        n_channels,
    )
    .clamp(
        1,
        if solver == PaletteArgminSolver::SimulatedAnnealing {
            restart_count_bounds().1
        } else {
            8
        },
    );

    let float_luminance_values: Vec<f32> = luminance_values
        .iter()
        .map(|&x| (x as f32) / 100.0)
        .collect();
    let float_color_map: Vec<f32> = colors.iter().map(|&x| (x as f32) / 255.0).collect();
    let locked_colors_vec: Vec<bool> = locked_colors.iter().map(|&x| x == 1).collect();
    let oklab_color_map: Vec<f32> = float_color_map
        .chunks(3)
        .map(|color| {
            let rgb = Srgb::new(color[0], color[1], color[2]);
            let oklab: Oklab = Oklab::from_color(rgb);
            vec![oklab.l, oklab.a, oklab.b]
        })
        .flatten()
        .collect();
    let intensity_array = preprocess_data(colors, intensities, contrast_limits);
    let excluded_colors = merge_excluded_color_names(excluded_colors);
    let c3_eval = Arc::new(c3::C3::new());
    let mut excluded_colors_indices = Vec::new();
    for color in &excluded_colors {
        if let Some(index) = c3_eval.get_term_index(color) {
            excluded_colors_indices.push(index as f32);
        }
    }
    let mut color_name_indices = Vec::new();
    for color in color_names {
        if color.is_empty() {
            color_name_indices.push(-1.0f32);
            continue;
        }
        if let Some(index) = c3_eval.get_term_index(&color) {
            color_name_indices.push(index as f32);
        }
    }
    let intensity_arc = Arc::new(intensity_array);
    let base_seed = problem_seed(colors, intensities, contrast_limits, luminance_values);
    let spatial_w = if include_overlap {
        SPATIAL_CONFUSION_WEIGHT
    } else {
        0.0
    };
    let avg_confusion = if spatial_w > 0.0 {
        calculate_average_confusion(
            &float_luminance_values,
            &oklab_color_map,
            &intensity_arc,
            confusion_baseline_samples,
            Some(base_seed.wrapping_add(0xA5A5_5A5A_5A5A_5A5A)),
        )
    } else {
        1.0
    };
    let excluded_set: HashSet<usize> = excluded_colors_indices
        .iter()
        .map(|&x| x as usize)
        .collect();

    let mut best_oklab = oklab_color_map.clone();
    let mut best_total = f32::INFINITY;
    let mut solver_cost = f32::INFINITY;

    if solver == PaletteArgminSolver::PolishOnly {
        best_oklab = oklab_color_map.clone();
    } else {
        for restart in 0..num_restarts {
            let init_seed =
                base_seed.wrapping_add((restart as u64).wrapping_mul(RESTART_SEED_STRIDE));
            let anneal_seed = init_seed.wrapping_add(0x517C_C1B0_2722_0A95);
            let (candidate, cost) = run_palette_argmin_solver(
                solver,
                &oklab_color_map,
                &locked_colors_vec,
                Arc::clone(&intensity_arc),
                &float_luminance_values,
                &excluded_colors_indices,
                &color_name_indices,
                Arc::clone(&c3_eval),
                max_iters,
                init_seed,
                anneal_seed,
                confusion_baseline_samples,
                include_overlap,
                Some(avg_confusion),
                &params,
                restart,
            )
            .expect("solver run");
            let total = evaluate_palette_objective_breakdown_with_excluded_set(
                &c3_eval,
                &candidate,
                &intensity_arc,
                avg_confusion,
                spatial_w,
                &excluded_set,
                &color_name_indices,
            )
            .total;
            if total < best_total {
                best_total = total;
                best_oklab = candidate;
                solver_cost = cost;
            }
        }
    }

    study_postprocess_oklab(
        &mut best_oklab,
        &locked_colors_vec,
        &float_luminance_values,
        &c3_eval,
        &intensity_arc,
        avg_confusion,
        spatial_w,
        &excluded_set,
        &color_name_indices,
        base_seed,
    );
    best_total = evaluate_palette_objective_breakdown_with_excluded_set(
        &c3_eval,
        &best_oklab,
        &intensity_arc,
        avg_confusion,
        spatial_w,
        &excluded_set,
        &color_name_indices,
    )
    .total;

    let srgb_linear = best_oklab
        .chunks(3)
        .map(|color| {
            let okl = Oklab::new(color[0], color[1], color[2]);
            let rgb: Srgb = Srgb::from_color(okl);
            vec![
                rgb.red.clamp(0.0, 1.0),
                rgb.green.clamp(0.0, 1.0),
                rgb.blue.clamp(0.0, 1.0),
            ]
        })
        .flatten()
        .collect();

    OptimizePipelineResult {
        srgb_linear,
        sa_best_cost: solver_cost,
        oklab_best: best_oklab,
        intensity_arc,
        excluded_colors_indices,
        color_name_indices,
        restart_pool: Vec::new(),
    }
}

/// `include_spatial_channel_overlap`: `None` or `true` = full objective including per-pixel
/// multi-channel confusion; `false` = name + OKLab separation + terms only (round‑1 eval).
///
/// `num_restarts`: independent Nelder–Mead multistarts (default 18, scaled by channel count); best total wins.
///
/// Same objective as native `palette_study` (`evaluate_palette_objective_breakdown` on `oklab_best`).
fn pipeline_study_breakdown(
    run: &OptimizePipelineResult,
    include_spatial_channel_overlap: Option<bool>,
) -> PaletteObjectiveBreakdown {
    let include_overlap =
        include_spatial_channel_overlap.unwrap_or_else(default_include_spatial_overlap);
    let spatial_w = if include_overlap {
        SPATIAL_CONFUSION_WEIGHT
    } else {
        0.0
    };
    let c3_eval = c3::C3::new();
    evaluate_palette_objective_breakdown(
        &c3_eval,
        &run.oklab_best,
        &run.intensity_arc,
        1.0,
        spatial_w,
        &run.excluded_colors_indices,
        &run.color_name_indices,
    )
}

/// WASM / JS study reports: linear sRGB plus `L_tot` / `min_rgb` matching native `palette_study`.
#[wasm_bindgen]
pub struct OptimizeMetricsResult {
    srgb_linear: Vec<f32>,
    l_tot: f32,
    min_display_rgb_distance: f32,
}

#[wasm_bindgen]
impl OptimizeMetricsResult {
    #[wasm_bindgen(getter)]
    pub fn srgb_linear(&self) -> Vec<f32> {
        self.srgb_linear.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn l_tot(&self) -> f32 {
        self.l_tot
    }

    #[wasm_bindgen(getter)]
    pub fn min_display_rgb_distance(&self) -> f32 {
        self.min_display_rgb_distance
    }
}

/// Defaults match `palette_study`: `max_iters` 3000, `confusion_baseline_samples` 32,
/// `num_restarts` 18 (scaled × n/3, max 40), spatial overlap off, full polish + refine.
#[wasm_bindgen]
pub fn optimize(
    colors: &[u16],
    locked_colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
    max_iters: Option<u32>,
    confusion_baseline_samples: Option<u32>,
    include_spatial_channel_overlap: Option<bool>,
    num_restarts: Option<u32>,
) -> Vec<f32> {
    optimize_with_metrics(
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
    )
    .srgb_linear
}

#[wasm_bindgen]
pub fn optimize_with_metrics(
    colors: &[u16],
    locked_colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
    max_iters: Option<u32>,
    confusion_baseline_samples: Option<u32>,
    include_spatial_channel_overlap: Option<bool>,
    num_restarts: Option<u32>,
) -> OptimizeMetricsResult {
    utils::set_panic_hook();
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    let now = instant::Instant::now();

    let r = optimize_palette_pipeline(
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
        None,
    );
    let bd = pipeline_study_breakdown(&r, include_spatial_channel_overlap);

    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    {
        let elapsed = now.elapsed();
        console::log_1(
            &format!(
                "optimize_with_metrics ({} restarts) L_tot={:.4} took: {:?}",
                num_restarts.unwrap_or_else(default_num_restarts),
                bd.total,
                elapsed
            )
            .into(),
        );
    }

    OptimizeMetricsResult {
        srgb_linear: r.srgb_linear,
        l_tot: bd.total,
        min_display_rgb_distance: bd.min_display_rgb_distance,
    }
}

fn color_only_loss(
    param: &Vec<f32>,
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
) -> Result<HashMap<String, f32>, Error> {
    let excluded_colors = merge_excluded_color_names(excluded_colors);
    let oklab_colors = param;
    let c3_instance = c3::C3::new();

    let mut excluded_colors_indices = Vec::new();
    for color in excluded_colors {
        let analyzed_color = c3_instance.get_term_index(&color);
        // if color is some, push
        if let Some(index) = analyzed_color {
            excluded_colors_indices.push(index as f32);
        }
    }
    let mut color_name_indices = Vec::new();

    for color in color_names {
        if color == "" {
            color_name_indices.push(-1 as f32);
            continue;
        }
        let analyzed_color = c3_instance.get_term_index(&color);
        // if color is some, push
        if let Some(index) = analyzed_color {
            color_name_indices.push(index as f32);
        }
    }

    let excluded_set: HashSet<usize> = excluded_colors_indices
        .iter()
        .map(|&x| x as usize)
        .collect();

    let (
        average_cosine_distance,
        term_loss_val,
        minus_min_perceptual,
        perceptual_deficit_penalty,
        min_display_rgb_distance,
    ) = palette_eval::with_eval_scratch(|scratch| {
        palette_eval::fill_labs_from_oklab(oklab_colors, &mut scratch.labs);
        palette_eval::fill_c3_labs_from_oklab(oklab_colors, &mut scratch.c3_labs);
        palette_eval::fill_display_srgb255(oklab_colors, &mut scratch.display_rgb);
        c3_instance.fill_palette_c3(
            &scratch.c3_labs,
            10,
            &mut scratch.c3_samples,
            &mut scratch.palette_terms,
        );
        let avg = c3_instance.average_pairwise_color_name_distance(&scratch.c3_samples);
        let term = term_loss(&scratch.palette_terms, &excluded_set, &color_name_indices);
        let (mp, pd, md) =
            palette_eval::perceptual_objective_terms_from_display(&scratch.display_rgb);
        (avg, term, mp, pd, md)
    });
    let (hue_reward, hue_deficit, min_hue_gap) =
        hue_separation_terms(param, HUE_SEPARATION_WEIGHT);
    let (minus_min_saturation, saturation_deficit_penalty, min_srgb_saturation, min_oklab_chroma) =
        saturation_objective_terms(param);

    let mut loss_components = HashMap::new();
    loss_components.insert("name_distance".to_string(), -average_cosine_distance as f32);
    loss_components.insert("perceptual_distance".to_string(), minus_min_perceptual);
    loss_components.insert("perceptural_distance".to_string(), minus_min_perceptual);
    loss_components.insert("perceptual_deficit".to_string(), perceptual_deficit_penalty);
    loss_components.insert(
        "min_display_rgb_distance".to_string(),
        min_display_rgb_distance,
    );
    loss_components.insert(
        "min_perceptual_de2000".to_string(),
        min_display_rgb_distance,
    );
    loss_components.insert("hue_separation_reward".to_string(), hue_reward);
    loss_components.insert("hue_separation_deficit".to_string(), hue_deficit);
    loss_components.insert("min_hue_gap_deg".to_string(), min_hue_gap);
    loss_components.insert("term_loss".to_string(), term_loss_val as f32);
    loss_components.insert("saturation_deficit".to_string(), saturation_deficit_penalty);
    loss_components.insert("min_saturation_reward".to_string(), minus_min_saturation);
    loss_components.insert("min_srgb_saturation".to_string(), min_srgb_saturation);
    loss_components.insert("min_oklab_chroma".to_string(), min_oklab_chroma);
    // legacy keys for older UI code
    loss_components.insert("chroma_deficit".to_string(), saturation_deficit_penalty);
    loss_components.insert("mean_chroma_reward".to_string(), minus_min_saturation);
    loss_components.insert("mean_chroma".to_string(), min_oklab_chroma);
    Ok(loss_components)
}

#[wasm_bindgen]
pub fn calculate_palette_loss(
    intensities: &[u16],
    colors: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>,
    color_names: Vec<String>,
    include_spatial_channel_overlap: Option<bool>,
) -> JsValue {
    // Log all values
    let float_color_map: Vec<f32> = colors
        .iter()
        .map(|&x| (x as f32) / 255.0)
        .collect::<Vec<f32>>();

    // Convert to Oklab

    let float_luminance_values: Vec<f32> = luminance_values
        .iter()
        .map(|&x| (x as f32) / 100.0)
        .collect::<Vec<f32>>();

    let oklab_color_map: Vec<f32> = float_color_map
        .chunks(3)
        .map(|color| {
            let rgb = Srgb::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let oklab: Oklab = Oklab::from_color(rgb);
            vec![oklab.l, oklab.a, oklab.b]
        })
        .flatten()
        .collect::<Vec<f32>>();

    let excluded_colors = merge_excluded_color_names(excluded_colors);
    let mut loss: HashMap<String, f32> =
        color_only_loss(&oklab_color_map, excluded_colors, color_names).unwrap();

    let include_overlap = include_spatial_channel_overlap.unwrap_or(true);
    loss.insert(
        "spatial_channel_overlap".to_string(),
        if include_overlap { 1.0 } else { 0.0 },
    );

    if include_overlap {
        let confusion = optimize_for_confusion(
            intensities,
            colors,
            contrast_limits,
            &float_luminance_values,
        );
        loss.insert("confusion".to_string(), confusion - (1.0 as f32));
    } else {
        loss.insert("confusion".to_string(), 0.0);
    }
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    {
        console::log_1(&format!("confusion: {:?}", confusion).into());
        console::log_1(&format!("Loss: {:?}", loss).into());
    }

    JsValue::from_serde(&loss).unwrap()
}

fn compute_confusion_loss(
    oklab_color_map: &[f32],
    intensities_array_float: &Array2<f32>,
    avg_confusion: f32,
) -> f32 {
    palette_eval::with_eval_scratch(|scratch| {
        compute_confusion_loss_fast(
            oklab_color_map,
            intensities_array_float,
            avg_confusion,
            &mut scratch.mixed_oklab,
        )
    })
}

/// Spatial confusion term; reuses `mixed_out` buffer (rows × 3) across calls.
pub(crate) fn compute_confusion_loss_fast(
    oklab_color_map: &[f32],
    intensities_array_float: &Array2<f32>,
    avg_confusion: f32,
    mixed_out: &mut Array2<f32>,
) -> f32 {
    let num_channels = intensities_array_float.ncols();
    let num_rows = intensities_array_float.nrows();
    if mixed_out.nrows() != num_rows || mixed_out.ncols() != 3 {
        *mixed_out = Array2::zeros((num_rows, 3));
    }
    for row in 0..num_rows {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut z = 0.0f32;
        for channel in 0..num_channels {
            let intensity_value = intensities_array_float[[row, channel]];
            let oklab_colored = Oklab::new(
                oklab_color_map[channel * 3] * intensity_value,
                oklab_color_map[channel * 3 + 1] * intensity_value,
                oklab_color_map[channel * 3 + 2] * intensity_value,
            );
            let xyz: Xyz = Xyz::from_color(oklab_colored);
            x += xyz.x;
            y += xyz.y;
            z += xyz.z;
        }
        let okl = Oklab::from_color(Xyz::new(x, y, z));
        mixed_out[[row, 0]] = okl.l;
        mixed_out[[row, 1]] = okl.a;
        mixed_out[[row, 2]] = okl.b;
    }

    let rmse = calculate_ols_msre_borrowed(intensities_array_float, mixed_out);
    rmse.unwrap() / avg_confusion
}

fn calculate_ols_msre_borrowed(
    records: &Array2<f32>,
    targets: &Array2<f32>,
) -> Result<f32, Box<dyn std::error::Error>> {
    let num_targets = targets.ncols();
    let mut total_mse = 0.0f32;
    let records_owned = records.to_owned();

    for i in 0..num_targets {
        let target_column = targets.column(i).to_owned();
        let dataset_with_single_target = Dataset::new(records_owned.clone(), target_column.clone());
        let model = LinearRegression::new();
        let fitted_model = model.fit(&dataset_with_single_target)?;
        let predictions = fitted_model.predict(&dataset_with_single_target);
        let mse = (predictions - target_column)
            .mapv(|x| x.powi(2))
            .mean()
            .unwrap();
        total_mse += mse;
    }

    let avg_mse = total_mse / (num_targets as f32);
    Ok(avg_mse.sqrt() * 10.0)
}

fn calculate_average_confusion(
    luminance_values: &[f32],
    colors: &[f32],
    intensities_array_float: &Arc<Array2<f32>>,
    num_samples: u32,
    mc_rng_seed: Option<u64>,
) -> f32 {
    let mut total_confusion = 0.0;
    let mut num_samples_done = 0u32;
    match mc_rng_seed {
        Some(seed) => {
            let mut rng = StdRng::seed_from_u64(seed);
            for _ in 0..num_samples {
                let random_colors = random_palette_mc_sample(colors, luminance_values, &mut rng);
                let confusion =
                    compute_confusion_loss(&random_colors, intensities_array_float.as_ref(), 1.0);
                total_confusion += confusion;
                num_samples_done += 1;
            }
        }
        None => {
            let mut rng = thread_rng();
            for _ in 0..num_samples {
                let random_colors = random_palette_mc_sample(colors, luminance_values, &mut rng);
                let confusion =
                    compute_confusion_loss(&random_colors, intensities_array_float.as_ref(), 1.0);
                total_confusion += confusion;
                num_samples_done += 1;
            }
        }
    }
    let avg_confusion = total_confusion / (num_samples_done as f32);
    // console::log_1(&format!("luminance_values: {:?}", luminance_values).into());
    // console::log_1(&format!("avg_confusion: {:?}", avg_confusion).into());
    avg_confusion
}

fn optimize_for_confusion(
    intensities: &[u16],
    colors: &[u16],
    contrast_limits: &[u16],
    float_luminance_values: &Vec<f32>,
) -> f32 {
    let intensities_array_float: Array2<f32> =
        preprocess_data(colors, intensities, contrast_limits);

    let float_color_map: Vec<f32> = colors
        .iter()
        .map(|&x| (x as f32) / 255.0)
        .collect::<Vec<f32>>();

    let oklab_color_map: Vec<f32> = float_color_map
        .chunks(3)
        .map(|color| {
            let rgb = Srgb::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let oklab: Oklab = Oklab::from_color(rgb);
            vec![oklab.l, oklab.a, oklab.b]
        })
        .flatten()
        .collect::<Vec<f32>>();
    #[cfg(debug_assertions)]
    println!("color_map: {:?}", oklab_color_map);
    let intensities_arc = Arc::new(intensities_array_float);
    let avg_confusion: f32 = calculate_average_confusion(
        float_luminance_values,
        &oklab_color_map,
        &intensities_arc,
        DEFAULT_CONFUSION_BASELINE_SAMPLES,
        None,
    );

    let rmse = compute_confusion_loss(&oklab_color_map, intensities_arc.as_ref(), avg_confusion);
    rmse
}

#[wasm_bindgen]
pub fn optimize_in_lens(
    intensities: &[u16],
    colors: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
) -> f32 {
    // console log colors

    let float_luminance_values: Vec<f32> = luminance_values
        .iter()
        .map(|&x| (x as f32) / 100.0)
        .collect::<Vec<f32>>();

    optimize_for_confusion(
        intensities,
        colors,
        contrast_limits,
        &float_luminance_values,
    )
}

#[cfg(test)]
mod opt_eval_tests;
