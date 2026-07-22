//! Parity checks after `rust_c3` migration (run with `cargo test c3_migration --release`).

#[cfg(test)]
mod tests {
    use crate::c3::C3;
    use crate::evaluate_palette_objective_breakdown;
    use ndarray::Array2;
    use palette::{FromColor, Lab, Oklab};
    use std::sync::Arc;

    /// Golden index (full C3 dataset via `rust_c3` embedded `.npy`).
    #[test]
    fn known_lab_maps_to_color_index_7271() {
        let c3 = C3::new();
        assert_eq!(c3.color_index([60.3, 98.2, -60.8]), 7271);
    }

    #[test]
    fn fast_eval_matches_breakdown_api() {
        use crate::palette_eval::{evaluate_objective_fast, with_eval_scratch};
        use std::collections::HashSet;
        let c3 = C3::new();
        let mut oklab = Vec::with_capacity(18);
        let l = 0.58f32;
        for i in 0..6 {
            let angle = std::f32::consts::TAU * (i as f32) / 6.0;
            let chroma = 0.28f32;
            let okl = Oklab::new(l, chroma * angle.cos(), chroma * angle.sin());
            oklab.extend([okl.l, okl.a, okl.b]);
        }
        let intensity = Arc::new(Array2::<f32>::zeros((384, 6)));
        let names = [-1.0; 6];
        let direct =
            evaluate_palette_objective_breakdown(&c3, &oklab, &intensity, 1.0, 0.0, &[], &names);
        let fast = with_eval_scratch(|s| {
            evaluate_objective_fast(
                &c3,
                &oklab,
                &intensity,
                1.0,
                0.0,
                &HashSet::new(),
                &names,
                s,
            )
        });
        assert!(
            (direct.total - fast.total).abs() < 1e-4,
            "{} vs {}",
            direct.total,
            fast.total
        );
    }

    /// Six-channel spread palette: objective should be finite and in a stable band.
    #[test]
    fn six_channel_objective_in_expected_band() {
        let c3 = C3::new();
        let mut oklab = Vec::with_capacity(18);
        let l = 0.58f32;
        for i in 0..6 {
            let angle = std::f32::consts::TAU * (i as f32) / 6.0;
            let chroma = 0.28f32;
            let okl = Oklab::new(l, chroma * angle.cos(), chroma * angle.sin());
            oklab.extend([okl.l, okl.a, okl.b]);
        }
        let intensity = Arc::new(Array2::<f32>::zeros((384, 6)));
        let bd = evaluate_palette_objective_breakdown(
            &c3,
            &oklab,
            &intensity,
            1.0,
            0.0,
            &[],
            &[-1.0; 6],
        );
        assert!(bd.total.is_finite());
        assert!(bd.total < -1.0 && bd.total > -6.0, "L_tot={}", bd.total);
        assert!(bd.min_display_rgb_distance > 80.0);
    }

    #[test]
    fn dominant_term_for_saturated_red() {
        let c3 = C3::new();
        let okl = Oklab::new(0.58, 0.22, 0.12);
        let lab: Lab = Lab::from_color(okl);
        let name = c3.dominant_term_name([lab.l as f64, lab.a as f64, lab.b as f64]);
        assert!(
            name == "red" || name == "pink" || name == "orange",
            "got {name}"
        );
    }
}
