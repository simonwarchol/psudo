//! C3 color naming — [`rust_c3`] 0.3+.
//!
//! [`rust_c3::C3::new()`] loads compile-time embedded `c3_*.npy` (native and WASM).

use ndarray::Array2;
use rust_c3::C3_TERM_STRS;
use std::sync::{Arc, OnceLock};

static SHARED: OnceLock<Arc<rust_c3::C3>> = OnceLock::new();

fn shared_inner() -> Arc<rust_c3::C3> {
    SHARED
        .get_or_init(|| Arc::new(rust_c3::C3::new()))
        .clone()
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

    /// Average pairwise color-name distance (`1 - cosine_similarity`), lower triangle only.
    pub fn average_pairwise_color_name_distance(&self, data: &[rust_c3::ColorSample]) -> f64 {
        let n = data.len();
        if n < 2 {
            return 0.0;
        }
        let mut total_distance = 0.0;
        let mut total_pairs = 0usize;
        for i in 0..n {
            let ci = data[i].c;
            for j in 0..i {
                let cj = data[j].c;
                total_distance += 1.0 - self.inner.color_cosine(ci, cj);
                total_pairs += 1;
            }
        }
        total_distance / (total_pairs as f64)
    }

    /// Top C3 color-name label for a CIELAB point (for debugging / UI).
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
        assert_eq!(a.color_index([60.3, 98.2, -60.8]), b.color_index([60.3, 98.2, -60.8]));
    }

    #[test]
    fn rust_c3_embedded_npy_loads() {
        let _ = rust_c3::C3::try_new().expect("embedded c3_*.npy");
    }
}
