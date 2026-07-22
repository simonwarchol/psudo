//! Polar OKLCh local refinement (Exp 4) — search geometry only; objective unchanged.

use crate::c3;
use crate::palette_eval::{fill_c3_labs_from_oklab, fill_display_srgb255, fill_labs_from_oklab};
use crate::{
    enforce_channel_saturation, evaluate_palette_objective_breakdown_with_excluded_set,
    oklab_chroma, polish_oklab_palette, refine_oklab_palette, DEFAULT_MIN_OKLAB_CHROMA,
};
use ndarray::Array2;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::HashSet;
use std::sync::Arc;

/// Post-fold refine path. Production default remains [`PaletteRefineMode::Cartesian`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteRefineMode {
    /// Current behavior: Cartesian a/b jitter refine (+ polish as scheduled by caller).
    Cartesian,
    /// Replace Cartesian jitter refine with polar OKLCh proposals.
    Polar,
    /// Cartesian polish + jitter refine, then polar pair-targeted polish.
    Hybrid,
}

impl PaletteRefineMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Cartesian => "cartesian",
            Self::Polar => "polar",
            Self::Hybrid => "hybrid",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cartesian => "Cartesian refine (production)",
            Self::Polar => "Polar OKLCh refine",
            Self::Hybrid => "Hybrid (cartesian then polar)",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "cartesian" | "total" | "default" => Some(Self::Cartesian),
            "polar" => Some(Self::Polar),
            "hybrid" => Some(Self::Hybrid),
            _ => None,
        }
    }
}

#[inline]
pub fn oklab_to_oklch(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let c = oklab_chroma(a, b);
    let h = b.atan2(a);
    (l, c, h)
}

#[inline]
pub fn oklch_to_oklab(l: f32, c: f32, h: f32) -> (f32, f32, f32) {
    (l, c * h.cos(), c * h.sin())
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PolarRefineStats {
    pub accepted: u32,
    pub rejected: u32,
    pub pair_targeted_accepted: u32,
}

fn objective_total(
    c3: &c3::C3,
    oklab: &[f32],
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32],
) -> f32 {
    evaluate_palette_objective_breakdown_with_excluded_set(
        c3,
        oklab,
        intensity_arc,
        avg_confusion,
        spatial_w,
        excluded_set,
        color_name_indices,
    )
    .total
}

fn min_c3_and_rgb(c3: &c3::C3, oklab: &[f32]) -> (f64, f32) {
    let mut labs = Vec::new();
    let mut samples = Vec::new();
    let mut terms = Vec::new();
    fill_c3_labs_from_oklab(oklab, &mut labs);
    c3.fill_palette_c3(&labs, 10, &mut samples, &mut terms);
    let pairs = c3.pairwise_color_name_distances(&samples);
    let min_c3 = pairs
        .iter()
        .map(|(_, _, d)| *d)
        .fold(f64::INFINITY, f64::min);
    let mut display = Vec::new();
    fill_display_srgb255(oklab, &mut display);
    let n = display.len();
    let mut min_rgb = f32::INFINITY;
    for i in 0..n {
        for j in (i + 1)..n {
            let dr = (display[i][0] - display[j][0]) as f32;
            let dg = (display[i][1] - display[j][1]) as f32;
            let db = (display[i][2] - display[j][2]) as f32;
            min_rgb = min_rgb.min((dr * dr + dg * dg + db * db).sqrt());
        }
    }
    (
        if min_c3.is_finite() { min_c3 } else { 0.0 },
        if min_rgb.is_finite() { min_rgb } else { 0.0 },
    )
}

fn dominant_families(c3: &c3::C3, oklab: &[f32]) -> Vec<String> {
    use crate::palette_diagnostics::coarse_name_family;
    use crate::palette_eval::fill_c3_labs_from_oklab;
    use rust_c3::C3_TERM_STRS;
    let mut labs = Vec::new();
    let mut samples = Vec::new();
    let mut terms = Vec::new();
    fill_c3_labs_from_oklab(oklab, &mut labs);
    c3.fill_palette_c3(&labs, 10, &mut samples, &mut terms);
    terms
        .iter()
        .map(|ch| {
            ch.first()
                .map(|t| {
                    let name = C3_TERM_STRS.get(t.index).copied().unwrap_or("unknown");
                    coarse_name_family(name).to_string()
                })
                .unwrap_or_else(|| "unknown_family".to_string())
        })
        .collect()
}

fn duplicate_family_count(families: &[String]) -> usize {
    let n = families.len();
    let mut c = 0usize;
    for i in 0..n {
        for j in (i + 1)..n {
            if families[i] == families[j] && families[i] != "unknown_family" {
                c += 1;
            }
        }
    }
    c
}

/// Max per-channel Euclidean distance in quantized display sRGB 0..255.
fn max_channel_display_delta(old_oklab: &[f32], new_oklab: &[f32]) -> f32 {
    let mut old_d = Vec::new();
    let mut new_d = Vec::new();
    fill_display_srgb255(old_oklab, &mut old_d);
    fill_display_srgb255(new_oklab, &mut new_d);
    let n = old_d.len().min(new_d.len());
    let mut best = 0.0f32;
    for i in 0..n {
        let or = old_d[i][0].round().clamp(0.0, 255.0);
        let og = old_d[i][1].round().clamp(0.0, 255.0);
        let ob = old_d[i][2].round().clamp(0.0, 255.0);
        let nr = new_d[i][0].round().clamp(0.0, 255.0);
        let ng = new_d[i][1].round().clamp(0.0, 255.0);
        let nb = new_d[i][2].round().clamp(0.0, 255.0);
        let dr = (or - nr) as f32;
        let dg = (og - ng) as f32;
        let db = (ob - nb) as f32;
        best = best.max((dr * dr + dg * dg + db * db).sqrt());
    }
    best
}

fn should_accept_polar(
    c3: &c3::C3,
    old_oklab: &[f32],
    new_oklab: &[f32],
    old_cost: f32,
    new_cost: f32,
    name_tail_tiebreak: bool,
) -> bool {
    if new_cost + 1e-7 < old_cost {
        return true;
    }
    if !name_tail_tiebreak {
        return false;
    }
    let (old_c3, old_rgb) = min_c3_and_rgb(c3, old_oklab);
    let (new_c3, new_rgb) = min_c3_and_rgb(c3, new_oklab);
    // Never accept a move that collapses display separation.
    if new_rgb + 25.0 < old_rgb {
        return false;
    }
    // Soft accepts must change 8-bit display RGB; otherwise review cards can
    // flip C3 labels (pink→purple) with identical hex swatches.
    const MIN_VISIBLE_RGB: f32 = 18.0;
    if max_channel_display_delta(old_oklab, new_oklab) < MIN_VISIBLE_RGB {
        return false;
    }
    // Soft accept: better worst-pair C3 name distance with modest L_tot tradeoff.
    if new_cost <= old_cost + 0.35 && new_c3 > old_c3 + 0.01 {
        return true;
    }
    // Soft accept: break coarse name-family collisions (red/pink). Polished
    // pool winners often need ~1.1 L_tot to leave a gamut-corner hue; keep this
    // study-only (production refine stays strict L_tot).
    let old_dup = duplicate_family_count(&dominant_families(c3, old_oklab));
    let new_dup = duplicate_family_count(&dominant_families(c3, new_oklab));
    new_dup < old_dup && new_cost <= old_cost + 2.0
}

fn worst_display_rgb_pair(oklab: &[f32]) -> Option<(usize, usize)> {
    let n = oklab.len() / 3;
    if n < 2 {
        return None;
    }
    let mut display = Vec::new();
    fill_display_srgb255(oklab, &mut display);
    let mut best = None;
    let mut best_d = f64::INFINITY;
    for i in 0..n {
        for j in (i + 1)..n {
            let dr = display[i][0] - display[j][0];
            let dg = display[i][1] - display[j][1];
            let db = display[i][2] - display[j][2];
            let d = (dr * dr + dg * dg + db * db).sqrt();
            if d < best_d {
                best_d = d;
                best = Some((i, j));
            }
        }
    }
    best
}

fn worst_c3_name_pair(c3: &c3::C3, oklab: &[f32]) -> Option<(usize, usize)> {
    let mut labs = Vec::new();
    let mut samples = Vec::new();
    let mut terms = Vec::new();
    fill_c3_labs_from_oklab(oklab, &mut labs);
    c3.fill_palette_c3(&labs, 10, &mut samples, &mut terms);
    let pairs = c3.pairwise_color_name_distances(&samples);
    pairs
        .into_iter()
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(i, j, _)| (i, j))
}

/// Channels that share a coarse family with at least one other channel.
fn duplicate_family_channel_indices(families: &[String]) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, fi) in families.iter().enumerate() {
        if fi == "unknown_family" {
            continue;
        }
        if families
            .iter()
            .enumerate()
            .any(|(j, fj)| i != j && fi == fj)
        {
            out.push(i);
        }
    }
    out
}

/// Study-only: re-place duplicate-family channels onto absolute OKLCh hues with
/// pulled-back chroma so display RGB actually leaves the gamut corner.
#[allow(clippy::too_many_arguments)]
fn escape_duplicate_families(
    oklab: &mut Vec<f32>,
    locked_colors: &[bool],
    luminance_values: &[f32],
    c3: &c3::C3,
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32],
    rng: &mut StdRng,
    best_cost: &mut f32,
    stats: &mut PolarRefineStats,
) {
    let target_hues_deg: [f32; 12] = [
        15.0, 45.0, 75.0, 105.0, 135.0, 165.0, 195.0, 225.0, 255.0, 285.0, 315.0, 345.0,
    ];
    let chromas: [f32; 5] = [0.20, 0.16, 0.13, 0.11, 0.09];
    let light_deltas: [f32; 3] = [0.0, 0.04, -0.04];

    // Up to two passes so resolving one collision can reveal the next.
    for _ in 0..2 {
        let families = dominant_families(c3, oklab);
        let focus = duplicate_family_channel_indices(&families);
        if focus.is_empty() {
            break;
        }
        let mut progressed = false;
        for &color_idx in &focus {
            if locked_colors.get(color_idx).copied().unwrap_or(true) {
                continue;
            }
            let base = color_idx * 3;
            let (l0, _c0, _h0) = oklab_to_oklch(oklab[base], oklab[base + 1], oklab[base + 2]);
            let mut best_local: Option<(Vec<f32>, f32, f32)> = None;
            for &hdeg in &target_hues_deg {
                for &chroma in &chromas {
                    for &dl in &light_deltas {
                        let mut trial = oklab.clone();
                        if !try_set_channel_oklch(
                            &mut trial,
                            color_idx,
                            l0 + dl,
                            chroma,
                            hdeg.to_radians(),
                            luminance_values,
                            rng,
                        ) {
                            stats.rejected += 1;
                            continue;
                        }
                        let trial_cost = objective_total(
                            c3,
                            &trial,
                            intensity_arc,
                            avg_confusion,
                            spatial_w,
                            excluded_set,
                            color_name_indices,
                        );
                        if should_accept_polar(c3, oklab, &trial, *best_cost, trial_cost, true) {
                            let vis = max_channel_display_delta(oklab, &trial);
                            let better = best_local.as_ref().map_or(true, |(_, c, v)| {
                                vis > *v + 1.0 || (vis + 1.0 >= *v && trial_cost < *c)
                            });
                            if better {
                                best_local = Some((trial, trial_cost, vis));
                            }
                        } else {
                            stats.rejected += 1;
                        }
                    }
                }
            }
            if let Some((trial, trial_cost, _)) = best_local {
                *oklab = trial;
                *best_cost = trial_cost;
                stats.accepted += 1;
                stats.pair_targeted_accepted += 1;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
}

fn try_set_channel_oklch(
    oklab: &mut [f32],
    color_idx: usize,
    l: f32,
    chroma: f32,
    h: f32,
    luminance_values: &[f32],
    rng: &mut StdRng,
) -> bool {
    let base = color_idx * 3;
    let chroma = chroma.max(DEFAULT_MIN_OKLAB_CHROMA);
    let (nl, na, nb) = oklch_to_oklab(l.clamp(luminance_values[0], luminance_values[1]), chroma, h);
    oklab[base] = nl;
    oklab[base + 1] = na.clamp(-0.4, 0.4);
    oklab[base + 2] = nb.clamp(-0.4, 0.4);
    // Re-derive chroma after a/b clamp; reject if collapsed.
    if oklab_chroma(oklab[base + 1], oklab[base + 2]) < DEFAULT_MIN_OKLAB_CHROMA * 0.85 {
        return false;
    }
    enforce_channel_saturation(oklab, color_idx, rng);
    oklab_chroma(oklab[base + 1], oklab[base + 2]) >= DEFAULT_MIN_OKLAB_CHROMA * 0.85
}

/// Polar single-channel + optional pair-targeted proposals.
///
/// Default accept: strict objective improvement.
/// When `name_tail_tiebreak` is true (study review only), also accept a move with
/// slightly worse total if min pairwise C3 name distance improves and display-RGB
/// separation does not drop materially.
#[allow(clippy::too_many_arguments)]
pub fn refine_oklch_palette(
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
    pair_targeted: bool,
    name_tail_tiebreak: bool,
) -> PolarRefineStats {
    let n = oklab.len() / 3;
    let mut stats = PolarRefineStats::default();
    if n == 0 {
        return stats;
    }
    let mut rng = StdRng::seed_from_u64(rng_seed);
    let mut best_cost = objective_total(
        c3,
        oklab,
        intensity_arc,
        avg_confusion,
        spatial_w,
        excluded_set,
        color_name_indices,
    );

    // Study review: pull colliding coarse families off gamut corners before local
    // polar polish (relative hue steps often clip back to the same 8-bit hex).
    if name_tail_tiebreak {
        escape_duplicate_families(
            oklab,
            locked_colors,
            luminance_values,
            c3,
            intensity_arc,
            avg_confusion,
            spatial_w,
            excluded_set,
            color_name_indices,
            &mut rng,
            &mut best_cost,
            &mut stats,
        );
    }

    let hue_steps_deg: [f32; 8] = [20.0, -20.0, 10.0, -10.0, 4.0, -4.0, 1.5, -1.5];
    let chroma_steps: [f32; 6] = [0.04, -0.04, 0.02, -0.02, 0.01, -0.01];
    let light_steps: [f32; 6] = [0.06, -0.06, 0.03, -0.03, 0.015, -0.015];

    let mut improved = true;
    let mut sweeps = 0u32;
    while improved && sweeps < 4 {
        improved = false;
        sweeps += 1;
        for color_idx in 0..n {
            if locked_colors[color_idx] {
                continue;
            }
            let base = color_idx * 3;
            let (l0, c0, h0) = oklab_to_oklch(oklab[base], oklab[base + 1], oklab[base + 2]);

            let mut proposals: Vec<(f32, f32, f32)> = Vec::new();
            for &ddeg in &hue_steps_deg {
                proposals.push((l0, c0, h0 + ddeg.to_radians()));
            }
            for &dc in &chroma_steps {
                proposals.push((l0, (c0 + dc).max(DEFAULT_MIN_OKLAB_CHROMA), h0));
            }
            for &dl in &light_steps {
                proposals.push((l0 + dl, c0, h0));
            }

            for (l, c, h) in proposals {
                let mut trial = oklab.clone();
                if !try_set_channel_oklch(
                    &mut trial,
                    color_idx,
                    l,
                    c,
                    h,
                    luminance_values,
                    &mut rng,
                ) {
                    stats.rejected += 1;
                    continue;
                }
                let trial_cost = objective_total(
                    c3,
                    &trial,
                    intensity_arc,
                    avg_confusion,
                    spatial_w,
                    excluded_set,
                    color_name_indices,
                );
                if should_accept_polar(c3, oklab, &trial, best_cost, trial_cost, name_tail_tiebreak)
                {
                    *oklab = trial;
                    best_cost = trial_cost;
                    stats.accepted += 1;
                    improved = true;
                } else {
                    stats.rejected += 1;
                }
            }
        }
    }

    if pair_targeted && n >= 2 {
        let mut focus: Vec<usize> = Vec::new();
        if let Some((i, j)) = worst_c3_name_pair(c3, oklab) {
            focus.push(i);
            focus.push(j);
        }
        if let Some((i, j)) = worst_display_rgb_pair(oklab) {
            focus.push(i);
            focus.push(j);
        }
        if name_tail_tiebreak {
            focus.extend(duplicate_family_channel_indices(&dominant_families(
                c3, oklab,
            )));
        }
        focus.sort_unstable();
        focus.dedup();

        for &color_idx in &focus {
            if locked_colors[color_idx] {
                continue;
            }
            let base = color_idx * 3;
            let (l0, c0, h0) = oklab_to_oklch(oklab[base], oklab[base + 1], oklab[base + 2]);
            // Stronger hue moves to separate the worst pair.
            // (cost, -visible_delta) so we minimize cost then maximize visibility.
            let mut best_local: Option<(Vec<f32>, f32, f32)> = None;
            for &c_scale in &[1.0f32, 0.88, 0.75, 0.62] {
                for &ddeg in &[
                    90.0f32, -90.0, 60.0, -60.0, 45.0, -45.0, 30.0, -30.0, 20.0, -20.0, 12.0, -12.0,
                ] {
                    let mut trial = oklab.clone();
                    if !try_set_channel_oklch(
                        &mut trial,
                        color_idx,
                        l0,
                        (c0 * c_scale).max(DEFAULT_MIN_OKLAB_CHROMA),
                        h0 + ddeg.to_radians(),
                        luminance_values,
                        &mut rng,
                    ) {
                        stats.rejected += 1;
                        continue;
                    }
                    let trial_cost = objective_total(
                        c3,
                        &trial,
                        intensity_arc,
                        avg_confusion,
                        spatial_w,
                        excluded_set,
                        color_name_indices,
                    );
                    if should_accept_polar(
                        c3,
                        oklab,
                        &trial,
                        best_cost,
                        trial_cost,
                        name_tail_tiebreak,
                    ) {
                        let vis = max_channel_display_delta(oklab, &trial);
                        let better = best_local.as_ref().map_or(true, |(_, c, v)| {
                            if name_tail_tiebreak {
                                // Prefer visible family escapes over tiny cost wins.
                                vis > *v + 1.0 || (vis + 1.0 >= *v && trial_cost < *c)
                            } else {
                                trial_cost < *c
                            }
                        });
                        if better {
                            best_local = Some((trial, trial_cost, vis));
                        }
                    } else {
                        stats.rejected += 1;
                    }
                }
            }
            if let Some((trial, trial_cost, _)) = best_local {
                *oklab = trial;
                best_cost = trial_cost;
                stats.accepted += 1;
                stats.pair_targeted_accepted += 1;
            }
        }
    }

    let mut fin = StdRng::seed_from_u64(rng_seed.wrapping_add(0x701A_5EED));
    for color_idx in 0..n {
        enforce_channel_saturation(oklab, color_idx, &mut fin);
    }
    stats
}

/// Apply the selected refine mode after (or instead of) Cartesian postprocess pieces.
///
/// - [`Cartesian`]: `polish` (if `do_polish`) + Cartesian jitter refine.
/// - [`Polar`]: optional polish, then polar OKLCh refine (with pair targeting).
/// - [`Hybrid`]: polish + Cartesian refine, then polar pair-targeted refine.
#[allow(clippy::too_many_arguments)]
pub fn apply_palette_refine(
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
    mode: PaletteRefineMode,
    do_polish: bool,
    polish_fast: bool,
) -> PolarRefineStats {
    apply_palette_refine_ex(
        oklab,
        locked_colors,
        luminance_values,
        c3,
        intensity_arc,
        avg_confusion,
        spatial_w,
        excluded_set,
        color_name_indices,
        rng_seed,
        mode,
        do_polish,
        polish_fast,
        false,
    )
}

/// Like [`apply_palette_refine`], with optional name-tail tiebreak for polar moves (study review).
#[allow(clippy::too_many_arguments)]
pub fn apply_palette_refine_ex(
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
    mode: PaletteRefineMode,
    do_polish: bool,
    polish_fast: bool,
    name_tail_tiebreak: bool,
) -> PolarRefineStats {
    let stats = match mode {
        PaletteRefineMode::Cartesian => {
            if do_polish {
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
                    polish_fast,
                );
            }
            refine_oklab_palette(
                oklab,
                locked_colors,
                luminance_values,
                c3,
                intensity_arc,
                avg_confusion,
                spatial_w,
                excluded_set,
                color_name_indices,
                rng_seed,
            );
            PolarRefineStats::default()
        }
        PaletteRefineMode::Polar => {
            if do_polish {
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
                    polish_fast,
                );
            }
            refine_oklch_palette(
                oklab,
                locked_colors,
                luminance_values,
                c3,
                intensity_arc,
                avg_confusion,
                spatial_w,
                excluded_set,
                color_name_indices,
                rng_seed,
                true,
                name_tail_tiebreak,
            )
        }
        PaletteRefineMode::Hybrid => {
            if do_polish {
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
                    polish_fast,
                );
            }
            refine_oklab_palette(
                oklab,
                locked_colors,
                luminance_values,
                c3,
                intensity_arc,
                avg_confusion,
                spatial_w,
                excluded_set,
                color_name_indices,
                rng_seed,
            );
            refine_oklch_palette(
                oklab,
                locked_colors,
                luminance_values,
                c3,
                intensity_arc,
                avg_confusion,
                spatial_w,
                excluded_set,
                color_name_indices,
                rng_seed.wrapping_add(0xB01A5),
                true,
                name_tail_tiebreak,
            )
        }
    };
    crate::palette_solvers::clamp_oklab_to_luminance_bounds(oklab, luminance_values);
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklch_round_trip() {
        let (l, a, b) = (0.58f32, 0.12, -0.08);
        let (ol, c, h) = oklab_to_oklch(l, a, b);
        let (l2, a2, b2) = oklch_to_oklab(ol, c, h);
        assert!((l - l2).abs() < 1e-5);
        assert!((a - a2).abs() < 1e-5);
        assert!((b - b2).abs() < 1e-5);
    }

    #[test]
    fn parse_refine_modes() {
        assert_eq!(
            PaletteRefineMode::parse("hybrid"),
            Some(PaletteRefineMode::Hybrid)
        );
        assert_eq!(
            PaletteRefineMode::parse("total"),
            Some(PaletteRefineMode::Cartesian)
        );
    }

    #[test]
    fn polar_tiebreak_can_move_red_pink() {
        use crate::c3::C3;
        use ndarray::Array2;
        use std::sync::Arc;

        // Two similar warm colors + a cool one so pair separation has room.
        let mut oklab = vec![
            0.55, 0.18, 0.08, // reddish
            0.62, 0.16, 0.04, // pinkish
            0.55, -0.10, -0.12, // bluish
        ];
        let before = oklab.clone();
        let c3 = C3::new();
        let intensity = Arc::new(Array2::<f32>::zeros((8, 3)));
        let locked = vec![false, false, false];
        let lum = vec![0.50f32, 0.92];
        let excl = HashSet::new();
        let names = vec![-1.0f32, -1.0, -1.0];
        let stats = refine_oklch_palette(
            &mut oklab, &locked, &lum, &c3, &intensity, 1.0, 0.0, &excl, &names, 42, true, true,
        );

        assert!(
            stats.accepted > 0 || oklab != before,
            "expected polar tiebreak to accept at least one move on a close warm pair; stats={stats:?}"
        );
    }

    #[test]
    fn polar_tiebreak_separates_primary_red_pink() {
        use crate::c3::C3;
        use ndarray::Array2;
        use palette::{FromColor, Oklab, Srgb};
        use std::sync::Arc;

        // Gamut-corner 6-ch palette with red + magenta (same coarse family).
        let srgb = [
            [1.0f32, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.71, 0.0],
        ];
        let mut oklab = Vec::new();
        for rgb in &srgb {
            let o: Oklab = Oklab::from_color(Srgb::new(rgb[0], rgb[1], rgb[2]));
            oklab.extend_from_slice(&[o.l, o.a, o.b]);
        }
        let c3 = C3::new();
        let before_dup = duplicate_family_count(&dominant_families(&c3, &oklab));
        assert!(
            before_dup > 0,
            "fixture should start with a family collision"
        );
        let intensity = Arc::new(Array2::<f32>::zeros((8, 6)));
        let locked = vec![false; 6];
        let lum = vec![0.50f32, 0.92];
        let excl = HashSet::new();
        let names = vec![-1.0f32; 6];
        let stats = refine_oklch_palette(
            &mut oklab, &locked, &lum, &c3, &intensity, 1.0, 0.0, &excl, &names, 99, true, true,
        );
        let after_dup = duplicate_family_count(&dominant_families(&c3, &oklab));
        assert!(
            stats.accepted > 0 && after_dup < before_dup,
            "expected polar family-dup tiebreak to separate red/pink; stats={:?} dup {}->{}",
            stats,
            before_dup,
            after_dup
        );
    }
}
