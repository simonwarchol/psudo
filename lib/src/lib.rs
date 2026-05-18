//! Primary wall-time costs: Monte Carlo confusion baseline, then simulated-annealing `Loss::cost`
//! (C3 name distance + regression confusion each iteration). Profiling: browser Performance panel
//! around `optimize`, or `cargo build --release` timings on native tests.

mod utils;
use ndarray::{ Array1, Array2 };
use web_sys::console;
use linfa::traits::Fit; // Import the Fit trait
use rand::seq::SliceRandom; // Import
use rand::thread_rng; // Import the RNG
use linfa_linear::LinearRegression;
use linfa::prelude::Predict; // Continue to include linfa prelude for other necessary traits and structures
use linfa_linear;
use ndarray::Axis;
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use linfa_clustering::{ GaussianMixtureModel };
use linfa::Dataset;
use statrs::distribution::{ Continuous, Normal };
use ndarray_stats::QuantileExt; // <-- Add this line
use palette::{ FromColor, Oklab, Srgb, Lab, Xyz };
use std::sync::{ Arc, Mutex };
use rand_xoshiro::Xoshiro256PlusPlus;
use argmin::core::{ CostFunction, Error, Executor, State };
use argmin::solver::simulatedannealing::{ Anneal, SATempFunc, SimulatedAnnealing };
use rand::Rng;

use rand::distributions::Uniform;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::{ HashMap, HashSet };

use wasm_bindgen::prelude::*;

pub mod c3;

/// Default simulated-annealing iterations (native / palette study).
const DEFAULT_MAX_ITERS: u32 = 3000;
/// Browser WASM: fewer SA steps; spread inits + one light polish recover quality.
#[cfg(target_arch = "wasm32")]
const DEFAULT_MAX_ITERS_WASM: u32 = 1500;
/// Independent SA runs per `optimize`; best palette by recomputed [`PaletteObjectiveBreakdown::total`].
const DEFAULT_NUM_RESTARTS: u32 = 4;
#[cfg(target_arch = "wasm32")]
const DEFAULT_NUM_RESTARTS_WASM: u32 = 2;
/// Initial SA temperature (must match [`build_simulated_annealing_solver`] and [`Loss::anneal`] scaling).
const SA_INITIAL_TEMP: f32 = 12.0;
/// Exponential cooling factor per SA iteration: `T_i = T_0 * factor^i`.
const SA_TEMP_DECAY: f32 = 0.997;
/// Baseline confusion uses fewer MC draws than legacy 100 for speed; override via `optimize` options.
const DEFAULT_CONFUSION_BASELINE_SAMPLES: u32 = 32;
#[cfg(target_arch = "wasm32")]
const DEFAULT_CONFUSION_BASELINE_SAMPLES_WASM: u32 = 16;

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
        (2, 6)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        (4, 16)
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
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}
/// Golden-ratio stride for per-restart RNG seeds derived from [`problem_seed`].
const RESTART_SEED_STRIDE: u64 = 0x9E3779B97F4A7C15;
/// Weight on the multi-channel spatial confusion term (`compute_confusion_loss`).
const SPATIAL_CONFUSION_WEIGHT: f32 = 0.1;

// Palette objective (minimize total):
//   −mean C3 name distance  −min display-sRGB Δ  + perc_deficit  + term_loss  + w·confusion
//   −w_sat·min(chroma,sat)  + w_def·Σ(chroma/sat deficits)²
// Hard floors in SA `anneal`: OKLab chroma ≥ DEFAULT_MIN_OKLAB_CHROMA, sRGB sat ≥ MIN_SRGB_SATURATION.
/// Minimum OKLab chroma √(a²+b²) per channel (optimization space).
const DEFAULT_MIN_OKLAB_CHROMA: f32 = 0.16;
/// Minimum linear-sRGB saturation (max−min)/max per channel — matches “pale” in the viewer.
const MIN_SRGB_SATURATION: f32 = 0.42;
/// Penalty when any channel violates OKLab chroma or sRGB saturation floors.
const SATURATION_DEFICIT_WEIGHT: f32 = 10.0;
/// Reward on the *dullest* channel: `-weight * min(min_oklab_chroma, min_srgb_sat)`.
const MIN_SAT_REWARD_WEIGHT: f32 = 2.5;
/// Target minimum display-sRGB distance (0–255 scale) between channel colors.
const MIN_DISPLAY_RGB_DISTANCE: f64 = 90.0;
const PERCEPTUAL_SCALE: f64 = 255.0;
const PERCEPTUAL_DEFICIT_WEIGHT: f32 = 6.0;
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

fn oklab_chroma(a: f32, b: f32) -> f32 {
    (a * a + b * b).sqrt()
}

fn channel_srgb_saturation(l: f32, a: f32, b: f32) -> f32 {
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
fn saturation_objective_terms(oklab_flat: &[f32]) -> (f32, f32, f32, f32) {
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
            let okl = Oklab::new(c[0], c[1], c[2]);
            let lab: Lab = Lab::from_color(okl);
            let name = c3.dominant_term_name([lab.l as f64, lab.a as f64, lab.b as f64]);
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
fn enforce_channel_saturation(oklab_flat: &mut [f32], color_idx: usize, rng: &mut impl Rng) {
    let base = color_idx * 3;
    for _ in 0..32 {
        let ch = oklab_chroma(oklab_flat[base + 1], oklab_flat[base + 2]);
        let sat = channel_srgb_saturation(
            oklab_flat[base],
            oklab_flat[base + 1],
            oklab_flat[base + 2]
        );
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
    luminance_values: &[u16]
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
    initial_temp: f32
) -> Result<SimulatedAnnealing<f32, Xoshiro256PlusPlus>, Error> {
    let reanneal = (max_iters / 4).clamp(250, 900);
    Ok(
        SimulatedAnnealing::new(initial_temp)?
            .with_temp_func(SATempFunc::Exponential(SA_TEMP_DECAY))
            .with_reannealing_best(reanneal)
    )
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
fn polish_oklab_palette(
    oklab: &mut Vec<f32>,
    locked_colors: &[bool],
    luminance_values: &[f32],
    c3: &c3::C3,
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32],
    fast: bool
) {
    let cost = |p: &[f32]| {
        evaluate_palette_objective_breakdown_with_excluded_set(
            c3,
            p,
            intensity_arc,
            avg_confusion,
            spatial_w,
            excluded_set,
            color_name_indices
        )
        .total
    };
    let n_colors = oklab.len() / 3;
    if n_colors == 0 {
        return;
    }
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
                                    trial[base] = trial[base].clamp(
                                        luminance_values[0],
                                        luminance_values[1]
                                    );
                                } else {
                                    trial[base + comp] =
                                        trial[base + comp].clamp(-0.4, 0.4);
                                }
                                let mut rng = StdRng::seed_from_u64(
                                    (base as u64)
                                        .wrapping_add(comp as u64)
                                        .wrapping_add(trial[base + 1].to_bits() as u64)
                                );
                                enforce_channel_saturation(
                                    &mut trial,
                                    color_idx,
                                    &mut rng
                                );
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
fn refine_oklab_palette(
    oklab: &mut Vec<f32>,
    locked_colors: &[bool],
    luminance_values: &[f32],
    c3: &c3::C3,
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32],
    rng_seed: u64
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
        color_name_indices
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
            color_name_indices
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
        false
    );
}

fn random_palette_mc_sample(colors: &[f32], luminance_values: &[f32], rng: &mut impl Rng) -> Vec<f32> {
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
    rng: &mut impl Rng
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

/// Evenly spaced hues in OKLab — fewer SA restarts needed than fully random inits (WASM default).
fn spread_initial_oklab(
    fallback: &[f32],
    locked_colors: &[bool],
    luminance_values: &[f32],
    n_colors: usize,
    rng: &mut impl Rng
) -> Vec<f32> {
    let mut out = fallback.to_vec();
    let l = 0.58f32;
    for color_idx in 0..n_colors {
        if locked_colors[color_idx] {
            continue;
        }
        let base = color_idx * 3;
        let angle = std::f32::consts::TAU * (color_idx as f32) / (n_colors as f32)
            + rng.gen_range(-0.12f32..0.12f32);
        let chroma = rng.gen_range(0.18f32..0.34f32);
        out[base] = l.clamp(luminance_values[0], luminance_values[1]);
        out[base + 1] = chroma * angle.cos();
        out[base + 2] = chroma * angle.sin();
        enforce_channel_saturation(&mut out, color_idx, rng);
    }
    out
}

#[inline]
fn sa_initial_oklab(
    oklab_flat: &[f32],
    locked_colors: &[bool],
    luminance_values: &[f32],
    rng: &mut impl Rng
) -> Vec<f32> {
    #[cfg(target_arch = "wasm32")]
    {
        spread_initial_oklab(
            oklab_flat,
            locked_colors,
            luminance_values,
            oklab_flat.len() / 3,
            rng
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
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
            intensities_array_float[[row, channel]] =
                (intensities_array[[row, channel]] as f32) /
                ((contrast_limits[channel * 2 + 1] - contrast_limits[channel * 2]) as f32);
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
    let vals = array_vec.par_iter().map(|&x| x as f32).collect::<Vec<f32>>();
    // take a random sample of 1000 values
    // Iterate over vals, if value is 0 or nan, make it 0, otherwise take the log

    let vals_log = vals
        .iter()
        .map(|&x| {
            if x <= 0.0 || x.is_nan() { 0.0 } else { x.ln() }
        })
        .collect::<Array1<f32>>();
    return vals_log.to_vec();
}

#[wasm_bindgen]
pub fn channel_gmm(array: &[u16]) -> Vec<f32> {
    // console::log_1(&"Starting GMM".into());
    let sampled_array = if array.len() > 20_000 {
        let mut rng = rand::thread_rng();
        array.choose_multiple(&mut rng, 20_000).cloned().collect::<Vec<_>>()
    } else {
        array.to_vec()
    };

    #[cfg(target_arch = "wasm32")]
    let vals: Vec<f32> = sampled_array.iter().map(|&x| x as f32).collect();
    #[cfg(not(target_arch = "wasm32"))]
    let vals = sampled_array.par_iter().map(|&x| x as f32).collect::<Vec<f32>>();
    // take a random sample of 1000 values
    // Iterate over vals, if value is 0 or nan, make it 0, otherwise take the log

    let vals_log = vals
        .iter()
        .map(|&x| {
            if x <= 0.0 || x.is_nan() { 0.0 } else { x.ln() }
        })
        .collect::<Array1<f32>>();

    let dataset = Dataset::from(vals_log.insert_axis(Axis(1)));

    // console::log_1(&"Created Dataset!".into());
    let gmm_result = GaussianMixtureModel::params(3)
        .n_runs(10)
        .tolerance(1e-4)
        .max_n_iterations(500)
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

    let flattend_means = means.view().into_shape((means.len(),)).unwrap().to_owned().into_raw_vec();
    let mut indexed_values: Vec<(usize, f32)> = flattend_means
        .iter()
        .enumerate()
        .map(|(i, &val)| (i, val))
        .collect();
    indexed_values.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    // Extract indices from the sorted pairs.
    let (_i0, i1, i2) = (indexed_values[0].0, indexed_values[1].0, indexed_values[2].0);
    let (mean1, mean2) = (flattend_means[i1], flattend_means[i2]);
    let (std1, std2) = (covariances[[i1, 0, 0]].sqrt(), covariances[[i2, 0, 0]].sqrt());
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
            oklab_flat[idx * 3 + 2]
        );
        let rgb: Srgb = Srgb::from_color(okl);
        [
            (rgb.red.clamp(0.0, 1.0) * 255.0) as f64,
            (rgb.green.clamp(0.0, 1.0) * 255.0) as f64,
            (rgb.blue.clamp(0.0, 1.0) * 255.0) as f64
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
    let deficit_penalty =
        (deficit_sq / (scale * scale) * PERCEPTUAL_DEFICIT_WEIGHT as f64) as f32;
    (minus_min, deficit_penalty, min_dist as f32)
}

// ///////////////////////////////////////// Optimization /////////////////////////////////////
struct Loss {
    rng: Arc<Mutex<Xoshiro256PlusPlus>>,
    locked_colors: Vec<bool>,
    intensity_array: Arc<Array2<f32>>,
    luminance_values: Vec<f32>,
    avg_confusion: f32,
    /// When `0.0`, skip `compute_confusion_loss` (no spatial multi-channel overlap objective).
    spatial_confusion_weight: f32,
    excluded_colors_set: HashSet<usize>,
    color_name_indices: Vec<f32>,
    c3_instance: c3::C3,
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
        c3_instance: c3::C3,
        anneal_rng_seed: Option<u64>
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

fn term_loss(
    palette_terms: &[Vec<HashMap<&str, f64>>],
    excluded_colors: &HashSet<usize>,
    color_name_indices: &[f32]
) -> f32 {
    let mut loss = 0.0;

    for term in palette_terms.iter() {
        for color in term {
            if excluded_colors.contains(&(color["index"] as usize)) {
                loss += color["score"];
            }
        }
    }

    let mut iter = 0;
    for term in palette_terms.iter() {
        if iter >= color_name_indices.len() || color_name_indices[iter] == -1.0 {
            iter += 1;
            continue;
        }
        for color in term {
            if (color["index"] as f32) == color_name_indices[iter] {
                loss -= color["score"];
            }
        }
        iter += 1;
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
    /// Negative minimum display-sRGB distance (÷ [`PERCEPTUAL_SCALE`]).
    pub minus_min_perceptual_distance: f32,
    /// Penalty when any pair is too close in display sRGB.
    pub perceptual_deficit_penalty: f32,
    /// Minimum display-sRGB distance 0–255 (diagnostic).
    pub min_display_rgb_distance: f32,
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
    color_name_indices: &[f32]
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
        color_name_indices
    )
}

fn evaluate_palette_objective_breakdown_with_excluded_set(
    c3: &c3::C3,
    oklab_flat: &[f32],
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_confusion_weight: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32]
) -> PaletteObjectiveBreakdown {
    let mut cielab_colors: Vec<Vec<f32>> = Vec::new();
    for color in oklab_flat.chunks(3) {
        let okl = Oklab::new(color[0] as f32, color[1] as f32, color[2] as f32);
        let lab: Lab = Lab::from_color(okl);
        cielab_colors.push(vec![lab.l, lab.a, lab.b]);
    }
    let lab_palette = Array2::from_shape_vec(
        (oklab_flat.len() / 3, 3),
        cielab_colors
            .iter()
            .flatten()
            .map(|&x| x as f64)
            .collect::<Vec<f64>>()
    ).unwrap();
    let analyzed_palette: Vec<HashMap<&str, f64>> = c3.analyze_palette(lab_palette.clone());
    let palette_terms = c3.get_palette_terms(lab_palette.clone(), 10);

    let average_cosine_distance = c3.average_pairwise_color_name_distance(&analyzed_palette);
    let term = term_loss(&palette_terms, excluded_set, color_name_indices);

    let minus_mean = -average_cosine_distance as f32;
    let (minus_min_perceptual, perceptual_deficit_penalty, min_display_rgb_distance) =
        perceptual_objective_terms(oklab_flat);
    let (minus_min_saturation, saturation_deficit_penalty, min_srgb_saturation, min_oklab_chroma) =
        saturation_objective_terms(oklab_flat);
    let mut confusion_weighted = 0.0f32;
    if spatial_confusion_weight > 0.0 {
        confusion_weighted = spatial_confusion_weight *
            compute_confusion_loss(oklab_flat, intensity_arc.as_ref(), avg_confusion);
    }
    let total = minus_mean
        + minus_min_perceptual
        + perceptual_deficit_penalty
        + term
        + confusion_weighted
        + minus_min_saturation
        + saturation_deficit_penalty;

    PaletteObjectiveBreakdown {
        total,
        minus_mean_color_name_distance: minus_mean,
        minus_min_perceptual_distance: minus_min_perceptual,
        perceptual_deficit_penalty,
        min_display_rgb_distance,
        term_loss: term,
        confusion_weighted,
        minus_min_saturation,
        saturation_deficit_penalty,
        min_srgb_saturation,
        min_oklab_chroma,
    }
}

impl CostFunction for Loss {
    type Param = Vec<f32>;
    type Output = f32;

    fn cost(&self, param: &Self::Param) -> Result<Self::Output, Error> {
        let b = evaluate_palette_objective_breakdown_with_excluded_set(
            &self.c3_instance,
            param,
            &self.intensity_array,
            self.avg_confusion,
            self.spatial_confusion_weight,
            &self.excluded_colors_set,
            &self.color_name_indices
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
                        param_n[idx] = param_n[idx].clamp(
                            self.luminance_values[0],
                            self.luminance_values[1]
                        );
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
fn annealing(
    colors: &[f32],
    locked_colors: &[bool],
    intensity_array: Arc<Array2<f32>>,
    luminance_values: &[f32],
    excluded_colors_indices: &[f32],
    color_name_indices: &[f32],
    c3_instance: c3::C3,
    max_iters: u32,
    confusion_baseline_samples: u32,
    init_rng_seed: Option<u64>,
    anneal_rng_seed: Option<u64>,
    mc_rng_seed: Option<u64>,
    include_spatial_channel_overlap: bool,
    precomputed_avg_confusion: Option<f32>,
    start_oklab: Option<&[f32]>,
    sa_initial_temp: Option<f32>
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
            mc_rng_seed
        ),
        None => 1.0,
    };
    let start_param = if let Some(start) = start_oklab {
        start.to_vec()
    } else {
        match init_rng_seed {
            Some(seed) => {
                let mut rng = StdRng::seed_from_u64(seed);
                sa_initial_oklab(colors, locked_colors, luminance_values, &mut rng)
            }
            None => {
                let mut rng = thread_rng();
                sa_initial_oklab(colors, locked_colors, luminance_values, &mut rng)
            }
        }
    };

    let cost_function = Loss::new(
        locked_colors.to_vec(),
        intensity_array,
        luminance_values.to_vec(),
        average_confusion,
        spatial_w,
        excluded_colors_indices.to_vec(),
        color_name_indices.to_vec(),
        c3_instance,
        anneal_rng_seed
    );
    // Optional: Define temperature function (defaults to `SATempFunc::TemperatureFast`)
    let res = Executor::new(cost_function, solver)
        .configure(|state| {
            state.param(start_param).max_iters(max_iters as u64)
        })
        // Optional: Attach an observer
        .run()?;
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    {
        console::log_1(&format!("loss: {:?}", res.state().best_cost).into());
        let best_param_dbg = res.state().get_best_param().unwrap().clone();
        console::log_1(&format!("best_param: {:?}", best_param_dbg).into());
    }
    let best_param = res.state().get_best_param().unwrap().clone();
    let best_cost = res.state().get_best_cost();

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
    num_restarts: Option<u32>
) -> OptimizePipelineResult {
    let n_channels = colors.len() / 3;
    let (restart_min, restart_max) = restart_count_bounds();
    let max_iters = scale_budget_for_channels(
        max_iters.unwrap_or_else(default_max_iters).max(1),
        n_channels
    );
    let confusion_baseline_samples = confusion_baseline_samples
        .unwrap_or_else(default_confusion_baseline_samples)
        .max(1);
    let include_overlap =
        include_spatial_channel_overlap.unwrap_or_else(default_include_spatial_overlap);
    let num_restarts = scale_budget_for_channels(
        num_restarts.unwrap_or_else(default_num_restarts).max(1),
        n_channels
    )
    .clamp(restart_min, restart_max);
    let full_post = pipeline_use_full_postprocess();
    let polish_fast = !full_post;

    let float_luminance_values: Vec<f32> = luminance_values
        .iter()
        .map(|&x| (x as f32) / 100.0)
        .collect::<Vec<f32>>();

    let float_color_map: Vec<f32> = colors
        .iter()
        .map(|&x| (x as f32) / 255.0)
        .collect::<Vec<f32>>();

    let locked_colors_vec = locked_colors
        .iter()
        .map(|&x| x == 1)
        .collect::<Vec<bool>>();

    let oklab_color_map: Vec<f32> = float_color_map
        .chunks(3)
        .map(|color| {
            let rgb = Srgb::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let oklab: Oklab = Oklab::from_color(rgb);
            vec![oklab.l, oklab.a, oklab.b]
        })
        .flatten()
        .collect::<Vec<f32>>();
    let intensity_array = preprocess_data(colors, intensities, contrast_limits);
    let excluded_colors = merge_excluded_color_names(excluded_colors);
    let c3_instance = c3::C3::new();
    let mut excluded_colors_indices = Vec::new();
    for color in excluded_colors {
        if let Some(index) = c3_instance.get_term_index(&color) {
            excluded_colors_indices.push(index as f32);
        }
    }

    let mut color_name_indices = Vec::new();
    for color in color_names {
        if color == "" {
            color_name_indices.push(-1.0f32);
            continue;
        }
        if let Some(index) = c3_instance.get_term_index(&color) {
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
            Some(base_seed.wrapping_add(0xA5A5_5A5A_5A5A_5A5A))
        )
    } else {
        1.0
    };

    let c3_eval = c3::C3::new();
    let excluded_set: HashSet<usize> = excluded_colors_indices
        .iter()
        .map(|&x| x as usize)
        .collect();

    let eval_total = |oklab: &[f32]| {
        evaluate_palette_objective_breakdown_with_excluded_set(
            &c3_eval,
            oklab,
            &intensity_arc,
            avg_confusion,
            spatial_w,
            &excluded_set,
            &color_name_indices
        )
        .total
    };

    let mut best_oklab = oklab_color_map.clone();
    let mut best_total = f32::INFINITY;
    let mut best_sa_cost = f32::INFINITY;

    for restart in 0..num_restarts {
        let init_seed = base_seed.wrapping_add((restart as u64).wrapping_mul(RESTART_SEED_STRIDE));
        let anneal_seed = init_seed.wrapping_add(0x517C_C1B0_2722_0A95);
        let (mut candidate, sa_cost) = annealing(
            &oklab_color_map,
            &locked_colors_vec,
            Arc::clone(&intensity_arc),
            &float_luminance_values,
            &excluded_colors_indices,
            &color_name_indices,
            c3::C3::new(),
            max_iters,
            confusion_baseline_samples,
            Some(init_seed),
            Some(anneal_seed),
            None,
            include_overlap,
            Some(avg_confusion),
            None,
            None
        )
        .expect("annealing");

        if full_post {
            polish_oklab_palette(
                &mut candidate,
                &locked_colors_vec,
                &float_luminance_values,
                &c3_eval,
                &intensity_arc,
                avg_confusion,
                spatial_w,
                &excluded_set,
                &color_name_indices,
                false
            );
        }
        let total = eval_total(&candidate);
        if total < best_total {
            best_total = total;
            best_oklab = candidate;
            best_sa_cost = sa_cost;
        }
    }

    polish_oklab_palette(
        &mut best_oklab,
        &locked_colors_vec,
        &float_luminance_values,
        &c3_eval,
        &intensity_arc,
        avg_confusion,
        spatial_w,
        &excluded_set,
        &color_name_indices,
        polish_fast
    );
    best_total = eval_total(&best_oklab);

    if full_post {
        let quench_iters = scale_budget_for_channels(max_iters / 4, n_channels).max(150);
        let quench_seed = base_seed.wrapping_add(0x51EE_C0DE);
        if let Ok((mut quenched, _)) = annealing(
            &oklab_color_map,
            &locked_colors_vec,
            Arc::clone(&intensity_arc),
            &float_luminance_values,
            &excluded_colors_indices,
            &color_name_indices,
            c3::C3::new(),
            quench_iters,
            confusion_baseline_samples,
            Some(quench_seed),
            Some(quench_seed.wrapping_add(1)),
            None,
            include_overlap,
            Some(avg_confusion),
            Some(&best_oklab),
            Some(QUENCH_TEMP)
        ) {
            polish_oklab_palette(
                &mut quenched,
                &locked_colors_vec,
                &float_luminance_values,
                &c3_eval,
                &intensity_arc,
                avg_confusion,
                spatial_w,
                &excluded_set,
                &color_name_indices,
                false
            );
            let total = eval_total(&quenched);
            if total < best_total {
                best_total = total;
                best_oklab = quenched;
            }
        }

        refine_oklab_palette(
            &mut best_oklab,
            &locked_colors_vec,
            &float_luminance_values,
            &c3_eval,
            &intensity_arc,
            avg_confusion,
            spatial_w,
            &excluded_set,
            &color_name_indices,
            base_seed.wrapping_add(0xA11CE)
        );
        best_total = eval_total(&best_oklab);
    }

    let optimized_oklab = best_oklab;
    let sa_best_cost = best_sa_cost;

    let srgb_linear = optimized_oklab
        .chunks(3)
        .map(|color| {
            let okl = Oklab::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let rgb: Srgb = Srgb::from_color(okl);
            vec![rgb.red.clamp(0.0, 1.0), rgb.green.clamp(0.0, 1.0), rgb.blue.clamp(0.0, 1.0)]
        })
        .flatten()
        .collect::<Vec<f32>>();

    OptimizePipelineResult {
        srgb_linear,
        sa_best_cost,
        oklab_best: optimized_oklab,
        intensity_arc,
        excluded_colors_indices,
        color_name_indices,
    }
}

/// `include_spatial_channel_overlap`: `None` or `true` = full objective including per-pixel
/// multi-channel confusion; `false` = name + OKLab separation + terms only (round‑1 eval).
///
/// `num_restarts`: independent SA runs (default 4 native, 2 WASM); keeps the lowest recomputed total loss.
///
/// WASM builds use a smaller SA budget, spread hue inits, and a single light polish (no quench/refine).
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
    num_restarts: Option<u32>
) -> Vec<f32> {
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
        num_restarts
    );

    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    {
        let elapsed = now.elapsed();
        console::log_1(
            &format!(
                "optimize ({} restarts) took: {:?}",
                num_restarts.unwrap_or_else(default_num_restarts),
                elapsed
            )
            .into()
        );
    }
    r.srgb_linear
}

fn color_only_loss(
    param: &Vec<f32>,
    excluded_colors: Vec<String>,
    color_names: Vec<String>
) -> Result<HashMap<String, f32>, Error> {
    let excluded_colors = merge_excluded_color_names(excluded_colors);
    let mut cielab_colors: Vec<Vec<f32>> = Vec::new();
    let oklab_colors = param;
    let c3_instance = c3::C3::new();
    for color in oklab_colors.chunks(3) {
        let okl = Oklab::new(color[0] as f32, color[1] as f32, color[2] as f32);
        let lab: Lab = Lab::from_color(okl);
        cielab_colors.push(vec![lab.l, lab.a, lab.b]);
    }

    let lab_palette = Array2::from_shape_vec(
        (oklab_colors.len() / 3, 3),
        cielab_colors
            .iter()
            .flatten()
            .map(|&x| x as f64)
            .collect::<Vec<f64>>()
    ).unwrap();

    let analyzed_palette: Vec<HashMap<&str, f64>> = c3_instance.analyze_palette(
        lab_palette.clone()
    );

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
    let palette_terms = c3_instance.get_palette_terms(lab_palette.clone(), 10);
    let term_loss_val = term_loss(
        &palette_terms,
        &excluded_set,
        &color_name_indices
    );

    let average_cosine_distance = c3_instance.average_pairwise_color_name_distance(
        &analyzed_palette
    );
    let (minus_min_perceptual, perceptual_deficit_penalty, min_display_rgb_distance) =
        perceptual_objective_terms(param);
    let (minus_min_saturation, saturation_deficit_penalty, min_srgb_saturation, min_oklab_chroma) =
        saturation_objective_terms(param);

    let mut loss_components = HashMap::new();
    loss_components.insert("name_distance".to_string(), -average_cosine_distance as f32);
    loss_components.insert("perceptual_distance".to_string(), minus_min_perceptual);
    loss_components.insert("perceptural_distance".to_string(), minus_min_perceptual);
    loss_components.insert(
        "perceptual_deficit".to_string(),
        perceptual_deficit_penalty
    );
    loss_components.insert(
        "min_display_rgb_distance".to_string(),
        min_display_rgb_distance
    );
    loss_components.insert("min_perceptual_de2000".to_string(), min_display_rgb_distance);
    loss_components.insert("term_loss".to_string(), term_loss_val as f32);
    loss_components.insert(
        "saturation_deficit".to_string(),
        saturation_deficit_penalty
    );
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
    include_spatial_channel_overlap: Option<bool>
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
    let mut loss: HashMap<String, f32> = color_only_loss(
        &oklab_color_map,
        excluded_colors,
        color_names
    ).unwrap();

    let include_overlap = include_spatial_channel_overlap.unwrap_or(true);
    loss.insert(
        "spatial_channel_overlap".to_string(),
        if include_overlap { 1.0 } else { 0.0 }
    );

    if include_overlap {
        let confusion = optimize_for_confusion(
            intensities,
            colors,
            contrast_limits,
            &float_luminance_values
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

fn calculate_ols_msre(dataset: Dataset<f32, f32>) -> Result<f32, Box<dyn std::error::Error>> {
    let num_targets = dataset.targets().dim().1;
    let mut total_mse = 0.0;

    // Fit a model on each target individually
    for i in 0..num_targets {
        let target_column = dataset.targets().column(i).to_owned();

        let dataset_with_single_target = Dataset::new(
            dataset.records().to_owned(),
            target_column.clone()
        );
        let model = LinearRegression::new();
        let fitted_model = model.fit(&dataset_with_single_target)?;
        let predictions = fitted_model.predict(&dataset_with_single_target);

        let mse = (predictions - target_column)
            .mapv(|x| x.powi(2))
            .mean()
            .unwrap();
        #[cfg(debug_assertions)]
        println!("mse: {:?}", mse);
        total_mse += mse;
    }

    // Calculate Avg MSE
    let avg_mse = total_mse / (num_targets as f32);
    // Take log of avg_mse
    let sqrt_mse = avg_mse.sqrt() * 10.0;
    // let cbrt_mse = avg_mse.cbrt();
    Ok(sqrt_mse)
}

fn compute_confusion_loss(
    oklab_color_map: &[f32],
    intensities_array_float: &Array2<f32>,
    avg_confusion: f32
) -> f32 {
    let num_channels = intensities_array_float.shape()[1];
    let num_rows = intensities_array_float.shape()[0];
    let mut mixed_array: Array2<f32> = Array2::zeros((num_rows, 3));
    for row in 0..num_rows {
        for channel in 0..num_channels {
            let intensity_value = intensities_array_float[[row, channel]];
            let oklab_colored = Oklab::new(
                oklab_color_map[channel * 3] * intensity_value,
                oklab_color_map[channel * 3 + 1] * intensity_value,
                oklab_color_map[channel * 3 + 2] * intensity_value
            );
            let xyz: Xyz = Xyz::from_color(oklab_colored);
            mixed_array[[row, 0]] += xyz.x;
            mixed_array[[row, 1]] += xyz.y;
            mixed_array[[row, 2]] += xyz.z;
        }
        let okl = Oklab::from_color(
            Xyz::new(mixed_array[[row, 0]], mixed_array[[row, 1]], mixed_array[[row, 2]])
        );
        mixed_array[[row, 0]] = okl.l;
        mixed_array[[row, 1]] = okl.a;
        mixed_array[[row, 2]] = okl.b;
    }

    let dataset = Dataset::new(intensities_array_float.clone(), mixed_array);
    let rmse = calculate_ols_msre(dataset);
    // console::log_1(&format!("rmse: {:?}", rmse).into());
    rmse.unwrap() / avg_confusion
}

fn calculate_average_confusion(
    luminance_values: &[f32],
    colors: &[f32],
    intensities_array_float: &Arc<Array2<f32>>,
    num_samples: u32,
    mc_rng_seed: Option<u64>
) -> f32 {
    let mut total_confusion = 0.0;
    let mut num_samples_done = 0u32;
    match mc_rng_seed {
        Some(seed) => {
            let mut rng = StdRng::seed_from_u64(seed);
            for _ in 0..num_samples {
                let random_colors = random_palette_mc_sample(colors, luminance_values, &mut rng);
                let confusion = compute_confusion_loss(
                    &random_colors,
                    intensities_array_float.as_ref(),
                    1.0
                );
                total_confusion += confusion;
                num_samples_done += 1;
            }
        }
        None => {
            let mut rng = thread_rng();
            for _ in 0..num_samples {
                let random_colors = random_palette_mc_sample(colors, luminance_values, &mut rng);
                let confusion = compute_confusion_loss(
                    &random_colors,
                    intensities_array_float.as_ref(),
                    1.0
                );
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
    float_luminance_values: &Vec<f32>
) -> f32 {
    let intensities_array_float: Array2<f32> = preprocess_data(
        colors,
        intensities,
        contrast_limits
    );

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
        None
    );

    let rmse = compute_confusion_loss(&oklab_color_map, intensities_arc.as_ref(), avg_confusion);
    rmse
}

#[wasm_bindgen]
pub fn optimize_in_lens(
    intensities: &[u16],
    colors: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16]
) -> f32 {
    // console log colors

    let float_luminance_values: Vec<f32> = luminance_values
        .iter()
        .map(|&x| (x as f32) / 100.0)
        .collect::<Vec<f32>>();

    optimize_for_confusion(intensities, colors, contrast_limits, &float_luminance_values)
}

#[cfg(test)]
mod opt_eval_tests;
