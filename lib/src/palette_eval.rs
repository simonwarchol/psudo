//! Fast CPU path for [`crate::evaluate_palette_objective_breakdown_with_excluded_set`].

use crate::c3::{self, RelatedTerm};
use crate::{
    saturation_objective_terms, term_loss, MIN_DISPLAY_RGB_DISTANCE, PERCEPTUAL_DEFICIT_WEIGHT,
    PERCEPTUAL_SCALE,
};
use ndarray::Array2;
use palette::{FromColor, Lab, Oklab, Srgb};
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

const C3_TERM_LIMIT: usize = 10;

/// Reusable buffers for one objective evaluation (OKLab param → scalar loss).
pub struct PaletteEvalScratch {
    pub labs: Vec<[f64; 3]>,
    pub display_rgb: Vec<[f64; 3]>,
    pub c3_samples: Vec<c3::ColorSample>,
    pub palette_terms: Vec<Vec<RelatedTerm>>,
    /// Spatial confusion: mixed OKLab rows (n_rows × 3).
    pub mixed_oklab: Array2<f32>,
}

impl PaletteEvalScratch {
    pub fn new() -> Self {
        Self {
            labs: Vec::new(),
            display_rgb: Vec::new(),
            c3_samples: Vec::new(),
            palette_terms: Vec::new(),
            mixed_oklab: Array2::zeros((0, 3)),
        }
    }
}

thread_local! {
    static EVAL_SCRATCH: RefCell<PaletteEvalScratch> = RefCell::new(PaletteEvalScratch::new());
}

pub fn with_eval_scratch<R>(f: impl FnOnce(&mut PaletteEvalScratch) -> R) -> R {
    EVAL_SCRATCH.with(|cell| f(&mut cell.borrow_mut()))
}

#[inline]
pub fn fill_labs_from_oklab(oklab_flat: &[f32], labs: &mut Vec<[f64; 3]>) {
    let n = oklab_flat.len() / 3;
    labs.clear();
    labs.reserve(n);
    for ch in oklab_flat.chunks(3) {
        let okl = Oklab::new(ch[0], ch[1], ch[2]);
        let lab: Lab = Lab::from_color(okl);
        labs.push([lab.l as f64, lab.a as f64, lab.b as f64]);
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
fn rgb_pair_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    (dr * dr + dg * dg + db * db).sqrt()
}

pub fn perceptual_objective_terms_from_display(
    display_rgb: &[[f64; 3]]
) -> (f32, f32, f32) {
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
    let deficit_penalty =
        (deficit_sq / (scale * scale) * PERCEPTUAL_DEFICIT_WEIGHT as f64) as f32;
    (minus_min, deficit_penalty, min_dist as f32)
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
    fill_labs_from_oklab(oklab_flat, &mut scratch.labs);
    fill_display_srgb255(oklab_flat, &mut scratch.display_rgb);

    c3.fill_palette_c3(
        &scratch.labs,
        C3_TERM_LIMIT,
        &mut scratch.c3_samples,
        &mut scratch.palette_terms,
    );

    let average_cosine_distance =
        c3.average_pairwise_color_name_distance(&scratch.c3_samples);
    let term = term_loss(
        &scratch.palette_terms,
        excluded_set,
        color_name_indices,
    );

    let minus_mean = -average_cosine_distance as f32;
    let (minus_min_perceptual, perceptual_deficit_penalty, min_display_rgb_distance) =
        perceptual_objective_terms_from_display(&scratch.display_rgb);
    let (minus_min_saturation, saturation_deficit_penalty, min_srgb_saturation, min_oklab_chroma) =
        saturation_objective_terms(oklab_flat);

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
        + minus_min_perceptual
        + perceptual_deficit_penalty
        + term
        + confusion_weighted
        + minus_min_saturation
        + saturation_deficit_penalty;

    crate::PaletteObjectiveBreakdown {
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
