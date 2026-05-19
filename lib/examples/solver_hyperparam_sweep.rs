//! Hyperparameter sweep for promising global solvers (6-channel focus).
//!
//! ```bash
//! pnpm run solver-hyperparam        # full grid (~30–60 min)
//! SWEEP_QUICK=1 pnpm run solver-hyperparam   # smaller grid
//! # open lib/target/solver_hyperparam/report.html
//! ```
//!
//! Scoring: `mean_L + 2×σ + 0.001×ms` on 6-channel palettes (lower is better).
//! Excludes polish-only — it wins the old σ+time score but gives poor actual loss.

use palette::{FromColor, Oklab, Srgb};
use psudo::{
    objective_total_for_oklab, optimize_palette_pipeline, optimize_palette_with_solver,
    OptimizePostprocess, PaletteArgminSolver, PaletteSolverParams,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

const CHANNELS: usize = 6;

#[derive(Clone)]
struct RunStats {
    mean_l: f32,
    std_l: f32,
    best_l: f32,
    ms_per_run: f64,
}

#[derive(Clone)]
struct Config {
    id: String,
    label: String,
    /// SA via production pipeline (`optimize_palette_pipeline` + Study post).
    sa_pipeline: Option<(u32, u32)>,
    /// Other solvers via `optimize_palette_with_solver`.
    argmin: Option<(PaletteArgminSolver, u32, u32, PaletteSolverParams)>,
}

fn parse_env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
        .max(1)
}

fn scaled_iters(base: u32, ch: usize) -> u32 {
    ((base as u64 * ch as u64) / 3).max(1) as u32
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

fn quality_score(mean_l: f32, std_l: f32, ms: f64) -> f64 {
    mean_l as f64 + 2.0 * std_l as f64 + 0.001 * ms
}

fn run_config(cfg: &Config, n_seeds: usize, n_rows: usize, intensities: &[u16]) -> RunStats {
    let contrast: Vec<u16> = (0..CHANNELS).flat_map(|_| [0u16, 65535]).collect();
    let lum = vec![45u16, 92];
    let locked = vec![0u16; CHANNELS];
    let names: Vec<String> = (0..CHANNELS).map(|_| String::new()).collect();
    let c3 = psudo::c3::C3::new();

    let t0 = Instant::now();
    let mut totals = Vec::with_capacity(n_seeds);

    for s in 0..n_seeds {
        let colors = spread_colors_u16(CHANNELS, 80_000 + s as u64);
        let run = if let Some((max_iters, restarts)) = cfg.sa_pipeline {
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
        } else if let Some((solver, budget, restarts, ref params)) = cfg.argmin {
            optimize_palette_with_solver(
                &colors,
                &locked,
                intensities,
                &contrast,
                &lum,
                vec![],
                names.clone(),
                Some(budget),
                Some(16),
                Some(false),
                Some(restarts),
                solver,
                Some(params.clone()),
            )
        } else {
            panic!("empty config {}", cfg.id);
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

    let n = totals.len() as f32;
    let mean = totals.iter().sum::<f32>() / n;
    let var = totals.iter().map(|t| (t - mean).powi(2)).sum::<f32>() / n;
    RunStats {
        mean_l: mean,
        std_l: var.sqrt(),
        best_l: totals.iter().cloned().fold(f32::INFINITY, f32::min),
        ms_per_run: t0.elapsed().as_secs_f64() * 1000.0 / n as f64,
    }
}

fn build_grid(quick: bool) -> Vec<Config> {
    let mut out = Vec::new();

    let sa_bases: &[u32] = if quick {
        &[1800, 2400, 3000]
    } else {
        &[1200, 1500, 1800, 2400, 3000]
    };
    let sa_restarts: &[u32] = if quick { &[2, 3] } else { &[1, 2, 3, 4] };

    for &base in sa_bases {
        for &r in sa_restarts {
            out.push(Config {
                id: format!("sa_{base}_r{r}"),
                label: format!("SA pipeline iters={base} restarts={r}"),
                sa_pipeline: Some((base, r)),
                argmin: None,
            });
        }
    }

    let sa_temps: &[f32] = if quick { &[10.0, 12.0, 16.0] } else { &[8.0, 10.0, 12.0, 16.0, 20.0] };
    for &t in sa_temps {
        let mut p = PaletteSolverParams::default();
        p.sa_initial_temp = Some(t);
        out.push(Config {
            id: format!("sa_t{t}_r2"),
            label: format!("SA T₀={t} budget=1800 restarts=2"),
            sa_pipeline: None,
            argmin: Some((
                PaletteArgminSolver::SimulatedAnnealing,
                1800,
                2,
                p,
            )),
        });
    }

    let nm_iters: &[u32] = if quick {
        &[900, 1200, 1800]
    } else {
        &[600, 900, 1200, 1800, 2400]
    };
    let nm_restarts: &[u32] = if quick { &[1, 2, 3] } else { &[1, 2, 3] };
    let nm_scales: &[f32] = if quick { &[1.0] } else { &[0.75, 1.0, 1.25] };

    for &it in nm_iters {
        let it6 = scaled_iters(it, CHANNELS);
        for &r in nm_restarts {
            for &sc in nm_scales {
                let mut p = PaletteSolverParams::default();
                p.argmin_max_iters = Some(it6);
                p.nm_perturb_scale = sc;
                out.push(Config {
                    id: format!("nm_i{it}_r{r}_s{sc}"),
                    label: format!("NM iters={it6} restarts={r} simplex×{sc}"),
                    sa_pipeline: None,
                    argmin: Some((PaletteArgminSolver::NelderMead, it, r, p)),
                });
            }
        }
    }

    let pso_iters: &[u32] = if quick {
        &[450, 800]
    } else {
        &[300, 450, 800, 1200]
    };
    let pso_parts: &[usize] = if quick { &[32] } else { &[20, 32, 48] };
    let pso_restarts: &[u32] = if quick { &[1, 2] } else { &[1, 2] };

    for &it in pso_iters {
        let it6 = scaled_iters(it, CHANNELS);
        for &parts in pso_parts {
            for &r in pso_restarts {
                let mut p = PaletteSolverParams::default();
                p.argmin_max_iters = Some(it6);
                p.pso_num_particles = Some(parts);
                out.push(Config {
                    id: format!("pso_i{it}_p{parts}_r{r}"),
                    label: format!("PSO iters={it6} particles={parts} restarts={r}"),
                    sa_pipeline: None,
                    argmin: Some((PaletteArgminSolver::ParticleSwarm, it, r, p)),
                });
            }
        }
    }

    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn main() {
    let quick = env::var("SWEEP_QUICK").ok().as_deref() == Some("1");
    let n_seeds = parse_env_usize("SWEEP_SEEDS", if quick { 8 } else { 10 });
    let n_rows = parse_env_usize("SWEEP_ROWS", 384);
    let grid = build_grid(quick);

    println!(
        "solver_hyperparam: {} configs, {} seeds, {}ch, quick={}",
        grid.len(),
        n_seeds,
        CHANNELS,
        quick
    );

    let intensities = fixed_intensities(n_rows, CHANNELS);
    let out_dir = PathBuf::from("target/solver_hyperparam");
    fs::create_dir_all(&out_dir).expect("mkdir");

    let mut rows: Vec<(Config, RunStats, f64)> = Vec::with_capacity(grid.len());

    for (i, cfg) in grid.iter().enumerate() {
        println!("[{}/{}] {}", i + 1, grid.len(), cfg.label);
        let st = run_config(cfg, n_seeds, n_rows, &intensities);
        let score = quality_score(st.mean_l, st.std_l, st.ms_per_run);
        println!(
            "  L={:.3}±{:.3} best={:.3} {:.0}ms score={:.3}",
            st.mean_l, st.std_l, st.best_l, st.ms_per_run, score
        );
        rows.push((cfg.clone(), st, score));
    }

    rows.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut tbody = String::new();
    for (i, (cfg, st, score)) in rows.iter().enumerate() {
        let cls = if i == 0 { " class=\"best\"" } else { "" };
        tbody.push_str(&format!(
            "<tr{cls}><td><code>{}</code></td><td>{}</td><td>{:.3}</td><td>{:.3}</td><td>{:.3}</td><td>{:.0}</td><td>{:.4}</td></tr>\n",
            esc(&cfg.id),
            esc(&cfg.label),
            st.mean_l,
            st.std_l,
            st.best_l,
            st.ms_per_run,
            score,
        ));
    }

    let (best_cfg, best_st, best_score) = &rows[0];

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<title>Solver hyperparameter sweep (6ch)</title>
<style>
body {{ font-family: system-ui, sans-serif; margin: 1.5rem; max-width: 1200px; }}
table {{ border-collapse: collapse; font-size: 13px; width: 100%; }}
th, td {{ border: 1px solid #ccc; padding: 6px 10px; text-align: right; }}
th:nth-child(1), td:nth-child(1), th:nth-child(2), td:nth-child(2) {{ text-align: left; }}
tr:nth-child(even) {{ background: #f8f8f8; }}
.best {{ background: #e8f5e9; }}
.note {{ color: #444; line-height: 1.5; max-width: 52em; }}
</style>
</head>
<body>
<h1>Solver hyperparameter sweep</h1>
<p class="note">
<strong>6 channels</strong>, spatial off, Study polish/refine, {n_seeds} seeds, {n_rows} intensity rows.
Score = mean L<sub>tot</sub> + 2σ + 0.001×ms (lower is better). Sorted by score.
</p>
<p class="note"><strong>Best:</strong> {best_label} (<code>{best_id}</code>) —
mean={mean:.3}, σ={std:.3}, best={best_l:.3}, {ms:.0} ms/run, score={score:.4}.</p>
<table>
<thead><tr>
<th>id</th><th>config</th><th>mean L</th><th>σ</th><th>best L</th><th>ms/run</th><th>score</th>
</tr></thead>
<tbody>
{tbody}
</tbody>
</table>
</body>
</html>"#,
        n_seeds = n_seeds,
        n_rows = n_rows,
        best_label = esc(&best_cfg.label),
        best_id = esc(&best_cfg.id),
        mean = best_st.mean_l,
        std = best_st.std_l,
        best_l = best_st.best_l,
        ms = best_st.ms_per_run,
        score = best_score,
        tbody = tbody,
    );

    let path = out_dir.join("report.html");
    fs::write(&path, html).expect("write html");
    println!("\nWrote {}", path.display());
}
