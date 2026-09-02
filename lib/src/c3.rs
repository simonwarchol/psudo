//! C3 color naming — [`rust_c3`] 0.3+.
//!
//! [`rust_c3::C3::new()`] loads compile-time embedded `c3_*.npy` (native and WASM).

use ndarray::Array2;
use rust_c3::C3_TERM_STRS;
use std::sync::{Arc, OnceLock};

static SHARED: OnceLock<Arc<rust_c3::C3>> = OnceLock::new();

fn shared_inner() -> Arc<rust_c3::C3> {
    SHARED.get_or_init(|| Arc::new(rust_c3::C3::new())).clone()
}

/// Psudo-facing C3 handle (cheap clone via inner `Arc`).
#[derive(Clone)]
pub struct C3 {
    inner: Arc<rust_c3::C3>,
}

impl C3 {
    pub fn new() -> Self {
        Self {
            inner: shared_inner(),
        }
    }

    #[cfg(test)]
    pub(crate) fn color_index(&self, lab: [f64; 3]) -> usize {
        self.inner.color_index(lab)
    }

    pub fn analyze_palette(&self, palette: Array2<f64>) -> Vec<rust_c3::ColorSample> {
        self.inner.analyze_palette(palette)
    }

    pub fn get_palette_terms(
        &self,
        palette: Array2<f64>,
        color_term_limit: usize,
    ) -> Vec<Vec<rust_c3::RelatedTerm>> {
        self.inner.get_palette_terms(palette, color_term_limit)
    }

    /// One KD lookup per channel: color index + top terms (no `Array2` build).
    /// The objective uses only `ColorSample.c`, so entropy is deliberately skipped.
    pub fn fill_palette_c3(
        &self,
        labs: &[[f64; 3]],
        color_term_limit: usize,
        samples: &mut Vec<rust_c3::ColorSample>,
        terms: &mut Vec<Vec<rust_c3::RelatedTerm>>,
    ) {
        self.inner
            .fill_lab_rows(labs, color_term_limit, samples, terms);
    }

    /// Pairwise color-name distance (`1 - cosine_similarity`) for every unordered pair.
    /// Returns `(i, j, distance)` with `i < j`.
    pub fn pairwise_color_name_distances(
        &self,
        data: &[rust_c3::ColorSample],
    ) -> Vec<(usize, usize, f64)> {
        self.inner.pairwise_color_name_distances(data)
    }

    /// Average pairwise color-name distance (`1 - cosine_similarity`), lower triangle only.
    pub fn average_pairwise_color_name_distance(&self, data: &[rust_c3::ColorSample]) -> f64 {
        self.inner.average_pairwise_color_name_distance(data)
    }

    /// Minimum pairwise color-name distance (`1 - cosine_similarity`).
    pub fn min_pairwise_color_name_distance(&self, data: &[rust_c3::ColorSample]) -> Option<f64> {
        self.inner.min_pairwise_color_name_distance(data)
    }

    /// Cosine similarity between two C3 color indices (for seed composite distances).
    pub fn color_cosine(&self, a: usize, b: usize) -> f64 {
        self.inner.color_cosine(a, b)
    }

    /// Dominant term + score from the same term vector used by the objective
    /// (`color_related_terms` with limit, no salience filter).
    pub fn top_term_for_lab(
        &self,
        lab: [f64; 3],
        limit: usize,
    ) -> Option<(usize, f64, &'static str)> {
        let sample = self.inner.color(lab);
        let terms = self
            .inner
            .color_related_terms(sample.c, Some(limit), None, None);
        terms
            .first()
            .map(|t| (t.index, t.score, C3_TERM_STRS[t.index]))
    }

    /// Full related-term list for one Lab (objective-aligned filters).
    pub fn related_terms_for_lab(
        &self,
        lab: [f64; 3],
        limit: usize,
    ) -> (rust_c3::ColorSample, Vec<rust_c3::RelatedTerm>) {
        let sample = self.inner.color(lab);
        let terms = self
            .inner
            .color_related_terms(sample.c, Some(limit), None, None);
        (sample, terms)
    }

    /// Top C3 color-name label for a CIELAB point (for debugging / UI).
    /// Prefer display-referred Lab (gamut-clipped sRGB→Lab) so labels match swatches.
    pub fn dominant_term_name(&self, lab: [f64; 3]) -> String {
        let c = self.inner.color_index(lab);
        let related = self
            .inner
            .color_related_terms(c, Some(1), Some(10), Some(0.1));
        related
            .first()
            .map(|t| C3_TERM_STRS[t.index].to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    pub fn get_term_index(&self, term: &str) -> Option<usize> {
        C3_TERM_STRS.iter().position(|&s| s == term)
    }
}

pub use rust_c3::{ColorSample, RelatedTerm};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_matches_known_color_index() {
        let c3 = C3::new();
        assert_eq!(c3.color_index([60.3, 98.2, -60.8]), 7271);
    }

    #[test]
    fn singleton_reuses_inner() {
        let a = C3::new();
        let b = C3::new();
        assert_eq!(
            a.color_index([60.3, 98.2, -60.8]),
            b.color_index([60.3, 98.2, -60.8])
        );
    }

    #[test]
    fn rust_c3_embedded_npy_loads() {
        let _ = rust_c3::C3::try_new().expect("embedded c3_*.npy");
    }

    #[test]
    fn direct_average_matches_pairwise_order() {
        let c3 = C3::new();
        let samples = vec![
            rust_c3::ColorSample {
                c: c3.color_index([60.3, 98.2, -60.8]),
                h: 0.0,
            },
            rust_c3::ColorSample {
                c: c3.color_index([80.0, -70.0, 60.0]),
                h: 0.0,
            },
            rust_c3::ColorSample {
                c: c3.color_index([35.0, 50.0, 40.0]),
                h: 0.0,
            },
        ];
        let pairs = c3.pairwise_color_name_distances(&samples);
        let expected =
            pairs.iter().map(|(_, _, distance)| distance).sum::<f64>() / pairs.len() as f64;
        assert_eq!(
            c3.average_pairwise_color_name_distance(&samples).to_bits(),
            expected.to_bits()
        );
    }
}
