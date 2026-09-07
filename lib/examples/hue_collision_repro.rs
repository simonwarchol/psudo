//! Repro: 7th color near an existing purple vs unused #0000ff, including locked search.
//!
//! ```bash
//! cd lib && cargo run --example hue_collision_repro --release
//! ```

use palette::{FromColor, Oklab, Srgb};
use psudo::c3::C3;
use psudo::{
    coarse_name_family, compute_diagnostics, debug_palette_channels,
    evaluate_palette_objective_breakdown, optimize_palette_pipeline, OptimizePostprocess,
    PaletteObjectiveBreakdown,
};
use std::sync::Arc;
use std::time::Instant;

const SCREENSHOT: [[u8; 3]; 7] = [
    [255, 0, 0],
    [155, 0, 255],
    [174, 255, 2],
    [0, 218, 255],
    [255, 0, 255],
    [255, 196, 3],
    [192, 0, 255],
];

const BLUE: [u8; 3] = [0, 0, 255];
const MINERVA_UNLOCK_SEED: [u8; 3] = [0x0d, 0xab, 0xff];

fn rgb_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
    let s = Srgb::new(rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0);
    let o: Oklab = Oklab::from_color(s);
    [o.l, o.a, o.b]
}

fn palette_oklab(rgbs: &[[u8; 3]]) -> Vec<f32> {
    rgbs.iter().flat_map(|c| rgb_to_oklab(*c)).collect()
}

fn oklab_to_hex(oklab: &[f32]) -> Vec<String> {
    oklab
        .chunks(3)
        .map(|c| {
            let rgb: Srgb = Srgb::from_color(Oklab::new(c[0], c[1], c[2]));
            format!(
                "#{:02x}{:02x}{:02x}",
                (rgb.red.clamp(0.0, 1.0) * 255.0).round() as u8,
                (rgb.green.clamp(0.0, 1.0) * 255.0).round() as u8,
                (rgb.blue.clamp(0.0, 1.0) * 255.0).round() as u8
            )
        })
        .collect()
}

fn dummy_intensity(n: usize) -> Arc<ndarray::Array2<f32>> {
    Arc::new(ndarray::Array2::zeros((0, n)))
}

fn print_breakdown(label: &str, bd: &PaletteObjectiveBreakdown) {
    println!(
        "  {label}: total={:.4} mean_c3={:.4} min_rgb={:.1} perc_def={:.4} hue_gap={:.1} hue_rew={:.4} hue_def={:.4} sat={:.4}",
        bd.total,
        bd.minus_mean_color_name_distance,
        bd.min_display_rgb_distance,
        bd.perceptual_deficit_penalty,
        bd.min_hue_gap_deg,
        bd.hue_separation_reward,
        bd.hue_separation_deficit,
        bd.minus_min_saturation
    );
}

fn report_palette(c3: &C3, label: &str, oklab: &[f32]) {
    let n = oklab.len() / 3;
    let intensity = dummy_intensity(n);
    let names = vec![-1.0f32; n];
    let bd = evaluate_palette_objective_breakdown(c3, oklab, &intensity, 1.0, 0.0, &[], &names);
    let diag = compute_diagnostics(c3, oklab, &bd);
    let hexes = oklab_to_hex(oklab);
    let debug = debug_palette_channels(c3, oklab);
    println!("\n== {label} ==");
    print_breakdown("L_tot", &bd);
    println!(
        "  families dup={} min_c3={:.3} min_oklab={:.3}",
        diag.duplicate_family_pair_count, diag.min_c3_name_distance, diag.min_oklab_distance
    );
    for (i, ch) in debug.iter().enumerate() {
        println!(
            "  [{i}] {}  {:>12}  family={:<18} hue={:7.1} chroma={:.3}",
            hexes[i],
            ch.name,
            coarse_name_family(&ch.name),
            ch.hue_deg,
            ch.chroma
        );
    }
}

fn colors_u16(rgbs: &[[u8; 3]]) -> Vec<u16> {
    rgbs.iter()
        .flat_map(|c| [c[0] as u16, c[1] as u16, c[2] as u16])
        .collect()
}

fn contrast_all(n: usize) -> Vec<u16> {
    (0..n).flat_map(|_| [0u16, 65535]).collect()
}

fn optimize_slots(rgbs: &[[u8; 3]], locked: &[u16], restarts: u32) -> Vec<f32> {
    let n = rgbs.len();
    let colors = colors_u16(rgbs);
    let lum = vec![50u16, 92];
    let t0 = Instant::now();
    let run = optimize_palette_pipeline(
        &colors,
        locked,
        &[],
        &contrast_all(n),
        &lum,
        vec![],
        vec![String::new(); n],
        Some(2700),
        Some(32),
        Some(false),
        Some(restarts),
        Some(OptimizePostprocess::Full),
    );
    eprintln!(
        "[optimize] n={n} locked={} restarts={restarts} {:.1}s sa_cost={:.4}",
        locked.iter().filter(|&&x| x == 1).count(),
        t0.elapsed().as_secs_f32(),
        run.sa_best_cost
    );
    run.oklab_best
}

fn rgb_drift(before: &[u8; 3], oklab: &[f32]) -> f32 {
    let hex = oklab_to_hex(oklab);
    let h = hex[0].trim_start_matches('#');
    let r = u8::from_str_radix(&h[0..2], 16).unwrap() as f32;
    let g = u8::from_str_radix(&h[2..4], 16).unwrap() as f32;
    let b = u8::from_str_radix(&h[4..6], 16).unwrap() as f32;
    let dr = r - before[0] as f32;
    let dg = g - before[1] as f32;
    let db = b - before[2] as f32;
    (dr * dr + dg * dg + db * db).sqrt()
}

fn main() {
    let c3 = C3::new();
    let shot = palette_oklab(&SCREENSHOT);
    report_palette(&c3, "screenshot (as shown)", &shot);

    let mut blue_swap = SCREENSHOT;
    blue_swap[6] = BLUE;
    report_palette(&c3, "screenshot with last=#0000ff", &palette_oklab(&blue_swap));

    let mut start = SCREENSHOT;
    start[6] = MINERVA_UNLOCK_SEED;
    let mut locked = vec![1u16; 7];
    locked[6] = 0;
    let restarts: u32 = std::env::var("RESTARTS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    let opt = optimize_slots(&start, &locked, restarts);
    report_palette(&c3, "optimize 6 locked + 1 free (Minerva stack path)", &opt);

    println!("\n== locked-channel RGB drift after NM ==");
    for i in 0..6 {
        let d = rgb_drift(&SCREENSHOT[i], &opt[i * 3..i * 3 + 3]);
        println!("  locked[{i}] drift ΔRGB={d:.1}");
    }
    let free_hex = &oklab_to_hex(&opt)[6];
    println!(
        "  free[6] {free_hex}  start was #{:02x}{:02x}{:02x}",
        MINERVA_UNLOCK_SEED[0], MINERVA_UNLOCK_SEED[1], MINERVA_UNLOCK_SEED[2]
    );

    let mut restored = Vec::from(&shot[..18]);
    restored.extend_from_slice(&opt[18..21]);
    report_palette(
        &c3,
        "what the UI shows (locks snapped back, free kept)",
        &restored,
    );
    println!(
        "  NM thought total={:.4}; after snap-back the real palette is above",
        evaluate_palette_objective_breakdown(
            &c3,
            &opt,
            &dummy_intensity(7),
            1.0,
            0.0,
            &[],
            &[-1.0f32; 7],
        )
        .total
    );

    let free_all = optimize_slots(&SCREENSHOT, &[0u16; 7], restarts);
    report_palette(&c3, "optimize all 7 free", &free_all);
}
