//! Fast CPU path for [`crate::evaluate_palette_objective_breakdown_with_excluded_set`].

use crate::c3::{self, RelatedTerm};
use crate::palette_objective::{MIN_OKLAB_DISTANCE, OKLAB_PERCEPTUAL_SCALE};
use crate::{term_loss, MIN_DISPLAY_RGB_DISTANCE, PERCEPTUAL_DEFICIT_WEIGHT, PERCEPTUAL_SCALE};
use ndarray::Array2;
use palette::{FromColor, Lab, Oklab, Srgb};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::sync::Arc;

const C3_TERM_LIMIT: usize = 10;

/// Reusable buffers for one objective evaluation (OKLab param → scalar loss).
pub struct PaletteEvalScratch {
    /// Display-referred Lab (gamut-clipped sRGB→Lab) for C3 naming.
    pub c3_labs: Vec<[f64; 3]>,
    pub display_rgb: Vec<[f64; 3]>,
    pub projected_oklab: Vec<f32>,
    pub srgb_saturations: Vec<f32>,
    pub projected_hues: Vec<f32>,
    pub projected_chromas: Vec<f32>,
    pub c3_samples: Vec<c3::ColorSample>,
    pub palette_terms: Vec<Vec<RelatedTerm>>,
    /// Spatial confusion: mixed OKLab rows (n_rows × 3).
    pub mixed_oklab: Array2<f32>,
}

impl PaletteEvalScratch {
    pub fn new() -> Self {
        Self {
            c3_labs: Vec::new(),
            display_rgb: Vec::new(),
            projected_oklab: Vec::new(),
            srgb_saturations: Vec::new(),
            projected_hues: Vec::new(),
            projected_chromas: Vec::new(),
            c3_samples: Vec::new(),
            palette_terms: Vec::new(),
            mixed_oklab: Array2::zeros((0, 3)),
        }
    }
}

thread_local! {
    static EVAL_SCRATCH: RefCell<PaletteEvalScratch> = RefCell::new(PaletteEvalScratch::new());
    static CLAMPED_PARAM: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    static OBJECTIVE_EVAL_COUNT: Cell<u64> = const { Cell::new(0) };
    static OBJECTIVE_EVAL_COUNTING: Cell<bool> = const { Cell::new(false) };
}

pub fn with_eval_scratch<R>(f: impl FnOnce(&mut PaletteEvalScratch) -> R) -> R {
    EVAL_SCRATCH.with(|cell| f(&mut cell.borrow_mut()))
}

pub fn with_clamped_param<R>(param: &[f32], f: impl FnOnce(&mut Vec<f32>) -> R) -> R {
    CLAMPED_PARAM.with(|cell| {
        let mut clamped = cell.borrow_mut();
        clamped.clear();
        clamped.extend_from_slice(param);
        f(&mut clamped)
    })
}

pub fn objective_eval_count() -> u64 {
    OBJECTIVE_EVAL_COUNT.with(Cell::get)
}

pub fn with_objective_eval_counting<R>(f: impl FnOnce() -> R) -> R {
    OBJECTIVE_EVAL_COUNTING.with(|enabled| {
        let previous = enabled.replace(true);
        let result = f();
        enabled.set(previous);
        result
    })
}

fn record_objective_eval() {
    OBJECTIVE_EVAL_COUNTING.with(|enabled| {
        if enabled.get() {
            OBJECTIVE_EVAL_COUNT.with(|count| count.set(count.get().wrapping_add(1)));
        }
    });
}

/// CIELAB of the gamut-clipped display color — what C3 (and the eye) actually see.
#[inline]
pub fn lab_from_display_clipped_oklab(l: f32, a: f32, b: f32) -> [f64; 3] {
    let okl = Oklab::new(l, a, b);
    let rgb: Srgb = Srgb::from_color(okl);
    let clipped = Srgb::new(
        rgb.red.clamp(0.0, 1.0),
        rgb.green.clamp(0.0, 1.0),
        rgb.blue.clamp(0.0, 1.0),
    );
    let lab: Lab = Lab::from_color(clipped);
    [lab.l as f64, lab.a as f64, lab.b as f64]
}

#[inline]
pub fn fill_c3_labs_from_oklab(oklab_flat: &[f32], labs: &mut Vec<[f64; 3]>) {
    let n = oklab_flat.len() / 3;
    labs.clear();
    labs.reserve(n);
    for ch in oklab_flat.chunks(3) {
        labs.push(lab_from_display_clipped_oklab(ch[0], ch[1], ch[2]));
    }
}

#[inline]
pub fn fill_display_srgb255(oklab_flat: &[f32], out: &mut Vec<[f64; 3]>) {
    let n = oklab_flat.len() / 3;
    out.clear();
    out.reserve(n);
    for ch in oklab_flat.chunks(3) {
        let okl = Oklab::new(ch[0], ch[1], ch[2]);
        let rgb: Srgb = Srgb::from_color(okl);
        out.push([
            (rgb.red.clamp(0.0, 1.0) * 255.0) as f64,
            (rgb.green.clamp(0.0, 1.0) * 255.0) as f64,
            (rgb.blue.clamp(0.0, 1.0) * 255.0) as f64,
        ]);
    }
}

#[inline]
fn fill_display_features_from_oklab(
    oklab_flat: &[f32],
    c3_labs: &mut Vec<[f64; 3]>,
    display_rgb: &mut Vec<[f64; 3]>,
    projected_oklab: &mut Vec<f32>,
    srgb_saturations: &mut Vec<f32>,
) {
    let n = oklab_flat.len() / 3;
    c3_labs.clear();
    display_rgb.clear();
    projected_oklab.clear();
    srgb_saturations.clear();
    c3_labs.reserve(n);
    display_rgb.reserve(n);
    projected_oklab.reserve(oklab_flat.len());
    srgb_saturations.reserve(n);
    for ch in oklab_flat.chunks(3) {
        let okl = Oklab::new(ch[0], ch[1], ch[2]);
        let rgb: Srgb = Srgb::from_color(okl);
        let clipped = Srgb::new(
            rgb.red.clamp(0.0, 1.0),
            rgb.green.clamp(0.0, 1.0),
            rgb.blue.clamp(0.0, 1.0),
        );
        let lab: Lab = Lab::from_color(clipped);
        let projected: Oklab = Oklab::from_color(clipped);
        let maximum = rgb.red.max(rgb.green).max(rgb.blue);
        let minimum = rgb.red.min(rgb.green).min(rgb.blue);
        c3_labs.push([lab.l as f64, lab.a as f64, lab.b as f64]);
        display_rgb.push([
            (clipped.red * 255.0) as f64,
            (clipped.green * 255.0) as f64,
            (clipped.blue * 255.0) as f64,
        ]);
        projected_oklab.push(projected.l);
        projected_oklab.push(projected.a);
        projected_oklab.push(projected.b);
        srgb_saturations.push(if maximum < 1e-5 {
            0.0
        } else {
            (maximum - minimum) / maximum
        });
    }
}

#[inline]
fn rgb_pair_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    (dr * dr + dg * dg + db * db).sqrt()
}

pub fn perceptual_objective_terms_from_display(display_rgb: &[[f64; 3]]) -> (f32, f32, f32) {
    let n = display_rgb.len();
    if n < 2 {
        return (0.0, 0.0, 0.0);
    }
    let mut min_dist = f64::MAX;
    let mut deficit_sq = 0.0f64;
    for i in 0..n {
        for j in i + 1..n {
            let d = rgb_pair_distance(display_rgb[i], display_rgb[j]);
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

/// Project OKLab through clamped display sRGB and back so out-of-gamut duplicates
/// (same clipped hex, wild preimage `a,b`) do not get a free distance reward.
pub fn project_oklab_through_display(oklab_flat: &[f32], out: &mut Vec<f32>) {
    out.clear();
    out.reserve(oklab_flat.len());
    for ch in oklab_flat.chunks(3) {
        let okl = Oklab::new(ch[0], ch[1], ch[2]);
        let rgb: Srgb = Srgb::from_color(okl);
        let clipped = Srgb::new(
            rgb.red.clamp(0.0, 1.0),
            rgb.green.clamp(0.0, 1.0),
            rgb.blue.clamp(0.0, 1.0),
        );
        let back: Oklab = Oklab::from_color(clipped);
        out.push(back.l);
        out.push(back.a);
        out.push(back.b);
    }
}

/// Min pairwise OKLab Euclidean + deficit below [`MIN_OKLAB_DISTANCE`] (study `oklab_sep`).
/// Distances use display-projected OKLab (see [`project_oklab_through_display`]).
pub fn perceptual_objective_terms_from_oklab(oklab_flat: &[f32]) -> (f32, f32, f32) {
    let n = oklab_flat.len() / 3;
    if n < 2 {
        return (0.0, 0.0, 0.0);
    }
    let mut projected = Vec::with_capacity(oklab_flat.len());
    project_oklab_through_display(oklab_flat, &mut projected);
    perceptual_objective_terms_from_oklab_raw(&projected)
}

fn perceptual_objective_terms_from_oklab_raw(oklab_flat: &[f32]) -> (f32, f32, f32) {
    let n = oklab_flat.len() / 3;
    if n < 2 {
        return (0.0, 0.0, 0.0);
    }
    let mut min_dist = f64::INFINITY;
    let mut deficit_sq = 0.0f64;
    let mut pairs = 0usize;
    for i in 0..n {
        let bi = i * 3;
        for j in i + 1..n {
            let bj = j * 3;
            let dl = (oklab_flat[bi] - oklab_flat[bj]) as f64;
            let da = (oklab_flat[bi + 1] - oklab_flat[bj + 1]) as f64;
            let db = (oklab_flat[bi + 2] - oklab_flat[bj + 2]) as f64;
            let d = (dl * dl + da * da + db * db).sqrt();
            if !d.is_finite() {
                continue;
            }
            min_dist = min_dist.min(d);
            let deficit = (MIN_OKLAB_DISTANCE - d).max(0.0);
            deficit_sq += deficit * deficit;
            pairs += 1;
        }
    }
    if pairs == 0 || !min_dist.is_finite() {
        // Non-finite params: heavy but finite penalty so solvers do not go NaN.
        return (0.0, 50.0, 0.0);
    }
    let scale = OKLAB_PERCEPTUAL_SCALE;
    let minus_min = (-(min_dist / scale) as f32).clamp(-10.0, 0.0);
    let deficit_penalty =
        (deficit_sq / (scale * scale) * PERCEPTUAL_DEFICIT_WEIGHT as f64).clamp(0.0, 50.0) as f32;
    (minus_min, deficit_penalty, min_dist as f32)
}

/// Display-projected OKLab Euclidean distance between two channels (diagnostic / tests).
pub fn oklab_pair_distance(oklab_flat: &[f32], i: usize, j: usize) -> f64 {
    let mut projected = Vec::with_capacity(oklab_flat.len());
    project_oklab_through_display(oklab_flat, &mut projected);
    let bi = i * 3;
    let bj = j * 3;
    let dl = (projected[bi] - projected[bj]) as f64;
    let da = (projected[bi + 1] - projected[bj + 1]) as f64;
    let db = (projected[bi + 2] - projected[bj + 2]) as f64;
    (dl * dl + da * da + db * db).sqrt()
}

pub fn evaluate_objective_fast(
    c3: &c3::C3,
    oklab_flat: &[f32],
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_confusion_weight: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32],
    scratch: &mut PaletteEvalScratch,
) -> crate::PaletteObjectiveBreakdown {
    record_objective_eval();
    fill_display_features_from_oklab(
        oklab_flat,
        &mut scratch.c3_labs,
        &mut scratch.display_rgb,
        &mut scratch.projected_oklab,
        &mut scratch.srgb_saturations,
    );

    c3.fill_palette_c3(
        &scratch.c3_labs,
        C3_TERM_LIMIT,
        &mut scratch.c3_samples,
        &mut scratch.palette_terms,
    );

    let average_cosine_distance = c3.average_pairwise_color_name_distance(&scratch.c3_samples);
    let min_name_w = crate::current_min_name_weight();
    let minus_min_name = if min_name_w > 0.0 {
        match c3.min_pairwise_color_name_distance(&scratch.c3_samples) {
            Some(min_pair) => -min_name_w * min_pair as f32,
            None => 0.0,
        }
    } else {
        0.0
    };
    let term = term_loss(&scratch.palette_terms, excluded_set, color_name_indices);

    let minus_mean = -average_cosine_distance as f32;
    let (minus_min_perceptual, perceptual_deficit_penalty, min_display_rgb_distance) =
        if crate::current_objective_mode().uses_oklab_separation() {
            let (m, p, _) = perceptual_objective_terms_from_oklab(oklab_flat);
            // Keep display RGB min as a diagnostic even when OKLab drives the objective.
            let (_, _, disp_min) = perceptual_objective_terms_from_display(&scratch.display_rgb);
            (m, p, disp_min)
        } else {
            perceptual_objective_terms_from_display(&scratch.display_rgb)
        };
    let (hue_separation_reward, hue_separation_deficit, min_hue_gap_deg) =
        crate::hue_separation_terms_from_projected_with_scratch(
            &scratch.projected_oklab,
            crate::HUE_SEPARATION_WEIGHT,
            &mut scratch.projected_hues,
            &mut scratch.projected_chromas,
        );
    let (minus_min_saturation, saturation_deficit_penalty, min_srgb_saturation, min_oklab_chroma) =
        crate::saturation_objective_terms_from_features(oklab_flat, &scratch.srgb_saturations);

    let mut confusion_weighted = 0.0f32;
    if spatial_confusion_weight > 0.0 {
        confusion_weighted = spatial_confusion_weight
            * crate::compute_confusion_loss_fast(
                oklab_flat,
                intensity_arc.as_ref(),
                avg_confusion,
                &mut scratch.mixed_oklab,
            );
    }

    let total = minus_mean
        + minus_min_name
        + minus_min_perceptual
        + perceptual_deficit_penalty
        + hue_separation_reward
        + hue_separation_deficit
        + term
        + confusion_weighted
        + minus_min_saturation
        + saturation_deficit_penalty;

    crate::PaletteObjectiveBreakdown {
        total,
        minus_mean_color_name_distance: minus_mean,
        minus_min_color_name_distance: minus_min_name,
        minus_min_perceptual_distance: minus_min_perceptual,
        perceptual_deficit_penalty,
        min_display_rgb_distance,
        hue_separation_reward,
        hue_separation_deficit,
        min_hue_gap_deg,
        term_loss: term,
        confusion_weighted,
        minus_min_saturation,
        saturation_deficit_penalty,
        min_srgb_saturation,
        min_oklab_chroma,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fused_display_features_match_separate_conversions() {
        let colors = vec![0.58, 0.22, 0.11, 0.72, -0.18, 0.14, 0.43, 0.08, -0.25];
        let mut expected_labs = Vec::new();
        let mut expected_rgb = Vec::new();
        fill_c3_labs_from_oklab(&colors, &mut expected_labs);
        fill_display_srgb255(&colors, &mut expected_rgb);
        let mut expected_projected = Vec::new();
        project_oklab_through_display(&colors, &mut expected_projected);
        let expected_saturations: Vec<f32> = colors
            .chunks(3)
            .map(|color| crate::channel_srgb_saturation(color[0], color[1], color[2]))
            .collect();

        let mut actual_labs = Vec::new();
        let mut actual_rgb = Vec::new();
        let mut actual_projected = Vec::new();
        let mut actual_saturations = Vec::new();
        fill_display_features_from_oklab(
            &colors,
            &mut actual_labs,
            &mut actual_rgb,
            &mut actual_projected,
            &mut actual_saturations,
        );

        assert_eq!(actual_labs, expected_labs);
        assert_eq!(actual_rgb, expected_rgb);
        assert_eq!(actual_projected, expected_projected);
        assert_eq!(actual_saturations, expected_saturations);
        assert_eq!(
            crate::saturation_objective_terms_from_features(&colors, &actual_saturations),
            crate::saturation_objective_terms(&colors),
        );
        assert_eq!(
            crate::hue_separation_terms_from_projected(
                &actual_projected,
                crate::HUE_SEPARATION_WEIGHT,
            ),
            crate::hue_separation_terms(&colors, crate::HUE_SEPARATION_WEIGHT),
        );
        let mut hues = Vec::new();
        let mut chromas = Vec::new();
        assert_eq!(
            crate::hue_separation_terms_from_projected_with_scratch(
                &actual_projected,
                crate::HUE_SEPARATION_WEIGHT,
                &mut hues,
                &mut chromas,
            ),
            crate::hue_separation_terms(&colors, crate::HUE_SEPARATION_WEIGHT),
        );
    }
}
