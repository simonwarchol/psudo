//! Compare argmin global-search methods under npm-like budgets (spatial OFF).
//!
//! ```bash
//! cd lib && cargo run --example solver_benchmark --release
//! # open target/solver_benchmark/report.html
//! ```
//!
//! Environment:
//! - `BENCH_SEEDS` (default 6)
//! - `BENCH_ROWS` (default 384)
//! - `BENCH_MAX_ITERS` (default 1800) — nominal SA iteration budget per run
//! - `BENCH_RESTARTS` (default 2) — SA / pipeline reference only

use palette::{FromColor, Oklab, Srgb};
use psudo::{
    objective_total_for_oklab, optimize_palette_pipeline, optimize_palette_with_solver,
    scaled_solver_iters, OptimizePostprocess, PaletteArgminSolver,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone)]
struct RunStats {
    mean_l: f32,
    std_l: f32,
    best_l: f32,
    worst_l: f32,
    ms_per_run: f64,
    scaled_iters: u32,
}

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

fn spread_colors_u16(channels: usize, seed: u64) -> Vec<u16> {
    let mut rng = StdRng::seed_from_u64(seed);
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

fn fixed_intensities(n_rows: usize, channels: usize) -> Vec<u16> {
    let mut rng = StdRng::seed_from_u64(9000);
    let mut out = vec![0u16; n_rows * channels];
    for ch in 0..channels {
        for row in 0..n_rows {
            out[ch * n_rows + row] = rng.gen_range(2000u16..62000u16);
        }
    }
    out
}

fn stats_from_totals(totals: &[f32], elapsed_ms: f64) -> RunStats {
    let n = totals.len().max(1) as f32;
    let mean = totals.iter().sum::<f32>() / n;
    let var = totals
        .iter()
        .map(|t| {
            let d = *t - mean;
            d * d
        })
        .sum::<f32>()
        / n;
    RunStats {
        mean_l: mean,
        std_l: var.sqrt(),
        best_l: totals.iter().cloned().fold(f32::INFINITY, f32::min),
        worst_l: totals.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
        ms_per_run: elapsed_ms / totals.len().max(1) as f64,
        scaled_iters: 0,
    }
}

fn run_solver(
    solver: PaletteArgminSolver,
    channels: usize,
    n_seeds: usize,
    n_rows: usize,
    max_iters: u32,
    restarts: u32,
    intensities: &[u16],
) -> RunStats {
    let contrast: Vec<u16> = (0..channels).flat_map(|_| [0u16, 65535]).collect();
    let lum = vec![45u16, 92];
    let locked = vec![0u16; channels];
    let names: Vec<String> = (0..channels).map(|_| String::new()).collect();
    let c3 = psudo::c3::C3::new();

    let t0 = Instant::now();
    let mut totals = Vec::with_capacity(n_seeds);

    for s in 0..n_seeds {
        let colors = spread_colors_u16(channels, 70_000 + s as u64);
        let run = if solver == PaletteArgminSolver::SimulatedAnnealing
            && restarts == 2
            && max_iters == 1800
        {
            // Production-style reference: 2× SA + Study post (same as param_sweep "study" profile).
            optimize_palette_pipeline(
                &colors,
                &locked,
                intensities,
                &contrast,
                &lum,
                vec![],
                names.clone(),
                Some(max_iters),
                Some(16),
                Some(false),
                Some(restarts),
                Some(OptimizePostprocess::Study),
            )
        } else if solver == PaletteArgminSolver::SimulatedAnnealing {
            optimize_palette_with_solver(
                &colors,
                &locked,
                intensities,
                &contrast,
                &lum,
                vec![],
                names.clone(),
                Some(max_iters),
                Some(16),
                Some(false),
                Some(restarts),
                PaletteArgminSolver::SimulatedAnnealing,
                None,
            )
        } else {
            optimize_palette_with_solver(
                &colors,
                &locked,
                intensities,
                &contrast,
                &lum,
                vec![],
                names.clone(),
                Some(max_iters),
                Some(16),
                Some(false),
                None,
                solver,
                None,
            )
        };
        let excluded: HashSet<usize> = run
            .excluded_colors_indices
            .iter()
            .map(|&x| x as usize)
            .collect();
        let bd = objective_total_for_oklab(
            &c3,
            &run.oklab_best,
            &run.intensity_arc,
            1.0,
            0.0,
            &excluded,
            &run.color_name_indices,
        );
        totals.push(bd.total);
    }

    let mut st = stats_from_totals(&totals, t0.elapsed().as_secs_f64() * 1000.0);
    st.scaled_iters = scaled_solver_iters(solver, max_iters);
    st
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn main() {
    let n_seeds = parse_env_usize("BENCH_SEEDS", 6);
    let n_rows = parse_env_usize("BENCH_ROWS", 384);
    let max_iters = parse_env_u32("BENCH_MAX_ITERS", 1800);
    let restarts = parse_env_u32("BENCH_RESTARTS", 2);

    let out_dir = PathBuf::from("target/solver_benchmark");
    fs::create_dir_all(&out_dir).expect("create output dir");

    let channel_counts: Vec<usize> = (2..=6).collect();
    let mut solvers: Vec<PaletteArgminSolver> = PaletteArgminSolver::ALL.to_vec();
    // Rename SA row in report: pipeline reference is the 2-restart Study path.
    solvers.retain(|s| *s != PaletteArgminSolver::SimulatedAnnealing);
    solvers.insert(0, PaletteArgminSolver::SimulatedAnnealing);

    println!(
        "solver_benchmark: {} seeds, {} rows, SA budget {} iters, SA restarts {}",
        n_seeds, n_rows, max_iters, restarts
    );

    let intensities_cache: Vec<Vec<u16>> = channel_counts
        .iter()
        .map(|&ch| fixed_intensities(n_rows, ch))
        .collect();

    let mut rows_html = String::new();
    let mut summary: Vec<(PaletteArgminSolver, f32, f32, f64, f64)> = Vec::new();

    for &solver in &solvers {
        let label = if solver == PaletteArgminSolver::SimulatedAnnealing {
            format!(
                "{} ({}× restart, Study post)",
                solver.label(),
                restarts
            )
        } else {
            solver.label().to_string()
        };
        println!("--- {}", label);

        let mut agg_std = 0.0f32;
        let mut agg_best = 0.0f32;
        let mut agg_ms = 0.0f64;
        let mut cells = String::new();
        let mut st_6ch: Option<RunStats> = None;

        for (idx, &channels) in channel_counts.iter().enumerate() {
            let st = run_solver(
                solver,
                channels,
                n_seeds,
                n_rows,
                max_iters,
                restarts,
                &intensities_cache[idx],
            );
            println!(
                "  {}ch: L={:.3}±{:.3} best={:.3} {:.0}ms/run (iters={})",
                channels, st.mean_l, st.std_l, st.best_l, st.ms_per_run, st.scaled_iters
            );
            if channels == 6 {
                st_6ch = Some(st.clone());
            }
            agg_std += st.std_l;
            agg_best += st.best_l;
            agg_ms += st.ms_per_run;
            cells.push_str(&format!(
                "<td>{:.3}±{:.3}</td><td>{:.3}</td><td>{:.0}</td>",
                st.mean_l, st.std_l, st.best_l, st.ms_per_run
            ));
        }
        let nch = channel_counts.len() as f32;
        agg_std /= nch;
        agg_best /= nch;
        agg_ms /= nch as f64;
        let st6 = st_6ch.expect("6ch row");
        let score =
            st6.mean_l as f64 + 2.0 * st6.std_l as f64 + 0.001 * st6.ms_per_run;
        summary.push((solver, agg_std, agg_best, agg_ms, score));

        rows_html.push_str(&format!(
            "<tr><th>{}</th><td><code>{}</code></td><td>{}</td>{}<td>{:.3}</td><td>{:.3}</td><td>{:.0}</td><td>{:.4}</td></tr>\n",
            esc(&label),
            solver.id(),
            scaled_solver_iters(solver, max_iters),
            cells,
            agg_std,
            agg_best,
            agg_ms,
            score,
        ));
    }

    summary.sort_by(|a, b| a.4.partial_cmp(&b.4).unwrap_or(std::cmp::Ordering::Equal));
    let (best_solver, _, _, _, _) = summary[0];

    let header_cells: String = channel_counts
        .iter()
        .map(|ch| {
            format!(
                "<th colspan=\"3\">{} ch</th>",
                ch
            )
        })
        .collect();
    let subheader: String = channel_counts
        .iter()
        .map(|_| "<th>mean±σ</th><th>best</th><th>ms</th>")
        .collect::<Vec<_>>()
        .join("");

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<title>Argmin solver benchmark</title>
<style>
body {{ font-family: system-ui, sans-serif; margin: 1.5rem; max-width: 1400px; }}
table {{ border-collapse: collapse; font-size: 13px; width: 100%; }}
th, td {{ border: 1px solid #ccc; padding: 4px 8px; text-align: right; }}
th:first-child, td:first-child {{ text-align: left; }}
tr:nth-child(even) {{ background: #f8f8f8; }}
.note {{ color: #444; max-width: 72em; line-height: 1.45; }}
.best {{ background: #e8f5e9; }}
</style>
</head>
<body>
<h1>Argmin solver benchmark</h1>
<p class="note">
Spatial overlap <strong>off</strong>. Nominal SA budget: <strong>{max_iters}</strong> iterations
(scaled per solver). Seeds per cell: <strong>{n_seeds}</strong>. Rows: <strong>{n_rows}</strong>.
SA reference uses <strong>{restarts}×</strong> restart + Study polish/refine.
Lower <code>L_tot</code> is better; lower σ across seeds is more consistent.
Score per channel = mean L + 2σ + 0.001×ms; table “score” column is the 6-channel row only (lower is better).
Recommended: <strong>{rec}</strong> (<code>{rec_id}</code>).
</p>
<p class="note">
Not benchmarked: 1-D solvers (Brent, golden-section), Newton/Gauss–Newton (need Hessian),
BFGS (inverse-Hessian type mismatch with <code>Vec&lt;f32&gt;</code> params), nonlinear CG
(diverged to <code>−∞</code> with finite-difference gradients on this non-smooth objective).
Gradient runs use backtracking Armijo line search, not More–Thuente.
</p>
<table>
<thead>
<tr><th rowspan="2">Solver</th><th rowspan="2">id</th><th rowspan="2">argmin iters</th>{header_cells}
<th rowspan="2">avg σ</th><th rowspan="2">avg best</th><th rowspan="2">avg ms</th><th rowspan="2">score</th></tr>
<tr>{subheader}</tr>
</thead>
<tbody>
{rows}
</tbody>
</table>
</body>
</html>"#,
        max_iters = max_iters,
        n_seeds = n_seeds,
        n_rows = n_rows,
        restarts = restarts,
        rec = if best_solver == PaletteArgminSolver::SimulatedAnnealing {
            format!("{} ({}× restart, Study post)", best_solver.label(), restarts)
        } else {
            best_solver.label().to_string()
        },
        rec_id = best_solver.id(),
        header_cells = header_cells,
        subheader = subheader,
        rows = rows_html.replace(
            &format!("<tr><th>{}</th>", esc(
                if best_solver == PaletteArgminSolver::SimulatedAnnealing {
                    format!("{} ({}× restart, Study post)", best_solver.label(), restarts)
                } else {
                    best_solver.label().to_string()
                }
                .as_str()
            )),
            &format!("<tr class=\"best\"><th>{}</th>", esc(
                if best_solver == PaletteArgminSolver::SimulatedAnnealing {
                    format!("{} ({}× restart, Study post)", best_solver.label(), restarts)
                } else {
                    best_solver.label().to_string()
                }
                .as_str()
            )),
        ),
    );

    let report_path = out_dir.join("report.html");
    fs::write(&report_path, html).expect("write report");
    println!("\nWrote {}", report_path.display());
}
