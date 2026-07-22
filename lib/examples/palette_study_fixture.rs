//! Fixed-input palette_study run for native vs WASM comparison.
//! Writes `target/palette_study/fixture.json` and prints native metrics.
//!
//! ```bash
//! cd lib && cargo run --example palette_study_fixture --release
//! pnpm run palette-study-compare   # from repo root (after wasm-build)
//! ```

use palette::{FromColor, Oklab, Srgb};
use psudo::c3::C3;
use psudo::{evaluate_palette_objective_breakdown, optimize_palette_pipeline, OptimizePostprocess};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

const CHANNELS: usize = 4;
const N_ROWS: usize = 384;
const MAX_ITERS: u32 = 3000;
const CONFUSION_SAMPLES: u32 = 32;
const NUM_RESTARTS: u32 = 6;
const PARENT_SEED: u64 = 50_000;

#[derive(Serialize)]
struct Fixture {
    channels: usize,
    colors: Vec<u16>,
    locked: Vec<u16>,
    intensities: Vec<u16>,
    contrast_limits: Vec<u16>,
    luminance: Vec<u16>,
    excluded: Vec<String>,
    color_names: Vec<String>,
    max_iters: u32,
    confusion_samples: u32,
    num_restarts: u32,
    spatial: bool,
    native_l_tot: f32,
    native_min_rgb: f32,
}

fn luminance_u16() -> Vec<u16> {
    vec![50, 92]
}

fn contrast_all(channels: usize) -> Vec<u16> {
    (0..channels).flat_map(|_| [0u16, 65535]).collect()
}

fn random_intensities(n_rows: usize, channels: usize, rng: &mut StdRng) -> Vec<u16> {
    let mut out = vec![0u16; n_rows * channels];
    for ch in 0..channels {
        for row in 0..n_rows {
            out[ch * n_rows + row] = rng.gen_range(2000u16..62000u16);
        }
    }
    out
}

fn spread_initial_colors_u16(channels: usize, rng: &mut StdRng) -> Vec<u16> {
    let mut out = Vec::with_capacity(channels * 3);
    let l = 0.58f32;
    for i in 0..channels {
        let angle = std::f32::consts::TAU * (i as f32) / (channels as f32)
            + rng.gen_range(-0.12f32..0.12f32);
        let chroma = rng.gen_range(0.18f32..0.34f32);
        let okl = Oklab::new(l, chroma * angle.cos(), chroma * angle.sin());
        let rgb: Srgb = Srgb::from_color(okl);
        out.push((rgb.red.clamp(0.0, 1.0) * 255.0) as u16);
        out.push((rgb.green.clamp(0.0, 1.0) * 255.0) as u16);
        out.push((rgb.blue.clamp(0.0, 1.0) * 255.0) as u16);
    }
    out
}

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/palette_study");
    fs::create_dir_all(&out_dir).expect("mkdir");

    let mut intensity_rng = StdRng::seed_from_u64(9000 + CHANNELS as u64);
    let intensities = random_intensities(N_ROWS, CHANNELS, &mut intensity_rng);
    let mut rng = StdRng::seed_from_u64(PARENT_SEED);
    let colors = spread_initial_colors_u16(CHANNELS, &mut rng);
    let locked = vec![0u16; CHANNELS];
    let contrast = contrast_all(CHANNELS);
    let lum = luminance_u16();
    let color_names: Vec<String> = (0..CHANNELS).map(|_| String::new()).collect();

    let run = optimize_palette_pipeline(
        &colors,
        &locked,
        &intensities,
        &contrast,
        &lum,
        vec![],
        color_names.clone(),
        Some(MAX_ITERS),
        Some(CONFUSION_SAMPLES),
        Some(false),
        Some(NUM_RESTARTS),
        Some(OptimizePostprocess::Full),
    );

    let c3_eval = C3::new();
    let bd = evaluate_palette_objective_breakdown(
        &c3_eval,
        &run.oklab_best,
        &run.intensity_arc,
        1.0,
        0.0,
        &run.excluded_colors_indices,
        &run.color_name_indices,
    );

    let fixture = Fixture {
        channels: CHANNELS,
        colors,
        locked,
        intensities,
        contrast_limits: contrast,
        luminance: lum,
        excluded: vec![],
        color_names,
        max_iters: MAX_ITERS,
        confusion_samples: CONFUSION_SAMPLES,
        num_restarts: NUM_RESTARTS,
        spatial: false,
        native_l_tot: bd.total,
        native_min_rgb: bd.min_display_rgb_distance,
    };

    let path = out_dir.join("fixture.json");
    let json = serde_json::to_string_pretty(&fixture).expect("json");
    fs::write(&path, json).expect("write fixture");

    eprintln!(
        "[palette_study_fixture] native L_tot={:.4} min_rgb={:.0}",
        bd.total, bd.min_display_rgb_distance
    );
    eprintln!("[palette_study_fixture] wrote {}", path.display());
}
