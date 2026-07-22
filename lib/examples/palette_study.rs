//! Palette consistency study: random optimized palettes per channel count.
//! Writes `target/palette_study/report.html`.
//!
//! ```bash
//! cd lib && cargo run --example palette_study --release
//! PALETTE_STUDY_PARENTS=10 PALETTE_STUDY_CHANNELS=4,6,8 cargo run --example palette_study --release
//! ```
//!
//! WASM / npm (same env vars, uses `psudo/sync` from repo root):
//! ```bash
//! pnpm run palette-study-wasm
//! pnpm run palette-study-wasm -- --document path/to/story.json
//! ```
//! Report: `lib/target/palette_study_wasm/report.html`
//!
//! Environment (optional):
//! - `PALETTE_STUDY_PARENTS` (default 10) — palettes per channel count
//! - `PALETTE_STUDY_CHANNELS` (default `4,6,8`) — comma-separated channel counts
//! - `PALETTE_STUDY_ROWS` (default 384) — synthetic intensity rows per run
//! - `PALETTE_STUDY_MAX_ITERS` (default 3000 — matches production / npm)
//! - `PALETTE_STUDY_CONFUSION_SAMPLES` (default 32)
//! - `PALETTE_STUDY_RESTARTS` (default 18) — Nelder–Mead multistarts
//! - `PALETTE_STUDY_STUDY=1` — lighter Study postprocess (benchmark-style; default is Full)
//! - `PALETTE_STUDY_SPATIAL=1` — spatial channel-overlap in the objective (default off)
//! - `PALETTE_STUDY_SPREAD_INIT=0` — random saturated sRGB starts instead of hue-spread OKLab inits
//! - `PSUDO_PALETTE_SELECTION` — selection modes for static report side-by-side (default `total`)
//! - `PSUDO_INIT` (default `current`) — `current` | `glasbey_v1` | `mixed` initializer
//! - `PSUDO_REFINE` (default `cartesian`) — refine used for `oklab_best` / report winner
//! - `PSUDO_REVIEW_METHODS` (default `total,oklab_sep`) — cards in interactive review.html
//!   (`total` = production mean C3 + sRGB sep; `oklab_sep` = mean C3 + OKLab sep;
//!   also `min_name`, `polar`/`hybrid`)
//! - `PSUDO_OBJECTIVE` (default `total`) — objective for the static report winner optimize
//!
//! Also writes:
//! - `target/palette_study/candidates.json` — all restart-pool diagnostics
//! - `target/palette_study/review.html` — interactive vote UI (pick best method per case → scoreboard)
//! - `target/palette_study/review_data.json` — same payload embedded in review.html
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
    apply_palette_refine_ex, compute_diagnostics, debug_palette_channels,
    evaluate_palette_objective_breakdown, optimize_palette_pipeline_with_init, select_best_restart,
    OptimizePipelineResult, OptimizePostprocess, PaletteInitMode, PaletteObjectiveBreakdown,
    PaletteObjectiveMode, PaletteRefineMode, PaletteSelectionMode, RestartRecord,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const DEFAULT_CHANNEL_COUNTS: &str = "4,6,8";

/// Same defaults as WASM / npm `optimize()` (spatial off).
const DEFAULT_MAX_ITERS: u32 = 3000;
const DEFAULT_CONFUSION_SAMPLES: u32 = 32;
const DEFAULT_RESTARTS: u32 = 18;

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
    let raw =
        env::var("PALETTE_STUDY_CHANNELS").unwrap_or_else(|_| DEFAULT_CHANNEL_COUNTS.to_string());
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

fn parse_selection_modes() -> Vec<PaletteSelectionMode> {
    // Static report still supports Lex A/B; default is production total only.
    let raw = env::var("PSUDO_PALETTE_SELECTION").unwrap_or_else(|_| "total".into());
    let mut modes: Vec<PaletteSelectionMode> = raw
        .split(',')
        .filter_map(|s| PaletteSelectionMode::parse(s.trim()))
        .collect();
    if modes.is_empty() {
        modes.push(PaletteSelectionMode::TotalLoss);
    }
    modes
}

fn parse_init_mode() -> PaletteInitMode {
    env::var("PSUDO_INIT")
        .ok()
        .and_then(|s| PaletteInitMode::parse(&s))
        .unwrap_or(PaletteInitMode::Current)
}

fn parse_refine_mode() -> PaletteRefineMode {
    env::var("PSUDO_REFINE")
        .ok()
        .and_then(|s| PaletteRefineMode::parse(&s))
        .unwrap_or(PaletteRefineMode::Cartesian)
}

fn parse_objective_mode() -> PaletteObjectiveMode {
    env::var("PSUDO_OBJECTIVE")
        .ok()
        .and_then(|s| PaletteObjectiveMode::parse(&s))
        .unwrap_or(PaletteObjectiveMode::MeanOnly)
}

/// Review cards: objective variants, refine variants, and/or selection modes.
#[derive(Clone, Copy, Debug)]
enum ReviewMethodSpec {
    Objective(PaletteObjectiveMode),
    Refine(PaletteRefineMode),
    Select(PaletteSelectionMode),
}

impl ReviewMethodSpec {
    fn id(self) -> &'static str {
        match self {
            Self::Objective(m) => m.id(),
            Self::Refine(PaletteRefineMode::Cartesian) => "cartesian",
            Self::Refine(m) => m.id(),
            Self::Select(m) => m.id(),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Objective(m) => m.label(),
            Self::Refine(PaletteRefineMode::Cartesian) => "Cartesian refine (on total)",
            Self::Refine(m) => m.label(),
            Self::Select(m) => m.label(),
        }
    }
}

fn parse_review_methods() -> Vec<ReviewMethodSpec> {
    let raw = env::var("PSUDO_REVIEW_METHODS").unwrap_or_else(|_| "total,oklab_sep".into());
    let mut out = Vec::new();
    for part in raw.split(',') {
        let s = part.trim();
        if s.is_empty() {
            continue;
        }
        // Objective first so `total` / `oklab_sep` / `min_name` are not stolen by other aliases.
        if let Some(o) = PaletteObjectiveMode::parse(s) {
            out.push(ReviewMethodSpec::Objective(o));
        } else if let Some(r) = PaletteRefineMode::parse(s) {
            out.push(ReviewMethodSpec::Refine(r));
        } else if let Some(sel) = PaletteSelectionMode::parse(s) {
            out.push(ReviewMethodSpec::Select(sel));
        }
    }
    if out.is_empty() {
        out.push(ReviewMethodSpec::Objective(PaletteObjectiveMode::MeanOnly));
        out.push(ReviewMethodSpec::Objective(PaletteObjectiveMode::OklabSep));
    }
    out
}

fn oklab_to_srgb_linear(oklab: &[f32]) -> Vec<f32> {
    oklab
        .chunks(3)
        .flat_map(|c| {
            let okl = Oklab::new(c[0], c[1], c[2]);
            let rgb: Srgb = Srgb::from_color(okl);
            [
                rgb.red.clamp(0.0, 1.0),
                rgb.green.clamp(0.0, 1.0),
                rgb.blue.clamp(0.0, 1.0),
            ]
        })
        .collect()
}

fn hex_colors_from_oklab(oklab: &[f32]) -> Vec<String> {
    optimized_to_rgb8(&oklab_to_srgb_linear(oklab))
        .iter()
        .map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
        .collect()
}

fn format_diag_badges(rec: &RestartRecord) -> String {
    let d = &rec.diagnostics;
    let mut badges = Vec::new();
    if d.duplicate_family_pair_count > 0 {
        badges.push(format!(
            r#"<span class="badge bad">dup×{}</span>"#,
            d.duplicate_family_pair_count
        ));
    }
    if d.earth_term_mass >= 0.15 {
        badges.push(format!(
            r#"<span class="badge warn">earth={:.2}</span>"#,
            d.earth_term_mass
        ));
    }
    if let Some((i, j)) = d.worst_name_pair {
        badges.push(format!(
            r#"<span class="badge">worst-name {}:{} ({:.3})</span>"#,
            i, j, d.min_c3_name_distance
        ));
    }
    if badges.is_empty() {
        badges.push(r#"<span class="badge ok">clean</span>"#.to_string());
    }
    badges.join(" ")
}

fn top_k_indices_by_total(pool: &[RestartRecord], k: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..pool.len()).collect();
    idx.sort_by(|&a, &b| {
        pool[a]
            .diagnostics
            .total_loss
            .partial_cmp(&pool[b].diagnostics.total_loss)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| pool[a].restart_id.cmp(&pool[b].restart_id))
    });
    idx.truncate(k.min(pool.len()));
    idx
}

fn top_k_indices_by_lex(pool: &[RestartRecord], k: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..pool.len()).collect();
    // Rank by comparing each pair with LexV1 against a synthetic "first".
    idx.sort_by(|&a, &b| {
        let tmp = [pool[a].clone(), pool[b].clone()];
        match select_best_restart(&tmp, PaletteSelectionMode::LexV1) {
            Some(0) => std::cmp::Ordering::Less,
            Some(1) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    });
    idx.truncate(k.min(pool.len()));
    idx
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
    vec![50, 92]
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
        let mut rgb = [
            rng.gen_range(40u16..80),
            rng.gen_range(40u16..80),
            rng.gen_range(40u16..80),
        ];
        rgb[dominant as usize] = rng.gen_range(200u16..255);
        c.extend_from_slice(&rgb);
    }
    c
}

/// `OptimizePipelineResult::srgb_linear` stores gamma-encoded (display) sRGB in 0–1
/// (`Srgb::from_color(oklab)`), so just scale to 0–255. Re-applying the sRGB OETF
/// here double-encodes gamma and washes swatches out vs. their C3 name.
fn srgb_linear_to_display8(r: f32, g: f32, b: f32) -> [u8; 3] {
    fn comp(c: f32) -> u8 {
        (c.clamp(0.0, 1.0) * 255.0).round() as u8
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
    let parts: [(&str, f32, &str); 8] = [
        ("name", bd.minus_mean_color_name_distance, "#6eb5ff"),
        ("RGB", bd.minus_min_perceptual_distance, "#7dcea0"),
        ("perc+", bd.perceptual_deficit_penalty, "#58d68d"),
        ("hue-", bd.hue_separation_reward, "#af7ac5"),
        ("hue+", bd.hue_separation_deficit, "#bb8fce"),
        ("sat-", bd.minus_min_saturation, "#e67e22"),
        ("sat+", bd.saturation_deficit_penalty, "#e74c3c"),
        ("term", bd.term_loss, "#f5b041"),
    ];
    let w = 118i32;
    let h = 138i32;
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
    case_id: String,
    parent_seed: u64,
    report_objective: PaletteObjectiveMode,
    /// Optimize used for the static report (`report_objective`).
    result: OptimizePipelineResult,
    breakdown: PaletteObjectiveBreakdown,
    optimize_elapsed: Duration,
    /// Extra objective-mode optimizes for review cards (same seed / intensities).
    alt_objectives: Vec<(PaletteObjectiveMode, OptimizePipelineResult)>,
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
    init_mode: PaletteInitMode,
    refine_mode: PaletteRefineMode,
    report_objective: PaletteObjectiveMode,
    review_objectives: &[PaletteObjectiveMode],
    c3_eval: &C3,
    spatial_w: f32,
) -> StudyBatch {
    let batch_start = Instant::now();
    let scaled_restarts = scaled_budget(num_restarts, channels);
    let nm_iters = scaled_budget(max_iters, channels) / 2;
    let extra_obj: Vec<PaletteObjectiveMode> = review_objectives
        .iter()
        .copied()
        .filter(|o| *o != report_objective)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    eprintln!(
        "[palette_study] {channels}ch: NM ~{nm_iters} iters/restart, {scaled_restarts} restarts (base {num_restarts}), {n_parents} palettes, init={} refine={} objective={} (+{} review objs)",
        init_mode.id(),
        refine_mode.id(),
        report_objective.id(),
        extra_obj.len()
    );

    let mut shared_intensity_rng = StdRng::seed_from_u64(9000 + channels as u64);
    let shared_intensities = random_intensities(n_rows, channels, &mut shared_intensity_rng);
    let mut parents = Vec::with_capacity(n_parents);
    let seed_base = 50_000u64 + (channels as u64) * 10_000;

    for i in 0..n_parents {
        let parent_seed = seed_base + i as u64;
        let mut rng = StdRng::seed_from_u64(parent_seed);
        let colors = if spread_init {
            spread_initial_colors_u16(channels, &mut rng)
        } else {
            random_initial_colors_u16(channels, &mut rng)
        };
        let locked = vec![0u16; channels];
        let contrast = contrast_all(channels);
        let lum = luminance_u16();
        let t0 = Instant::now();
        let run = optimize_palette_pipeline_with_init(
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
            init_mode,
            refine_mode,
            report_objective,
        );
        let mut alt_objectives = Vec::new();
        for &obj in &extra_obj {
            let alt = optimize_palette_pipeline_with_init(
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
                init_mode,
                refine_mode,
                obj,
            );
            alt_objectives.push((obj, alt));
        }
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
            "[palette_study] {channels}ch #{}/{} L_tot={:.4} min_rgb={:.0} pool={} time={} | {}",
            i + 1,
            n_parents,
            bd.total,
            bd.min_display_rgb_distance,
            run.restart_pool.len(),
            format_duration(optimize_elapsed),
            dbg
        );
        parents.push(PaletteRun {
            case_id: format!("{channels}ch_run{}", i + 1),
            parent_seed,
            report_objective,
            result: run,
            breakdown: bd,
            optimize_elapsed,
            alt_objectives,
        });
        if (i + 1) % 5 == 0 || i == 0 {
            eprintln!(
                "[palette_study] {channels}ch finished {}/{}",
                i + 1,
                n_parents
            );
        }
    }

    let batch_elapsed = batch_start.elapsed();
    log_loss_stats(
        &format!("{channels}-color palettes"),
        &parents
            .iter()
            .map(|p| p.breakdown.total)
            .collect::<Vec<_>>(),
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

fn append_mode_card(
    html: &mut String,
    mode: PaletteSelectionMode,
    pool: &[RestartRecord],
    c3_eval: &C3,
    sw: i32,
    sh: i32,
    production_bd: &PaletteObjectiveBreakdown,
    solver_cost: f32,
) {
    let Some(idx) = select_best_restart(pool, mode) else {
        html.push_str(r#"<div class="mode-card"><div class="label">no pool</div></div>"#);
        return;
    };
    let rec = &pool[idx];
    let srgb = oklab_to_srgb_linear(&rec.oklab);
    let rgb = optimized_to_rgb8(&srgb);
    let d = &rec.diagnostics;
    let families = d.coarse_name_families.join(" · ");
    let names = d.dominant_c3_names.join(" · ");
    let hue_dbg = format_channel_debug(&rec.oklab, c3_eval);
    let same_as_prod = (d.total_loss - production_bd.total).abs() < 1e-4
        && mode == PaletteSelectionMode::TotalLoss;
    html.push_str(r#"<div class="mode-card">"#);
    html.push_str(&format!(
        r#"<div class="mode-title">{} <code>{}</code>{}</div>"#,
        mode.label(),
        mode.id(),
        if same_as_prod {
            " · production fold"
        } else {
            ""
        }
    ));
    html.push_str(&format!(
        r#"<div class="label">restart {} · L_tot={:.4} · min RGB Δ={:.1} · hueΔ≥{:.0}° · min C3={:.3} · p10 C3={:.3}<br/>{}</div>"#,
        rec.restart_id,
        d.total_loss,
        d.min_display_rgb_distance,
        d.min_oklch_hue_gap_deg,
        d.min_c3_name_distance,
        d.p10_c3_name_distance,
        format_diag_badges(rec)
    ));
    html.push_str(&format!(
        r#"<div class="names">names: {}<br/>families: {}<br/>{}</div>"#,
        names, families, hue_dbg
    ));
    html.push_str(r#"<div class="card-body">"#);
    html.push_str(&svg_swatches(&rgb, sw, sh));
    // Approximate bars from diagnostics (full breakdown remaining on production path).
    let bd_for_bars = PaletteObjectiveBreakdown {
        total: d.total_loss,
        minus_mean_color_name_distance: -(d.mean_c3_name_distance as f32),
        minus_min_color_name_distance: 0.0,
        minus_min_perceptual_distance: -(d.min_display_rgb_distance / 255.0),
        perceptual_deficit_penalty: 0.0,
        min_display_rgb_distance: d.min_display_rgb_distance,
        hue_separation_reward: 0.0,
        hue_separation_deficit: 0.0,
        min_hue_gap_deg: d.min_oklch_hue_gap_deg as f32,
        term_loss: 0.0,
        confusion_weighted: 0.0,
        minus_min_saturation: 0.0,
        saturation_deficit_penalty: 0.0,
        min_srgb_saturation: d.min_srgb_saturation,
        min_oklab_chroma: d.min_oklab_chroma,
    };
    html.push_str(&format_loss_panel(&bd_for_bars, solver_cost));
    html.push_str("</div></div>");
}

fn append_section_html(
    html: &mut String,
    batch: &StudyBatch,
    c3_eval: &C3,
    modes: &[PaletteSelectionMode],
    sw: i32,
    sh: i32,
) {
    let parents = &batch.parents;
    let channels = batch.channels;
    let mut order: Vec<usize> = (0..parents.len()).collect();
    order.sort_by(|&a, &b| {
        parents[a]
            .breakdown
            .total
            .partial_cmp(&parents[b].breakdown.total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    html.push_str(&format!(
        r#"<h2>{channels}-color palettes (n={}, sorted by production L_tot)</h2>
<p class="sort-note">Each case shows selection modes side-by-side from the same restart pool. Batch wall {}.</p>
"#,
        parents.len(),
        format_duration(batch.batch_elapsed),
    ));

    // Per-mode summary table
    html.push_str(r#"<table class="summary"><tr><th>mode</th><th>median L_tot</th><th>median min C3</th><th>median min RGB</th><th>dup failures</th><th>earth failures</th></tr>"#);
    for &mode in modes {
        let mut totals = Vec::new();
        let mut min_c3 = Vec::new();
        let mut min_rgb = Vec::new();
        let mut dup_fail = 0usize;
        let mut earth_fail = 0usize;
        for pr in parents {
            if let Some(i) = select_best_restart(&pr.result.restart_pool, mode) {
                let d = &pr.result.restart_pool[i].diagnostics;
                totals.push(d.total_loss);
                min_c3.push(d.min_c3_name_distance);
                min_rgb.push(d.min_display_rgb_distance);
                if d.duplicate_family_pair_count > 0 {
                    dup_fail += 1;
                }
                if d.earth_term_mass >= 0.30 {
                    earth_fail += 1;
                }
            }
        }
        let med = |v: &mut [f32]| {
            if v.is_empty() {
                return 0.0;
            }
            v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            v[v.len() / 2]
        };
        let med_f64 = |v: &mut [f64]| {
            if v.is_empty() {
                return 0.0;
            }
            v.sort_by(|a, b| a.total_cmp(b));
            v[v.len() / 2]
        };
        html.push_str(&format!(
            r#"<tr><td>{}</td><td>{:.4}</td><td>{:.3}</td><td>{:.1}</td><td>{}/{}</td><td>{}/{}</td></tr>"#,
            mode.id(),
            med(&mut totals),
            med_f64(&mut min_c3),
            med(&mut min_rgb),
            dup_fail,
            parents.len(),
            earth_fail,
            parents.len(),
        ));
    }
    html.push_str("</table>");

    for (rank, &orig) in order.iter().enumerate() {
        let pr = &parents[orig];
        let pool = &pr.result.restart_pool;
        html.push_str(r#"<div class="case">"#);
        html.push_str(&format!(
            r#"<h3>#{} · {} · seed {} · pool {} · <span class="time">{}</span></h3>"#,
            rank + 1,
            pr.case_id,
            pr.parent_seed,
            pool.len(),
            format_duration(pr.optimize_elapsed)
        ));
        html.push_str(r#"<div class="modes">"#);
        for &mode in modes {
            append_mode_card(
                html,
                mode,
                pool,
                c3_eval,
                sw,
                sh,
                &pr.breakdown,
                pr.result.sa_best_cost,
            );
        }
        html.push_str("</div>");
        html.push_str("</div>");
    }
}

#[derive(Serialize)]
struct CandidateJsonRow {
    case_id: String,
    channels: usize,
    parent_seed: u64,
    init_mode: String,
    restart_id: u32,
    rank_by_total: usize,
    rank_by_lex_v1: usize,
    hex_colors: Vec<String>,
    dominant_names: Vec<String>,
    families: Vec<String>,
    total_loss: f32,
    min_c3_name_distance: f64,
    p10_c3_name_distance: f64,
    mean_c3_name_distance: f64,
    min_display_rgb_distance: f32,
    min_oklab_distance: f64,
    duplicate_family_pair_count: usize,
    earth_term_mass: f64,
    min_srgb_saturation: f32,
    min_oklab_chroma: f32,
}

#[derive(Serialize)]
struct ReviewModeMeta {
    id: String,
    label: String,
}

#[derive(Serialize)]
struct ReviewMethodPalette {
    mode_id: String,
    mode_label: String,
    restart_id: u32,
    hex_colors: Vec<String>,
    dominant_names: Vec<String>,
    families: Vec<String>,
    total_loss: f32,
    min_c3_name_distance: f64,
    min_display_rgb_distance: f32,
    duplicate_family_pair_count: usize,
    earth_term_mass: f64,
}

#[derive(Serialize)]
struct ReviewCase {
    case_id: String,
    channels: usize,
    parent_seed: u64,
    methods: Vec<ReviewMethodPalette>,
}

#[derive(Serialize)]
struct ReviewPayload {
    run_id: String,
    init_mode: String,
    modes: Vec<ReviewModeMeta>,
    cases: Vec<ReviewCase>,
}

fn build_review_payload(
    batches: &[StudyBatch],
    review_methods: &[ReviewMethodSpec],
    init_mode: PaletteInitMode,
    c3_eval: &C3,
    spatial_w: f32,
) -> ReviewPayload {
    let lum = vec![0.50f32, 0.92];
    let mut cases = Vec::new();
    for batch in batches {
        for pr in &batch.parents {
            let pool = &pr.result.restart_pool;
            let n = batch.channels;
            let locked = vec![false; n];
            let excluded: std::collections::HashSet<usize> = pr
                .result
                .excluded_colors_indices
                .iter()
                .map(|&x| x as usize)
                .collect();

            // Shared base: total-loss winner from the polished restart pool.
            let Some(base_idx) = select_best_restart(pool, PaletteSelectionMode::TotalLoss) else {
                continue;
            };
            let base_oklab = pool[base_idx].oklab.clone();
            let base_restart = pool[base_idx].restart_id;

            let mut methods = Vec::new();
            for &spec in review_methods {
                match spec {
                    ReviewMethodSpec::Objective(obj) => {
                        let run_ref = if obj == pr.report_objective {
                            &pr.result
                        } else if let Some((_, alt)) =
                            pr.alt_objectives.iter().find(|(o, _)| *o == obj)
                        {
                            alt
                        } else {
                            continue;
                        };
                        let bd = evaluate_palette_objective_breakdown(
                            c3_eval,
                            &run_ref.oklab_best,
                            &run_ref.intensity_arc,
                            1.0,
                            spatial_w,
                            &run_ref.excluded_colors_indices,
                            &run_ref.color_name_indices,
                        );
                        let d = compute_diagnostics(c3_eval, &run_ref.oklab_best, &bd);
                        methods.push(ReviewMethodPalette {
                            mode_id: spec.id().to_string(),
                            mode_label: spec.label().to_string(),
                            restart_id: 0,
                            hex_colors: hex_colors_from_oklab(&run_ref.oklab_best),
                            dominant_names: d.dominant_c3_names,
                            families: d.coarse_name_families,
                            total_loss: d.total_loss,
                            min_c3_name_distance: d.min_c3_name_distance,
                            min_display_rgb_distance: d.min_display_rgb_distance,
                            duplicate_family_pair_count: d.duplicate_family_pair_count,
                            earth_term_mass: d.earth_term_mass,
                        });
                    }
                    ReviewMethodSpec::Select(sel) => {
                        let Some(idx) = select_best_restart(pool, sel) else {
                            continue;
                        };
                        let rec = &pool[idx];
                        let d = &rec.diagnostics;
                        methods.push(ReviewMethodPalette {
                            mode_id: spec.id().to_string(),
                            mode_label: spec.label().to_string(),
                            restart_id: rec.restart_id,
                            hex_colors: hex_colors_from_oklab(&rec.oklab),
                            dominant_names: d.dominant_c3_names.clone(),
                            families: d.coarse_name_families.clone(),
                            total_loss: d.total_loss,
                            min_c3_name_distance: d.min_c3_name_distance,
                            min_display_rgb_distance: d.min_display_rgb_distance,
                            duplicate_family_pair_count: d.duplicate_family_pair_count,
                            earth_term_mass: d.earth_term_mass,
                        });
                    }
                    ReviewMethodSpec::Refine(refine) => {
                        let mut oklab = base_oklab.clone();
                        apply_palette_refine_ex(
                            &mut oklab,
                            &locked,
                            &lum,
                            c3_eval,
                            &pr.result.intensity_arc,
                            1.0,
                            spatial_w,
                            &excluded,
                            &pr.result.color_name_indices,
                            pr.parent_seed.wrapping_add(0xA11CE),
                            refine,
                            true,
                            false,
                            // Study review: allow polar to separate bad name pairs even if L_tot
                            // ticks up slightly (production apply_palette_refine stays strict).
                            matches!(refine, PaletteRefineMode::Polar | PaletteRefineMode::Hybrid),
                        );
                        let bd = evaluate_palette_objective_breakdown(
                            c3_eval,
                            &oklab,
                            &pr.result.intensity_arc,
                            1.0,
                            spatial_w,
                            &pr.result.excluded_colors_indices,
                            &pr.result.color_name_indices,
                        );
                        let d = compute_diagnostics(c3_eval, &oklab, &bd);
                        methods.push(ReviewMethodPalette {
                            mode_id: spec.id().to_string(),
                            mode_label: spec.label().to_string(),
                            restart_id: base_restart,
                            hex_colors: hex_colors_from_oklab(&oklab),
                            dominant_names: d.dominant_c3_names,
                            families: d.coarse_name_families,
                            total_loss: d.total_loss,
                            min_c3_name_distance: d.min_c3_name_distance,
                            min_display_rgb_distance: d.min_display_rgb_distance,
                            duplicate_family_pair_count: d.duplicate_family_pair_count,
                            earth_term_mass: d.earth_term_mass,
                        });
                    }
                }
            }
            if methods.is_empty() {
                continue;
            }
            cases.push(ReviewCase {
                case_id: pr.case_id.clone(),
                channels: batch.channels,
                parent_seed: pr.parent_seed,
                methods,
            });
        }
    }
    let run_id = format!(
        "{}_{}_{}",
        init_mode.id(),
        cases.len(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    ReviewPayload {
        run_id,
        init_mode: init_mode.id().to_string(),
        modes: review_methods
            .iter()
            .map(|m| ReviewModeMeta {
                id: m.id().to_string(),
                label: m.label().to_string(),
            })
            .collect(),
        cases,
    }
}

fn write_review_html(path: &PathBuf, payload: &ReviewPayload) {
    const TEMPLATE: &str = include_str!("palette_study_review.html");
    let json = serde_json::to_string(payload).expect("review json");
    // Escape </script> so embedded JSON cannot break out of the script tag.
    let safe = json.replace('<', "\\u003c");
    let html = TEMPLATE.replace("__REVIEW_DATA_JSON__", &safe);
    fs::write(path, html).expect("write review.html");
}

fn write_candidates_json(path: &PathBuf, batches: &[StudyBatch], init_mode: PaletteInitMode) {
    let mut rows = Vec::new();
    for batch in batches {
        for pr in &batch.parents {
            let pool = &pr.result.restart_pool;
            let by_total = top_k_indices_by_total(pool, pool.len());
            let by_lex = top_k_indices_by_lex(pool, pool.len());
            let mut rank_total = vec![0usize; pool.len()];
            let mut rank_lex = vec![0usize; pool.len()];
            for (rank, &i) in by_total.iter().enumerate() {
                rank_total[i] = rank + 1;
            }
            for (rank, &i) in by_lex.iter().enumerate() {
                rank_lex[i] = rank + 1;
            }
            for (i, rec) in pool.iter().enumerate() {
                let d = &rec.diagnostics;
                rows.push(CandidateJsonRow {
                    case_id: pr.case_id.clone(),
                    channels: batch.channels,
                    parent_seed: pr.parent_seed,
                    init_mode: init_mode.id().to_string(),
                    restart_id: rec.restart_id,
                    rank_by_total: rank_total[i],
                    rank_by_lex_v1: rank_lex[i],
                    hex_colors: hex_colors_from_oklab(&rec.oklab),
                    dominant_names: d.dominant_c3_names.clone(),
                    families: d.coarse_name_families.clone(),
                    total_loss: d.total_loss,
                    min_c3_name_distance: d.min_c3_name_distance,
                    p10_c3_name_distance: d.p10_c3_name_distance,
                    mean_c3_name_distance: d.mean_c3_name_distance,
                    min_display_rgb_distance: d.min_display_rgb_distance,
                    min_oklab_distance: d.min_oklab_distance,
                    duplicate_family_pair_count: d.duplicate_family_pair_count,
                    earth_term_mass: d.earth_term_mass,
                    min_srgb_saturation: d.min_srgb_saturation,
                    min_oklab_chroma: d.min_oklab_chroma,
                });
            }
        }
    }
    let json = serde_json::to_string_pretty(&rows).expect("json");
    fs::write(path, json).expect("write candidates.json");
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
  .case { margin: 1.5rem 0 2rem; padding: 12px; border: 1px solid #2a2a2a; border-radius: 10px; background: #161616; }
  .case h3 { font-size: 0.95rem; color: #cde; margin: 0 0 10px; }
  .modes { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 12px; }
  .mode-card { background: #1c1c1c; border: 1px solid #333; border-radius: 8px; padding: 10px; }
  .mode-title { font-size: 0.8rem; color: #9cf; font-weight: 600; margin-bottom: 4px; }
  .mode-title code { color: #7dcea0; }
  .badge { display: inline-block; font-size: 0.6rem; padding: 1px 5px; border-radius: 4px; margin: 1px 2px; background: #333; color: #bbb; }
  .badge.bad { background: #5c1a1a; color: #f88; }
  .badge.warn { background: #5c4a1a; color: #fd8; }
  .badge.ok { background: #1a3c2a; color: #8d8; }
  .topk { margin-top: 10px; }
  .topk-title { font-size: 0.75rem; color: #888; margin-bottom: 4px; }
  .topk-row { display: flex; flex-wrap: wrap; gap: 8px; }
  .topk-item { background: #1a1a1a; border: 1px solid #2c2c2c; border-radius: 6px; padding: 6px; min-width: 140px; }
  table.summary { border-collapse: collapse; font-size: 0.75rem; margin: 8px 0 16px; }
  table.summary th, table.summary td { border: 1px solid #333; padding: 4px 8px; text-align: right; }
  table.summary th { color: #9cf; text-align: left; }
</style>
</head>
<body>
"#;

fn main() {
    let channel_counts = parse_channel_counts();
    let n_parents = parse_env_usize("PALETTE_STUDY_PARENTS", 10);
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
    let selection_modes = parse_selection_modes();
    let init_mode = parse_init_mode();
    let refine_mode = parse_refine_mode();
    let report_objective = parse_objective_mode();
    let review_methods = parse_review_methods();
    let review_objectives: Vec<PaletteObjectiveMode> = review_methods
        .iter()
        .filter_map(|m| match m {
            ReviewMethodSpec::Objective(o) => Some(*o),
            _ => None,
        })
        .collect();

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/palette_study");
    fs::create_dir_all(&out_dir).expect("mkdir");

    let channels_label: String = channel_counts
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let modes_label: String = selection_modes
        .iter()
        .map(|m| m.id())
        .collect::<Vec<_>>()
        .join(",");
    let review_label: String = review_methods
        .iter()
        .map(|m| m.id())
        .collect::<Vec<_>>()
        .join(",");

    eprintln!(
        "[palette_study] Nelder–Mead · max_iters={} restarts={} confusion={} · channels=[{}] · {} palettes/ch · spatial={} post={:?} · init={} refine={} objective={} · report_selection=[{}] · review=[{}]",
        max_iters,
        num_restarts,
        confusion_samples,
        channels_label,
        n_parents,
        include_spatial,
        postprocess,
        init_mode.id(),
        refine_mode.id(),
        report_objective.id(),
        modes_label,
        review_label
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
            init_mode,
            refine_mode,
            report_objective,
            &review_objectives,
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
        r#"<h1>Palette study (experiment foundation)</h1>
<p class="note">
  <strong>{total_palettes}</strong> optimized cases ({n_parents} per channel count: <strong>{channels_label}</strong>).
  <code>max_iters={max_iters}</code>, <code>restarts={num_restarts}</code>,
  spatial <strong>{spatial}</strong>, init <strong>{init}</strong>, refine <strong>{refine}</strong>,
  report objective <strong>{obj}</strong>, review methods <strong>{review}</strong>.
  <br/><br/>
  Production default remains mean-only C3 + Cartesian refine. review.html compares methods
  (identical hex palettes are auto-tied / skipped).
  <br/><br/>
  Total study wall time: <strong>{study_time}</strong>. Also wrote <code>candidates.json</code>
  and interactive <a href="review.html" style="color:#9cf">review.html</a> (vote which method looks best).
</p>
"#,
        spatial = if include_spatial { "on" } else { "off" },
        init = init_mode.id(),
        refine = refine_mode.id(),
        obj = report_objective.id(),
        review = review_label,
        study_time = format_duration(study_elapsed),
    ));

    for batch in &batches {
        append_section_html(&mut html, batch, &c3_eval, &selection_modes, sw, sh);
    }
    html.push_str("</body></html>");

    let path = out_dir.join("report.html");
    fs::write(&path, &html).expect("write report");
    let cand_path = out_dir.join("candidates.json");
    write_candidates_json(&cand_path, &batches, init_mode);
    let review_payload =
        build_review_payload(&batches, &review_methods, init_mode, &c3_eval, spatial_w);
    let review_json_path = out_dir.join("review_data.json");
    fs::write(
        &review_json_path,
        serde_json::to_string_pretty(&review_payload).expect("review_data json"),
    )
    .expect("write review_data.json");
    let review_path = out_dir.join("review.html");
    write_review_html(&review_path, &review_payload);
    eprintln!(
        "[palette_study] total study wall time {}",
        format_duration(study_elapsed)
    );
    eprintln!(
        "[palette_study] wrote {} ({} bytes)",
        path.display(),
        html.len()
    );
    eprintln!("[palette_study] wrote {}", cand_path.display());
    eprintln!(
        "[palette_study] wrote {} ({} cases for interactive review)",
        review_path.display(),
        review_payload.cases.len()
    );
    eprintln!(
        "[palette_study] open interactive review: file://{}",
        review_path.canonicalize().unwrap_or(review_path).display()
    );
}
