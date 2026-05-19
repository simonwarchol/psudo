//! Palette consistency study: random optimized palettes per channel count.
//! Writes `target/palette_study/report.html`.
//!
//! ```bash
//! cd lib && cargo run --example palette_study --release
//! PALETTE_STUDY_PARENTS=20 PALETTE_STUDY_CHANNELS=4,6 cargo run --example palette_study --release
//! ```
//!
//! Environment (optional):
//! - `PALETTE_STUDY_PARENTS` (default 20) — palettes per channel count
//! - `PALETTE_STUDY_CHANNELS` (default `4,6`) — comma-separated channel counts
//! - `PALETTE_STUDY_ROWS` (default 384) — synthetic intensity rows per run
//! - `PALETTE_STUDY_MAX_ITERS` (default 3000 — matches production / npm)
//! - `PALETTE_STUDY_CONFUSION_SAMPLES` (default 32)
//! - `PALETTE_STUDY_RESTARTS` (default 6) — Nelder–Mead multistarts
//! - `PALETTE_STUDY_STUDY=1` — lighter Study postprocess (benchmark-style; default is Full)
//! - `PALETTE_STUDY_SPATIAL=1` — spatial channel-overlap in the objective (default off)
//! - `PALETTE_STUDY_SPREAD_INIT=0` — random saturated sRGB starts instead of hue-spread OKLab inits
//!
//! Timing is printed to stderr and embedded in `report.html` (per palette + batch totals).
//!
//! Profiling (native):
//! - `cargo instruments -t time --example palette_study --release` (macOS Instruments)
//! - `cargo flamegraph --example palette_study --release` (`cargo install flamegraph`)
//! - `samply record cargo run --example palette_study --release` (Firefox Profiler)
//! - Quick micro-bench: `cargo test -p psudo study_convergence_profiles -- --ignored --nocapture`
//!
//! Compare convergence profiles (ignored test):
//! `cargo test study_convergence_profiles -- --ignored --nocapture`

use palette::{FromColor, Oklab, Srgb};
use psudo::c3::C3;
use psudo::{
    debug_palette_channels,
    evaluate_palette_objective_breakdown,
    optimize_palette_pipeline,
    OptimizePipelineResult,
    OptimizePostprocess,
    PaletteObjectiveBreakdown,
};
use rand::rngs::StdRng;
use rand::{ Rng, SeedableRng };
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const DEFAULT_CHANNEL_COUNTS: &str = "4,6";

/// Same defaults as WASM / npm `optimize()` (spatial off).
const DEFAULT_MAX_ITERS: u32 = 3000;
const DEFAULT_CONFUSION_SAMPLES: u32 = 32;
const DEFAULT_RESTARTS: u32 = 6;

fn parse_env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
        .max(1)
}

fn parse_env_u32(key: &str, default: u32) -> u32 {
    env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
        .max(1)
}

fn parse_env_bool(key: &str) -> bool {
    env::var(key)
        .map(|s| matches!(s.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn parse_channel_counts() -> Vec<usize> {
    let raw = env::var("PALETTE_STUDY_CHANNELS").unwrap_or_else(|_| DEFAULT_CHANNEL_COUNTS.to_string());
    let mut out: Vec<usize> = raw
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n >= 2)
        .collect();
    if out.is_empty() {
        out.push(6);
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn scaled_budget(base: u32, channels: usize) -> u32 {
    ((base as u64 * channels as u64) / 3).max(1) as u32
}

fn format_duration(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms >= 10_000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else if ms >= 1000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else {
        format!("{:.0}ms", ms)
    }
}

fn log_timing_stats(phase: &str, times: &[Duration]) {
    if times.is_empty() {
        return;
    }
    let n = times.len() as f64;
    let secs: Vec<f64> = times.iter().map(|d| d.as_secs_f64()).collect();
    let sum: f64 = secs.iter().sum();
    let mean = sum / n;
    let var = secs.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / n;
    let mut sorted = secs.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if sorted.len() % 2 == 0 {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };
    eprintln!(
        "[palette_study] {phase} timing: n={} total={} mean={} median={} min={} max={} σ={}",
        times.len(),
        format_duration(Duration::from_secs_f64(sum)),
        format_duration(Duration::from_secs_f64(mean)),
        format_duration(Duration::from_secs_f64(median)),
        format_duration(Duration::from_secs_f64(sorted[0])),
        format_duration(Duration::from_secs_f64(sorted[sorted.len() - 1])),
        format_duration(Duration::from_secs_f64(var.sqrt()))
    );
}

fn log_loss_stats(phase: &str, totals: &[f32]) {
    if totals.is_empty() {
        return;
    }
    let n = totals.len() as f32;
    let mean = totals.iter().sum::<f32>() / n;
    let var = totals.iter().map(|t| (t - mean).powi(2)).sum::<f32>() / n;
    let mut sorted = totals.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut uniq = sorted.clone();
    uniq.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    eprintln!(
        "[palette_study] {phase}: n={} unique≈{} L_tot min={:.4} max={:.4} mean={:.4} std={:.4}",
        totals.len(),
        uniq.len(),
        sorted[0],
        sorted[sorted.len() - 1],
        mean,
        var.sqrt()
    );
}

fn luminance_u16() -> Vec<u16> {
    vec![45, 92]
}

fn empty_names(n: usize) -> Vec<String> {
    (0..n).map(|_| String::new()).collect()
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

/// Evenly spaced hues in OKLab (reduces “unlucky” random starts for 4–6 channels).
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

/// Random sRGB starts biased away from grey (per-channel max component high).
fn random_initial_colors_u16(channels: usize, rng: &mut StdRng) -> Vec<u16> {
    let mut c = Vec::with_capacity(channels * 3);
    for _ in 0..channels {
        let dominant = rng.gen_range(0u8..3);
        let mut rgb = [rng.gen_range(40u16..80), rng.gen_range(40u16..80), rng.gen_range(40u16..80)];
        rgb[dominant as usize] = rng.gen_range(200u16..255);
        c.extend_from_slice(&rgb);
    }
    c
}

fn srgb_linear_to_display8(r: f32, g: f32, b: f32) -> [u8; 3] {
    fn comp(c: f32) -> u8 {
        let c = c.clamp(0.0, 1.0);
        let c = if c <= 0.0031308 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (c.clamp(0.0, 1.0) * 255.0) as u8
    }
    [comp(r), comp(g), comp(b)]
}

fn optimized_to_rgb8(flat: &[f32]) -> Vec<[u8; 3]> {
    flat.chunks(3)
        .map(|c| srgb_linear_to_display8(c[0], c[1], c[2]))
        .collect()
}

fn format_channel_debug(oklab: &[f32], c3: &C3) -> String {
    let ch = debug_palette_channels(c3, oklab);
    ch.iter()
        .map(|c| {
            if c.hue_deg.is_nan() {
                format!("{}(grey)", c.name)
            } else {
                format!(
                    "{} {:.0}° sat={:.0}%",
                    c.name,
                    c.hue_deg,
                    c.srgb_saturation * 100.0
                )
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Signed horizontal bar chart for loss components (lower total = better).
fn svg_loss_bars(bd: &PaletteObjectiveBreakdown) -> String {
    let parts: [(&str, f32, &str); 6] = [
        ("name", bd.minus_mean_color_name_distance, "#6eb5ff"),
        ("RGB", bd.minus_min_perceptual_distance, "#7dcea0"),
        ("perc+", bd.perceptual_deficit_penalty, "#58d68d"),
        ("sat-", bd.minus_min_saturation, "#e67e22"),
        ("sat+", bd.saturation_deficit_penalty, "#e74c3c"),
        ("term", bd.term_loss, "#f5b041"),
    ];
    let w = 118i32;
    let h = 108i32;
    let bar_h = 12i32;
    let bar_gap = 3i32;
    let label_w = 34i32;
    let chart_w = w - label_w - 4;
    let mid = label_w + chart_w / 2;

    let max_abs = parts
        .iter()
        .map(|(_, v, _)| v.abs())
        .fold(0.0f32, f32::max)
        .max(bd.total.abs())
        .max(0.05);

    let mut s = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" class="loss-chart" aria-label="loss breakdown">"#
    );
    s.push_str(&format!(
        r#"<line x1="{mid}" y1="2" x2="{mid}" y2="{y1}" stroke="{axis}" stroke-width="1"/>"#,
        y1 = h - 18,
        axis = "#444"
    ));

    for (i, (label, val, color)) in parts.iter().enumerate() {
        let y = 4 + i as i32 * (bar_h + bar_gap);
        let half = ((val.abs() / max_abs) * (chart_w as f32 / 2.0 - 2.0)).round() as i32;
        let (x, width) = if *val >= 0.0 {
            (mid, half.max(1))
        } else {
            (mid - half.max(1), half.max(1))
        };
        s.push_str(&format!(
            r#"<text x="0" y="{ty}" font-size="9" fill="{lbl_fill}">{label}</text>"#,
            ty = y + 9,
            lbl_fill = "#888",
            label = label
        ));
        s.push_str(&format!(
            r#"<rect x="{x}" y="{y}" width="{width}" height="{bar_h}" fill="{color}" opacity="0.9"/>"#,
        ));
        s.push_str(&format!(
            r#"<text x="{w}" y="{ty}" font-size="8" fill="{val_fill}" text-anchor="end">{val:+.3}</text>"#,
            ty = y + 9,
            val_fill = "#aaa",
            val = val
        ));
    }

    let total_half = ((bd.total.abs() / max_abs) * (chart_w as f32 / 2.0 - 2.0)).round() as i32;
    let (tx, tw) = if bd.total >= 0.0 {
        (mid, total_half.max(1))
    } else {
        (mid - total_half.max(1), total_half.max(1))
    };
    let y_tot = h - 14;
    s.push_str(&format!(
        r#"<text x="0" y="{y_tot}" font-size="9" fill="{tot_lbl}" font-weight="bold">tot</text>"#,
        tot_lbl = "#ccc"
    ));
    s.push_str(&format!(
        r#"<rect x="{tx}" y="{y_tot}" width="{tw}" height="6" fill="{tot_fill}" opacity="0.85"/>"#,
        tot_fill = "#eee"
    ));
    s.push_str(&format!(
        r#"<text x="{w}" y="{y_tot}" font-size="8" fill="{tot_fill}" text-anchor="end">{tot:+.3}</text>"#,
        tot_fill = "#eee",
        tot = bd.total
    ));
    s.push_str("</svg>");
    s
}

fn format_loss_panel(bd: &PaletteObjectiveBreakdown, sa: f32) -> String {
    let gap = (sa - bd.total).abs();
    format!(
        r#"<div class="loss-panel"><div class="loss-total">L_tot={:.4} <span class="better">lower is better</span></div><div class="loss-meta">solver={:.4} |delta|={:.4}</div>{}</div>"#,
        bd.total,
        sa,
        gap,
        svg_loss_bars(bd)
    )
}

fn svg_swatches(rgb: &[[u8; 3]], sw: i32, sh: i32) -> String {
    let n = rgb.len() as i32;
    let w = sw * n;
    let mut s = String::from(r#"<div class="swatches-wrap">"#);
    s.push_str(&format!(
        r#"<svg class="swatches" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {sh}" width="{w}" height="{sh}" preserveAspectRatio="xMinYMid meet" role="img" aria-label="{n} swatches">"#,
        w = w,
        sh = sh,
        n = rgb.len()
    ));
    for (i, c) in rgb.iter().enumerate() {
        let x = i as i32 * sw;
        s.push_str(&format!(
            r#"<rect x="{}" y="0" width="{}" height="{}" fill="rgb({},{},{})"/>"#,
            x, sw, sh, c[0], c[1], c[2]
        ));
    }
    s.push_str("</svg></div>");
    s
}

struct PaletteRun {
    result: OptimizePipelineResult,
    breakdown: PaletteObjectiveBreakdown,
    optimize_elapsed: Duration,
}

struct StudyBatch {
    channels: usize,
    parents: Vec<PaletteRun>,
    batch_elapsed: Duration,
}

fn run_channel_batch(
    channels: usize,
    n_parents: usize,
    n_rows: usize,
    max_iters: u32,
    confusion_samples: u32,
    num_restarts: u32,
    include_spatial: bool,
    postprocess: OptimizePostprocess,
    spread_init: bool,
    c3_eval: &C3,
    spatial_w: f32,
) -> StudyBatch {
    let batch_start = Instant::now();
    let scaled_restarts = scaled_budget(num_restarts, channels);
    let nm_iters = scaled_budget(max_iters, channels) / 2;
    eprintln!(
        "[palette_study] {channels}ch: NM ~{nm_iters} iters/restart, {scaled_restarts} restarts (base {num_restarts}), {n_parents} palettes"
    );

    let mut shared_intensity_rng = StdRng::seed_from_u64(9000 + channels as u64);
    let shared_intensities = random_intensities(n_rows, channels, &mut shared_intensity_rng);
    let mut parents = Vec::with_capacity(n_parents);
    let seed_base = 50_000u64 + (channels as u64) * 10_000;

    for i in 0..n_parents {
        let mut rng = StdRng::seed_from_u64(seed_base + i as u64);
        let colors = if spread_init {
            spread_initial_colors_u16(channels, &mut rng)
        } else {
            random_initial_colors_u16(channels, &mut rng)
        };
        let locked = vec![0u16; channels];
        let contrast = contrast_all(channels);
        let lum = luminance_u16();
        let t0 = Instant::now();
        let run = optimize_palette_pipeline(
            &colors,
            &locked,
            &shared_intensities,
            &contrast,
            &lum,
            vec![],
            empty_names(channels),
            Some(max_iters),
            Some(confusion_samples),
            Some(include_spatial),
            Some(num_restarts),
            Some(postprocess),
        );
        let optimize_elapsed = t0.elapsed();
        let bd = evaluate_palette_objective_breakdown(
            c3_eval,
            &run.oklab_best,
            &run.intensity_arc,
            1.0,
            spatial_w,
            &run.excluded_colors_indices,
            &run.color_name_indices,
        );
        let dbg = format_channel_debug(&run.oklab_best, c3_eval);
        eprintln!(
            "[palette_study] {channels}ch #{}/{} L_tot={:.4} min_rgb={:.0} time={} | {}",
            i + 1,
            n_parents,
            bd.total,
            bd.min_display_rgb_distance,
            format_duration(optimize_elapsed),
            dbg
        );
        parents.push(PaletteRun {
            result: run,
            breakdown: bd,
            optimize_elapsed,
        });
        if (i + 1) % 5 == 0 || i == 0 {
            eprintln!("[palette_study] {channels}ch finished {}/{}", i + 1, n_parents);
        }
    }

    let batch_elapsed = batch_start.elapsed();
    log_loss_stats(
        &format!("{channels}-color palettes"),
        &parents.iter().map(|p| p.breakdown.total).collect::<Vec<_>>(),
    );
    log_timing_stats(
        &format!("{channels}-color optimize"),
        &parents
            .iter()
            .map(|p| p.optimize_elapsed)
            .collect::<Vec<_>>(),
    );
    eprintln!(
        "[palette_study] {channels}ch batch wall time {} ({} palettes)",
        format_duration(batch_elapsed),
        n_parents
    );
    StudyBatch {
        channels,
        parents,
        batch_elapsed,
    }
}

fn append_section_html(html: &mut String, batch: &StudyBatch, c3_eval: &C3, sw: i32, sh: i32) {
    let parents = &batch.parents;
    let channels = batch.channels;
    let mut parent_rows: Vec<(usize, Vec<[u8; 3]>, PaletteObjectiveBreakdown, f32, Duration)> =
        Vec::with_capacity(parents.len());
    for (i, pr) in parents.iter().enumerate() {
        parent_rows.push((
            i,
            optimized_to_rgb8(&pr.result.srgb_linear),
            pr.breakdown.clone(),
            pr.result.sa_best_cost,
            pr.optimize_elapsed,
        ));
    }
    parent_rows.sort_by(|a, b| {
        a.2.total
            .partial_cmp(&b.2.total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let time_secs: Vec<f64> = parents
        .iter()
        .map(|p| p.optimize_elapsed.as_secs_f64())
        .collect();
    let time_sum: f64 = time_secs.iter().sum();
    let time_mean = time_sum / time_secs.len().max(1) as f64;
    let mut time_sorted = time_secs.clone();
    time_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let time_median = if time_sorted.is_empty() {
        0.0
    } else if time_sorted.len() % 2 == 0 {
        (time_sorted[time_sorted.len() / 2 - 1] + time_sorted[time_sorted.len() / 2]) / 2.0
    } else {
        time_sorted[time_sorted.len() / 2]
    };
    let n_good_rgb = parents
        .iter()
        .filter(|p| p.breakdown.min_display_rgb_distance >= 190.0)
        .count();

    html.push_str(&format!(
        r#"<h2>{channels}-color palettes (n={}, sorted by L_tot)</h2>
<p class="sort-note">Best (lowest L_tot) appears first. {timing}</p>
<div class="grid3">"#,
        parents.len(),
        timing = format!(
            "Batch wall {} · optimize mean {} · median {} · range {}–{} · {}/{} with min RGB Δ≥190.",
            format_duration(batch.batch_elapsed),
            format_duration(Duration::from_secs_f64(time_mean)),
            format_duration(Duration::from_secs_f64(time_median)),
            format_duration(Duration::from_secs_f64(
                time_sorted.first().copied().unwrap_or(0.0)
            )),
            format_duration(Duration::from_secs_f64(
                time_sorted.last().copied().unwrap_or(0.0)
            )),
            n_good_rgb,
            parents.len(),
        ),
    ));

    for (rank, (orig, rgb, bd, solver_cost, elapsed)) in parent_rows.iter().enumerate() {
        html.push_str(r#"<div class="card">"#);
        let dbg = format_channel_debug(&parents[*orig].result.oklab_best, c3_eval);
        html.push_str(&format!(
            r#"<div class="label">#{} · run {} · L_tot={:.4} · min RGB Δ≥{:.1} · <span class="time">{}</span><br/><span class="names">{}</span></div>"#,
            rank + 1,
            orig + 1,
            bd.total,
            bd.min_display_rgb_distance,
            format_duration(*elapsed),
            dbg
        ));
        html.push_str(r#"<div class="card-body">"#);
        html.push_str(&svg_swatches(rgb, sw, sh));
        html.push_str(&format_loss_panel(bd, *solver_cost));
        html.push_str("</div></div>");
    }
    html.push_str("</div>");
}

const HTML_HEAD: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>Palette study</title>
<style>
  body { font-family: system-ui, sans-serif; margin: 24px; background: #111; color: #e8e8e8; overflow-x: auto; }
  h1 { font-size: 1.35rem; }
  h2 { font-size: 1rem; margin-top: 2rem; color: #9cf; }
  p.note { color: #888; max-width: 72ch; line-height: 1.45; }
  .grid3 {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 300px), 1fr));
    gap: 16px;
    max-width: 1600px;
  }
  .card {
    background: #1c1c1c;
    border-radius: 8px;
    padding: 10px;
    border: 1px solid #333;
    min-width: 0;
    overflow: visible;
  }
  .card .label { font-size: 0.75rem; color: #888; margin-bottom: 6px; word-break: break-word; }
  .names { font-size: 0.65rem; color: #9ab; line-height: 1.35; }
  .card-body {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: flex-start;
    min-width: 0;
  }
  .swatches-wrap {
    flex: 1 1 auto;
    min-width: 0;
    max-width: 100%;
    overflow-x: auto;
    overflow-y: hidden;
    -webkit-overflow-scrolling: touch;
  }
  .swatches-wrap svg.swatches {
    display: block;
    width: 100%;
    max-width: 100%;
    height: auto;
    max-height: 100px;
  }
  .loss-panel {
    font-size: 0.68rem;
    color: #bbb;
    font-family: ui-monospace, Menlo, monospace;
    flex: 0 0 auto;
    min-width: 118px;
  }
  @media (max-width: 520px) {
    .card-body { flex-direction: column; }
    .loss-panel { width: 100%; }
  }
  .loss-total { font-size: 0.75rem; color: #eee; margin-bottom: 4px; }
  .loss-meta { color: #666; font-size: 0.6rem; margin-bottom: 4px; }
  .better { color: #7dcea0; font-weight: normal; }
  .sort-note { color: #9cf; font-size: 0.85rem; margin-bottom: 8px; }
  .time { color: #f5b041; font-weight: 600; }
</style>
</head>
<body>
"#;

fn main() {
    let channel_counts = parse_channel_counts();
    let n_parents = parse_env_usize("PALETTE_STUDY_PARENTS", 20);
    let n_rows = parse_env_usize("PALETTE_STUDY_ROWS", 384);
    let max_iters = parse_env_u32("PALETTE_STUDY_MAX_ITERS", DEFAULT_MAX_ITERS);
    let confusion_samples =
        parse_env_u32("PALETTE_STUDY_CONFUSION_SAMPLES", DEFAULT_CONFUSION_SAMPLES);
    let num_restarts = parse_env_u32("PALETTE_STUDY_RESTARTS", DEFAULT_RESTARTS);
    let include_spatial = parse_env_bool("PALETTE_STUDY_SPATIAL");
    let study_post = parse_env_bool("PALETTE_STUDY_STUDY");
    let postprocess = if study_post {
        OptimizePostprocess::Study
    } else {
        OptimizePostprocess::Full
    };
    let spread_init = env::var("PALETTE_STUDY_SPREAD_INIT")
        .map(|s| !matches!(s.as_str(), "0" | "false" | "FALSE" | "no" | "NO"))
        .unwrap_or(true);

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/palette_study");
    fs::create_dir_all(&out_dir).expect("mkdir");

    let channels_label: String = channel_counts
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    eprintln!(
        "[palette_study] Nelder–Mead · max_iters={} restarts={} confusion={} · channels=[{}] · {} palettes/ch · spatial={} post={:?}",
        max_iters,
        num_restarts,
        confusion_samples,
        channels_label,
        n_parents,
        include_spatial,
        postprocess
    );

    let c3_eval = C3::new();
    let spatial_w = if include_spatial { 0.1 } else { 0.0 };

    let study_start = Instant::now();
    let mut batches = Vec::new();
    for &channels in &channel_counts {
        batches.push(run_channel_batch(
            channels,
            n_parents,
            n_rows,
            max_iters,
            confusion_samples,
            num_restarts,
            include_spatial,
            postprocess,
            spread_init,
            &c3_eval,
            spatial_w,
        ));
    }

    let study_elapsed = study_start.elapsed();
    let sw = 40i32;
    let sh = 100i32;
    let total_palettes = n_parents * channel_counts.len();
    let mut html = String::from(HTML_HEAD);
    html.push_str(&format!(
        r#"<h1>Palette study</h1>
<p class="note">
  <strong>{total_palettes}</strong> optimized palettes ({n_parents} per channel count: <strong>{channels_label}</strong>).
  Defaults: <code>max_iters={max_iters}</code>, <code>restarts={num_restarts}</code>,
  <code>confusion_samples={confusion_samples}</code>; Nelder–Mead multistart; spatial <strong>{spatial}</strong>.
  <br/><br/>
  Total study wall time: <strong>{study_time}</strong> (see per-section timing below).
  <br/><br/>
  Lower <code>L_tot</code> is better. Sorted by loss within each section.
</p>
"#,
        spatial = if include_spatial { "on" } else { "off" },
        study_time = format_duration(study_elapsed),
    ));

    for batch in &batches {
        append_section_html(&mut html, batch, &c3_eval, sw, sh);
    }
    html.push_str("</body></html>");

    let path = out_dir.join("report.html");
    fs::write(&path, &html).expect("write report");
    eprintln!(
        "[palette_study] total study wall time {}",
        format_duration(study_elapsed)
    );
    eprintln!(
        "[palette_study] wrote {} ({} bytes)",
        path.display(),
        html.len()
    );
    eprintln!(
        "[palette_study] open in browser: file://{}",
        path.canonicalize().unwrap_or(path).display()
    );
}
