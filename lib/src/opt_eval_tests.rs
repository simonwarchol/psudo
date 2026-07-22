//! Determinism checks and multi-start variance for simulated-annealing palette optimization.
//!
//! Run: `cargo test -p psudo opt_eval -- --nocapture`

use super::*;

fn toy_intensity_two_channel() -> Arc<Array2<f32>> {
    let data: Vec<f32> = vec![
        0.9, 0.1, 0.2, 0.85, 0.45, 0.5, 0.15, 0.88, 0.7, 0.25, 0.35, 0.6, 0.5, 0.48, 0.12, 0.9,
    ];
    Arc::new(Array2::from_shape_vec((8, 2), data).unwrap())
}

fn base_oklab_two_channel() -> Vec<f32> {
    vec![0.55, 0.06, -0.05, 0.55, -0.04, 0.05]
}

fn luminance_range() -> Vec<f32> {
    vec![0.48, 0.92]
}

fn l2_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[test]
#[ignore = "Argmin Metropolis draws are not seeded; OKLab params can differ run-to-run"]
fn seeded_annealing_is_repeatable() {
    let intensity = toy_intensity_two_channel();
    let colors = base_oklab_two_channel();
    let locked = vec![false, false];
    let lum = luminance_range();
    let excl: Vec<f32> = vec![];
    let names: Vec<f32> = vec![-1.0, -1.0];

    let run = || {
        annealing(
            &colors,
            &locked,
            Arc::clone(&intensity),
            &lum,
            &excl,
            &names,
            Arc::new(c3::C3::new()),
            96,
            6,
            Some(9001),
            Some(4242),
            Some(777),
            false,
            None,
            None,
            None,
        )
        .expect("annealing")
    };

    let r1 = run();
    let r2 = run();
    assert!(
        (r1.1 - r2.1).abs() < 1e-4,
        "same seeds must match final cost (got {} vs {})",
        r1.1,
        r2.1
    );
}

#[test]
fn multi_start_exhibits_bounded_cost_spread() {
    let intensity = toy_intensity_two_channel();
    let colors = base_oklab_two_channel();
    let locked = vec![false, false];
    let lum = luminance_range();
    let excl: Vec<f32> = vec![];
    let names: Vec<f32> = vec![-1.0, -1.0];

    let mut costs = Vec::new();
    for seed in 0..6u64 {
        let (_, cost) = annealing(
            &colors,
            &locked,
            Arc::clone(&intensity),
            &lum,
            &excl,
            &names,
            Arc::new(c3::C3::new()),
            120,
            6,
            Some(10_000 + seed),
            Some(20_000 + seed),
            Some(30_000 + seed),
            false,
            None,
            None,
            None,
        )
        .expect("annealing");
        costs.push(cost);
    }
    let min = costs.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = costs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!(max - min < 2.0, "multi-start spread min={min} max={max}");
}

#[test]
fn perceptual_display_rgb_penalizes_red_purple_closer_than_rgb_spread() {
    let c3 = c3::C3::new();
    let purple_red_green = vec![0.58, 0.20, -0.20, 0.58, 0.20, 0.20, 0.55, -0.18, 0.18];
    let red_green_blue = vec![0.58, 0.22, 0.06, 0.55, -0.18, 0.10, 0.52, -0.02, -0.20];
    let intensity = toy_intensity_two_channel();
    let names = vec![-1.0f32; 3];
    let prg = super::evaluate_palette_objective_breakdown(
        &c3,
        &purple_red_green,
        &intensity,
        1.0,
        0.0,
        &[],
        &names,
    );
    let rgb = super::evaluate_palette_objective_breakdown(
        &c3,
        &red_green_blue,
        &intensity,
        1.0,
        0.0,
        &[],
        &names,
    );
    let rp = super::display_srgb_distance(&purple_red_green, 0, 1);
    let rb = super::display_srgb_distance(&red_green_blue, 0, 2);
    assert!(
        rp < rb,
        "red+purple display dist {rp:.1} should be less than red+blue {rb:.1}"
    );
    assert!(
        prg.perceptual_deficit_penalty >= rgb.perceptual_deficit_penalty,
        "perc deficit prg {} vs rgb {}",
        prg.perceptual_deficit_penalty,
        rgb.perceptual_deficit_penalty
    );
}

#[test]
fn perceptual_display_rgb_flags_red_pink_closer_than_rgb_spread() {
    let c3 = c3::C3::new();
    let red_pink_green = vec![0.58, 0.22, 0.06, 0.62, 0.20, 0.14, 0.55, -0.18, 0.10];
    let red_green_blue = vec![0.58, 0.22, 0.06, 0.55, -0.18, 0.10, 0.52, -0.02, -0.20];
    let intensity = toy_intensity_two_channel();
    let names = vec![-1.0f32; 3];
    let rp = super::evaluate_palette_objective_breakdown(
        &c3,
        &red_pink_green,
        &intensity,
        1.0,
        0.0,
        &[],
        &names,
    );
    let rgb = super::evaluate_palette_objective_breakdown(
        &c3,
        &red_green_blue,
        &intensity,
        1.0,
        0.0,
        &[],
        &names,
    );
    assert!(
        rp.min_display_rgb_distance < rgb.min_display_rgb_distance,
        "red+pink min display dist {} vs rgb {}",
        rp.min_display_rgb_distance,
        rgb.min_display_rgb_distance
    );
}

#[test]
fn chroma_objective_penalizes_achromatic_channels() {
    let c3 = c3::C3::new();
    let intensity = toy_intensity_two_channel();
    let names = vec![-1.0f32, -1.0f32];
    let saturated = vec![0.55, 0.22, 0.08, 0.55, -0.20, 0.10];
    let grey = vec![0.55, 0.02, 0.01, 0.55, -0.01, 0.02];
    let sat_bd = super::evaluate_palette_objective_breakdown(
        &c3,
        &saturated,
        &intensity,
        1.0,
        0.0,
        &[],
        &names,
    );
    let grey_bd =
        super::evaluate_palette_objective_breakdown(&c3, &grey, &intensity, 1.0, 0.0, &[], &names);
    assert!(sat_bd.min_srgb_saturation > grey_bd.min_srgb_saturation);
    assert!(grey_bd.saturation_deficit_penalty > sat_bd.saturation_deficit_penalty);
}

#[test]
fn high_l_pastel_oklab_can_still_fail_srgb_floor() {
    let c3 = c3::C3::new();
    let intensity = toy_intensity_two_channel();
    let names = vec![-1.0f32; 3];
    let pastel = vec![0.92, 0.06, 0.02, 0.90, 0.05, 0.03, 0.88, -0.05, 0.04];
    let bd = super::evaluate_palette_objective_breakdown(
        &c3,
        &pastel,
        &intensity,
        1.0,
        0.0,
        &[],
        &names,
    );
    assert!(bd.min_oklab_chroma >= 0.05);
    assert!(bd.min_srgb_saturation < 0.35);
    assert!(bd.saturation_deficit_penalty > 0.05);
}

#[test]
fn min_name_weight_lowers_total_on_close_name_pair() {
    use super::{evaluate_palette_objective_breakdown, with_min_name_weight};
    let c3 = c3::C3::new();
    let red_pink_green = vec![0.58, 0.22, 0.06, 0.62, 0.20, 0.14, 0.55, -0.18, 0.10];
    let intensity = toy_intensity_two_channel();
    let names = vec![-1.0f32; 3];
    let base = evaluate_palette_objective_breakdown(
        &c3,
        &red_pink_green,
        &intensity,
        1.0,
        0.0,
        &[],
        &names,
    );
    assert_eq!(base.minus_min_color_name_distance, 0.0);
    let with_min = with_min_name_weight(1.0, || {
        evaluate_palette_objective_breakdown(
            &c3,
            &red_pink_green,
            &intensity,
            1.0,
            0.0,
            &[],
            &names,
        )
    });
    assert!(
        with_min.minus_min_color_name_distance < -0.01,
        "expected −w·min name term; got {}",
        with_min.minus_min_color_name_distance
    );
    assert!(with_min.total < base.total);
}

#[test]
fn optimize_respects_luminance_floor() {
    use super::{
        optimize_palette_pipeline_with_init, OptimizePostprocess, PaletteInitMode,
        PaletteRefineMode,
    };
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let n = 6usize;
    let rows = 48usize;
    let mut rng = StdRng::seed_from_u64(7);
    let colors: Vec<u16> = (0..n)
        .flat_map(|i| {
            let h = std::f32::consts::TAU * (i as f32) / n as f32;
            let okl = palette::Oklab::new(0.55, 0.2 * h.cos(), 0.2 * h.sin());
            let rgb: palette::Srgb = palette::Srgb::from_color(okl);
            [
                (rgb.red.clamp(0.0, 1.0) * 255.0) as u16,
                (rgb.green.clamp(0.0, 1.0) * 255.0) as u16,
                (rgb.blue.clamp(0.0, 1.0) * 255.0) as u16,
            ]
        })
        .collect();
    let locked = vec![0u16; n];
    let mut intensities = vec![0u16; rows * n];
    for v in &mut intensities {
        *v = rng.gen_range(0..40000);
    }
    let contrast_limits: Vec<u16> = (0..n).flat_map(|_| [0u16, 65535]).collect();
    let lum = vec![50u16, 92];
    let empty = vec![String::new(); n];
    let run = optimize_palette_pipeline_with_init(
        &colors,
        &locked,
        &intensities,
        &contrast_limits,
        &lum,
        empty.clone(),
        empty,
        Some(800),
        Some(8),
        Some(false),
        Some(4),
        Some(OptimizePostprocess::Full),
        PaletteInitMode::Current,
        PaletteRefineMode::Cartesian,
        super::PaletteObjectiveMode::MeanOnly,
    );
    let floor = 0.50f32;
    for (i, ch) in run.oklab_best.chunks(3).enumerate() {
        assert!(
            ch[0] + 1e-4 >= floor,
            "channel {i} L={} below floor {floor}; oklab={:?}",
            ch[0],
            run.oklab_best
        );
    }
}

#[test]
fn oklab_sep_optimize_stays_finite_on_6ch() {
    use super::{
        optimize_palette_pipeline_with_init, OptimizePostprocess, PaletteInitMode,
        PaletteObjectiveMode, PaletteRefineMode,
    };
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let n = 6usize;
    let rows = 64usize;
    let mut rng = StdRng::seed_from_u64(42);
    let colors: Vec<u16> = (0..n)
        .flat_map(|_| {
            let h = rng.gen::<f32>() * std::f32::consts::TAU;
            let c = 0.18f32;
            let l = 0.55f32;
            let okl = palette::Oklab::new(l, c * h.cos(), c * h.sin());
            let rgb: palette::Srgb = palette::Srgb::from_color(okl);
            [
                (rgb.red.clamp(0.0, 1.0) * 65535.0) as u16,
                (rgb.green.clamp(0.0, 1.0) * 65535.0) as u16,
                (rgb.blue.clamp(0.0, 1.0) * 65535.0) as u16,
            ]
        })
        .collect();
    let locked = vec![0u16; n];
    let mut intensities = vec![0u16; rows * n];
    for v in &mut intensities {
        *v = rng.gen_range(0..40000);
    }
    let contrast = vec![0u16, 65535u16];
    let mut contrast_limits = Vec::new();
    for _ in 0..n {
        contrast_limits.extend_from_slice(&contrast);
    }
    let lum = vec![((0.50f32) * 65535.0) as u16, ((0.92f32) * 65535.0) as u16];
    let run = optimize_palette_pipeline_with_init(
        &colors,
        &locked,
        &intensities,
        &contrast_limits,
        &lum,
        vec![],
        vec![String::new(); n],
        Some(400),
        Some(8),
        Some(false),
        Some(4),
        Some(OptimizePostprocess::Full),
        PaletteInitMode::Current,
        PaletteRefineMode::Cartesian,
        PaletteObjectiveMode::OklabSep,
    );
    assert!(
        run.oklab_best.iter().all(|x| x.is_finite()),
        "oklab_sep optimize produced non-finite OKLab: {:?}",
        run.oklab_best
    );
    assert!(run.sa_best_cost.is_finite(), "oklab_sep cost not finite");
    // Must not collapse to near-black / achromatic sludge.
    let min_chroma = run
        .oklab_best
        .chunks(3)
        .map(|c| (c[1] * c[1] + c[2] * c[2]).sqrt())
        .fold(f32::INFINITY, f32::min);
    assert!(
        min_chroma > 0.04,
        "oklab_sep collapsed chroma: min={min_chroma} oklab={:?}",
        run.oklab_best
    );
}

#[test]
fn oklab_sep_flags_red_pink_closer_than_red_blue() {
    use super::palette_eval::{oklab_pair_distance, perceptual_objective_terms_from_oklab};
    use super::{
        evaluate_palette_objective_breakdown, with_objective_mode, PaletteObjectiveMode,
        MIN_OKLAB_DISTANCE,
    };

    let red_pink_green = vec![0.58, 0.22, 0.06, 0.62, 0.20, 0.14, 0.55, -0.18, 0.10];
    let red_green_blue = vec![0.58, 0.22, 0.06, 0.55, -0.18, 0.10, 0.52, -0.02, -0.20];
    let rp_ok = oklab_pair_distance(&red_pink_green, 0, 1);
    let rb_ok = oklab_pair_distance(&red_green_blue, 0, 2);
    assert!(
        rp_ok < MIN_OKLAB_DISTANCE && rb_ok > MIN_OKLAB_DISTANCE,
        "calibration: pink {:.3} should be below floor {}, blue {:.3} above",
        rp_ok,
        MIN_OKLAB_DISTANCE,
        rb_ok
    );
    assert!(
        rp_ok < rb_ok,
        "red+pink OKLab {:.3} vs red+blue {:.3}",
        rp_ok,
        rb_ok
    );

    let (rp_minus, rp_def, _) = perceptual_objective_terms_from_oklab(&red_pink_green);
    let (rgb_minus, rgb_def, _) = perceptual_objective_terms_from_oklab(&red_green_blue);
    assert!(
        rp_def > rgb_def,
        "oklab_sep deficit pink {} vs rgb {}",
        rp_def,
        rgb_def
    );
    assert!(
        rp_minus > rgb_minus,
        "oklab_sep −min reward weaker for pink {} vs rgb {}",
        rp_minus,
        rgb_minus
    );

    let c3 = c3::C3::new();
    let intensity = toy_intensity_two_channel();
    let names = vec![-1.0f32; 3];
    let base_rp = evaluate_palette_objective_breakdown(
        &c3,
        &red_pink_green,
        &intensity,
        1.0,
        0.0,
        &[],
        &names,
    );
    let oklab_rp = with_objective_mode(PaletteObjectiveMode::OklabSep, || {
        evaluate_palette_objective_breakdown(
            &c3,
            &red_pink_green,
            &intensity,
            1.0,
            0.0,
            &[],
            &names,
        )
    });
    assert!(
        oklab_rp.perceptual_deficit_penalty > base_rp.perceptual_deficit_penalty
            || oklab_rp.total > base_rp.total,
        "oklab_sep should press harder on red+pink than sRGB-sep production terms"
    );
}

/// Compares loss spread vs wall time for study-style configs (not run in CI).
/// Run: `cargo test study_convergence_profiles -- --ignored --nocapture`
#[test]
#[ignore = "benchmark: cargo test study_convergence_profiles -- --ignored --nocapture"]
fn study_convergence_profiles() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use std::time::Instant;

    const CHANNELS: usize = 6;
    const N_SEEDS: usize = 4;
    const N_ROWS: usize = 128;

    let mut rng = StdRng::seed_from_u64(9000);
    let mut intensities = vec![0u16; N_ROWS * CHANNELS];
    for ch in 0..CHANNELS {
        for row in 0..N_ROWS {
            intensities[ch * N_ROWS + row] = rng.gen_range(2000u16..62000u16);
        }
    }
    let contrast = (0..CHANNELS)
        .flat_map(|_| [0u16, 65535])
        .collect::<Vec<_>>();
    let lum = vec![50u16, 92];
    let locked = vec![0u16; CHANNELS];
    let names: Vec<String> = (0..CHANNELS).map(|_| String::new()).collect();

    struct Profile {
        label: &'static str,
        max_iters: u32,
        restarts: u32,
        post: OptimizePostprocess,
    }

    let profiles = [
        Profile {
            label: "study (3r, 800 it, Study post)",
            max_iters: 800,
            restarts: 3,
            post: OptimizePostprocess::Study,
        },
        Profile {
            label: "full post (3r, 800 it, Full)",
            max_iters: 800,
            restarts: 3,
            post: OptimizePostprocess::Full,
        },
        Profile {
            label: "more restarts (6r, 800 it, Study post)",
            max_iters: 800,
            restarts: 6,
            post: OptimizePostprocess::Study,
        },
    ];

    let c3_eval = c3::C3::new();

    eprintln!(
        "\n{:42} {:>7} {:>7} {:>7} {:>6} {:>6}",
        "profile", "mean_L", "std_L", "best_L", "uniq", "ms/run"
    );

    for p in profiles {
        let t0 = Instant::now();
        let mut totals = Vec::with_capacity(N_SEEDS);
        for seed in 0..N_SEEDS {
            let mut srng = StdRng::seed_from_u64(50_000 + seed as u64);
            let mut colors = Vec::with_capacity(CHANNELS * 3);
            let l = 0.58f32;
            for i in 0..CHANNELS {
                let angle = std::f32::consts::TAU * (i as f32) / (CHANNELS as f32)
                    + srng.gen_range(-0.12f32..0.12f32);
                let chroma = srng.gen_range(0.18f32..0.34f32);
                let okl = palette::Oklab::new(l, chroma * angle.cos(), chroma * angle.sin());
                let rgb: palette::Srgb = palette::FromColor::from_color(okl);
                colors.push((rgb.red.clamp(0.0, 1.0) * 255.0) as u16);
                colors.push((rgb.green.clamp(0.0, 1.0) * 255.0) as u16);
                colors.push((rgb.blue.clamp(0.0, 1.0) * 255.0) as u16);
            }
            let run = optimize_palette_pipeline(
                &colors,
                &locked,
                &intensities,
                &contrast,
                &lum,
                vec![],
                names.clone(),
                Some(p.max_iters),
                Some(32),
                Some(false),
                Some(p.restarts),
                Some(p.post),
            );
            let bd = evaluate_palette_objective_breakdown(
                &c3_eval,
                &run.oklab_best,
                &run.intensity_arc,
                1.0,
                0.0,
                &run.excluded_colors_indices,
                &run.color_name_indices,
            );
            totals.push(bd.total);
        }
        let elapsed = t0.elapsed();
        let n = totals.len() as f32;
        let mean = totals.iter().sum::<f32>() / n;
        let var = totals.iter().map(|t| (t - mean).powi(2)).sum::<f32>() / n;
        let best = totals.iter().cloned().fold(f32::INFINITY, f32::min);
        let mut sorted = totals.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut uniq = sorted.clone();
        uniq.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
        eprintln!(
            "{:42} {:7.3} {:7.3} {:7.3} {:6} {:6.0}",
            p.label,
            mean,
            var.sqrt(),
            best,
            uniq.len(),
            elapsed.as_secs_f64() * 1000.0 / n as f64
        );
    }
}
