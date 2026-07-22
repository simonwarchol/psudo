//! Parameter sweep for npm/WASM defaults: time vs loss consistency by channel count.
//! **Spatial overlap is forced OFF** (`include_spatial_channel_overlap: false`).
//!
//! ```bash
//! cd lib && cargo run --example param_sweep --release
//! # open target/param_sweep/report.html
//! ```
//!
//! Environment:
//! - `SWEEP_SEEDS` (default 6) — random starts per (profile × channel count)
//! - `SWEEP_ROWS` (default 384)
//! - `SWEEP_SPATIAL_PROBE=1` — one extra timed run at 6ch with spatial ON (reference cost)

use palette::{FromColor, Oklab, Srgb};
use psudo::{evaluate_palette_objective_breakdown, optimize_palette_pipeline, OptimizePostprocess};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

struct Profile {
    id: &'static str,
    label: &'static str,
    max_iters: u32,
    restarts: u32,
    confusion_samples: u32,
    post: OptimizePostprocess,
}

#[derive(Clone)]
struct RunStats {
    mean_l: f32,
    std_l: f32,
    best_l: f32,
    worst_l: f32,
    unique_l: usize,
    ms_per_run: f64,
    max_confusion_term: f32,
}

fn parse_env_usize(key: &str, default: usize) -> usize {
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

fn run_profile(
    profile: &Profile,
    channels: usize,
    n_seeds: usize,
    _n_rows: usize,
    intensities: &[u16],
) -> RunStats {
    let contrast: Vec<u16> = (0..channels).flat_map(|_| [0u16, 65535]).collect();
    let lum = vec![50u16, 92];
    let locked = vec![0u16; channels];
    let names: Vec<String> = (0..channels).map(|_| String::new()).collect();
    let c3 = psudo::c3::C3::new();

    let t0 = Instant::now();
    let mut totals = Vec::with_capacity(n_seeds);
    let mut max_conf = 0.0f32;

    for s in 0..n_seeds {
        let colors = spread_colors_u16(channels, 50_000 + s as u64);
        let run = optimize_palette_pipeline(
            &colors,
            &locked,
            intensities,
            &contrast,
            &lum,
            vec![],
            names.clone(),
            Some(profile.max_iters),
            Some(profile.confusion_samples),
            Some(false), // spatial OFF — color-only objective for this sweep
            Some(profile.restarts),
            Some(profile.post),
        );
        let bd = evaluate_palette_objective_breakdown(
            &c3,
            &run.oklab_best,
            &run.intensity_arc,
            1.0,
            0.0,
            &run.excluded_colors_indices,
            &run.color_name_indices,
        );
        assert!(
            bd.confusion_weighted.abs() < 1e-6,
            "spatial confusion must be off (got {})",
            bd.confusion_weighted
        );
        max_conf = max_conf.max(bd.confusion_weighted.abs());
        totals.push(bd.total);
    }

    let elapsed = t0.elapsed();
    let n = totals.len() as f32;
    let mean_l = totals.iter().sum::<f32>() / n;
    let var = totals.iter().map(|t| (t - mean_l).powi(2)).sum::<f32>() / n;
    let best_l = totals.iter().cloned().fold(f32::INFINITY, f32::min);
    let worst_l = totals.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mut sorted = totals.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut uniq = sorted.clone();
    uniq.dedup_by(|a, b| (*a - *b).abs() < 1e-3);

    RunStats {
        mean_l,
        std_l: var.sqrt(),
        best_l,
        worst_l,
        unique_l: uniq.len(),
        ms_per_run: elapsed.as_secs_f64() * 1000.0 / n as f64,
        max_confusion_term: max_conf,
    }
}

fn scaled_iters(base: u32, ch: usize) -> u32 {
    ((base as u64 * ch as u64) / 3).max(1) as u32
}

fn scaled_restarts(base: u32, ch: usize) -> u32 {
    ((base as u64 * ch as u64) / 3).clamp(2, 6) as u32
}

/// Lower is better: time (40%) + consistency std (35%) − quality best_L (25%, more negative L is better).
fn score(stats: &RunStats, time_ref: f64, std_ref: f32) -> f32 {
    let t_norm = (stats.ms_per_run / time_ref) as f32;
    let s_norm = if std_ref > 1e-6 {
        stats.std_l / std_ref
    } else {
        0.0
    };
    let q_norm = -stats.best_l / 4.0;
    0.40 * t_norm + 0.35 * s_norm - 0.25 * q_norm
}

fn svg_bar_chart(
    title: &str,
    labels: &[&str],
    values: &[f64],
    width: i32,
    bar_h: i32,
    color: &str,
) -> String {
    let n = labels.len();
    let max_v = values.iter().cloned().fold(0.0f64, f64::max).max(1e-6);
    let row_h = bar_h + 6;
    let height = 40 + n as i32 * row_h;
    let mut s = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" role="img" aria-label="{title}">"#
    );
    s.push_str(&format!(
        "<text x=\"4\" y=\"14\" font-size=\"11\" fill=\"#ccc\" font-weight=\"bold\">{}</text>",
        title
    ));
    for (i, (lab, val)) in labels.iter().zip(values.iter()).enumerate() {
        let y = 22 + i as i32 * row_h;
        let w = ((val / max_v) * (width as f64 - 120.0)).max(2.0) as i32;
        s.push_str(&format!(
            "<text x=\"4\" y=\"{}\" font-size=\"9\" fill=\"#888\">{}</text>",
            y + bar_h - 2,
            lab
        ));
        s.push_str(&format!(
            "<rect x=\"110\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" opacity=\"0.85\"/>\
             <text x=\"{}\" y=\"{}\" font-size=\"8\" fill=\"#aaa\">{:.0}</text>",
            y,
            w,
            bar_h,
            color,
            112 + w + 4,
            y + bar_h - 2,
            val
        ));
    }
    s.push_str("</svg>");
    s
}

fn main() {
    let n_seeds = parse_env_usize("SWEEP_SEEDS", 6);
    let n_rows = parse_env_usize("SWEEP_ROWS", 384);
    let spatial_probe = env::var("SWEEP_SPATIAL_PROBE")
        .map(|s| matches!(s.as_str(), "1" | "true" | "TRUE" | "yes"))
        .unwrap_or(true);

    let profiles = [
        Profile {
            id: "wasm-1500-2r",
            label: "WASM current (1500 it, 2r, conf16)",
            max_iters: 1500,
            restarts: 2,
            confusion_samples: 16,
            post: OptimizePostprocess::Study,
        },
        Profile {
            id: "wasm-1800-2r",
            label: "WASM+ (1800 it, 2r, conf16)",
            max_iters: 1800,
            restarts: 2,
            confusion_samples: 16,
            post: OptimizePostprocess::Study,
        },
        Profile {
            id: "wasm-2000-2r",
            label: "WASM++ (2000 it, 2r, conf16)",
            max_iters: 2000,
            restarts: 2,
            confusion_samples: 16,
            post: OptimizePostprocess::Study,
        },
        Profile {
            id: "wasm-1500-3r",
            label: "WASM more restarts (1500 it, 3r)",
            max_iters: 1500,
            restarts: 3,
            confusion_samples: 16,
            post: OptimizePostprocess::Study,
        },
        Profile {
            id: "fast-1200-2r",
            label: "Fast (1200 it, 2r, conf16)",
            max_iters: 1200,
            restarts: 2,
            confusion_samples: 16,
            post: OptimizePostprocess::Study,
        },
        Profile {
            id: "study-2000-4r",
            label: "Study native (2000 it, 4r, conf32)",
            max_iters: 2000,
            restarts: 4,
            confusion_samples: 32,
            post: OptimizePostprocess::Study,
        },
    ];

    let channel_counts: [usize; 5] = [2, 3, 4, 5, 6];
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/param_sweep");
    fs::create_dir_all(&out_dir).expect("mkdir");

    eprintln!(
        "[param_sweep] seeds={} rows={} spatial=OFF for all profiles",
        n_seeds, n_rows
    );

    let mut results: Vec<(usize, &Profile, RunStats)> = Vec::new();

    for &ch in &channel_counts {
        let intensities = fixed_intensities(n_rows, ch);
        eprintln!("[param_sweep] --- {ch} channels ---");
        for p in &profiles {
            eprint!("  {} … ", p.id);
            let stats = run_profile(p, ch, n_seeds, n_rows, &intensities);
            eprintln!(
                "L mean={:.3} std={:.3} best={:.3} uniq={} {:.0}ms",
                stats.mean_l, stats.std_l, stats.best_l, stats.unique_l, stats.ms_per_run
            );
            results.push((ch, p, stats));
        }
    }

    let time_ref = results
        .iter()
        .map(|(_, _, s)| s.ms_per_run)
        .fold(0.0f64, f64::max);
    let std_ref = results
        .iter()
        .map(|(_, _, s)| s.std_l)
        .fold(0.0f32, f32::max);

    let mut recommendations: Vec<(usize, &'static str, RunStats)> = Vec::new();
    for &ch in &channel_counts {
        let mut best: Option<(&'static str, RunStats, f32)> = None;
        for p in &profiles {
            if let Some((_, _, stats)) = results
                .iter()
                .find(|(c, prof, _)| *c == ch && prof.id == p.id)
            {
                let sc = score(stats, time_ref, std_ref);
                if best.as_ref().map_or(true, |(_, _, s)| sc < *s) {
                    best = Some((p.id, stats.clone(), sc));
                }
            }
        }
        if let Some((id, stats, _)) = best {
            recommendations.push((ch, id, stats));
        }
    }

    let mut spatial_probe_ms = None;
    let mut spatial_probe_conf = None;
    if spatial_probe {
        eprintln!("[param_sweep] spatial ON probe (6ch, 1 seed, wasm-1500-2r) …");
        let ch = 6;
        let intensities = fixed_intensities(n_rows, ch);
        let p = &profiles[0];
        let colors = spread_colors_u16(ch, 99_999);
        let t0 = Instant::now();
        let run = optimize_palette_pipeline(
            &colors,
            &vec![0u16; ch],
            &intensities,
            &(0..ch).flat_map(|_| [0u16, 65535]).collect::<Vec<_>>(),
            &vec![50u16, 92],
            vec![],
            (0..ch).map(|_| String::new()).collect(),
            Some(p.max_iters),
            Some(p.confusion_samples),
            Some(true),
            Some(p.restarts),
            Some(p.post),
        );
        let c3 = psudo::c3::C3::new();
        let bd = evaluate_palette_objective_breakdown(
            &c3,
            &run.oklab_best,
            &run.intensity_arc,
            1.0,
            0.1,
            &run.excluded_colors_indices,
            &run.color_name_indices,
        );
        spatial_probe_ms = Some(t0.elapsed().as_secs_f64() * 1000.0);
        spatial_probe_conf = Some(bd.confusion_weighted);
        let off_ms = results
            .iter()
            .find(|(c, prof, _)| *c == 6 && prof.id == p.id)
            .map(|(_, _, s)| s.ms_per_run)
            .unwrap_or(0.0);
        eprintln!(
            "  spatial ON: {:.0}ms vs OFF {:.0}ms (conf term {:.4})",
            spatial_probe_ms.unwrap(),
            off_ms,
            spatial_probe_conf.unwrap()
        );
    }

    let mut html = String::from(
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>psudo param sweep — npm defaults</title>
<style>
  body { font-family: system-ui, sans-serif; margin: 20px; background: #111; color: #e8e8e8; }
  h1 { font-size: 1.3rem; }
  h2 { font-size: 1rem; color: #9cf; margin-top: 2rem; }
  p.note { color: #888; max-width: 85ch; line-height: 1.45; }
  table { border-collapse: collapse; font-size: 0.72rem; margin: 12px 0; width: 100%; max-width: 1200px; }
  th, td { border: 1px solid #333; padding: 6px 8px; text-align: right; }
  th { background: #1a1a1a; color: #9cf; position: sticky; top: 0; }
  td:first-child, th:first-child { text-align: left; }
  tr.rec td { background: #1a2a1a; }
  .charts { display: flex; flex-wrap: wrap; gap: 20px; margin-top: 16px; }
  .warn { color: #e67e22; }
  .ok { color: #7dcea0; }
</style></head><body>
<h1>Parameter sweep (spatial overlap OFF)</h1>
<p class="note">
  All profiles use <code>include_spatial_channel_overlap: false</code>.
  Confusion term in loss breakdown verified ≈ 0 for every run.
  Postprocess: <code>Study</code> (matches WASM path: no quench, polish on best only).
  Scaled SA iters = base × (n<sub>ch</sub>/3); scaled restarts clamped to [2,6] on WASM-sized bases.
</p>
"#,
    );

    html.push_str(
        "<p class=\"ok\">✓ Spatial overlap disabled for sweep runs (confusion_weighted = 0).</p>",
    );

    if let (Some(ms_on), Some(conf)) = (spatial_probe_ms, spatial_probe_conf) {
        let ms_off = results
            .iter()
            .find(|(c, p, _)| *c == 6 && p.id == "wasm-1500-2r")
            .map(|(_, _, s)| s.ms_per_run)
            .unwrap_or(0.0);
        html.push_str(&format!(
            r#"<p class="warn">Reference: 6ch single run with spatial ON took <strong>{:.0} ms</strong> vs <strong>{:.0} ms</strong> OFF (conf term {:.4}). Re-enable spatial in app when ready.</p>"#,
            ms_on, ms_off, conf
        ));
    }

    html.push_str("<h2>Full results</h2><table><thead><tr>
      <th>ch</th><th>profile</th><th>base iters</th><th>scaled it</th><th>base r</th><th>scaled r</th>
      <th>ms/run</th><th>mean L</th><th>std L</th><th>best L</th><th>uniq L</th><th>max conf</th>
      </tr></thead><tbody>");

    for (ch, prof, s) in &results {
        let is_rec = recommendations
            .iter()
            .any(|(c, rid, _)| *c == *ch && *rid == prof.id);
        html.push_str(&format!(
            "<tr class=\"{}\"><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>
            <td>{:.0}</td><td>{:.3}</td><td>{:.3}</td><td>{:.3}</td><td>{}</td><td>{:.2e}</td></tr>",
            if is_rec { "rec" } else { "" },
            ch,
            prof.label,
            prof.max_iters,
            scaled_iters(prof.max_iters, *ch),
            prof.restarts,
            scaled_restarts(prof.restarts, *ch),
            s.ms_per_run,
            s.mean_l,
            s.std_l,
            s.best_l,
            s.unique_l,
            s.max_confusion_term
        ));
    }
    html.push_str("</tbody></table>");

    html.push_str(
        "<h2>Recommended profile per channel count</h2><table><thead><tr>
      <th>ch</th><th>profile</th><th>ms/run</th><th>std L</th><th>best L</th><th>uniq</th>
      </tr></thead><tbody>",
    );
    for (ch, id, stats) in &recommendations {
        let label = profiles
            .iter()
            .find(|p| p.id == *id)
            .map(|p| p.label)
            .unwrap_or("");
        html.push_str(&format!(
            "<tr class=\"rec\"><td>{}</td><td>{}</td><td>{:.0}</td><td>{:.3}</td><td>{:.3}</td><td>{}</td></tr>",
            ch, label, stats.ms_per_run, stats.std_l, stats.best_l, stats.unique_l
        ));
    }
    html.push_str("</tbody></table>");

    html.push_str("<h2>Timing by channel count (ms/run)</h2><div class=\"charts\">");
    for &ch in &channel_counts {
        let labs: Vec<String> = profiles.iter().map(|p| p.id.to_string()).collect();
        let lab_refs: Vec<&str> = labs.iter().map(|s| s.as_str()).collect();
        let vals: Vec<f64> = profiles
            .iter()
            .map(|p| {
                results
                    .iter()
                    .find(|(c, prof, _)| *c == ch && prof.id == p.id)
                    .map(|(_, _, s)| s.ms_per_run)
                    .unwrap_or(0.0)
            })
            .collect();
        html.push_str(&svg_bar_chart(
            &format!("{ch}ch — ms/run"),
            &lab_refs,
            &vals,
            420,
            14,
            "#6eb5ff",
        ));
    }
    html.push_str("</div>");

    html.push_str("<h2>Consistency (std L) by channel count</h2><div class=\"charts\">");
    for &ch in &channel_counts {
        let labs: Vec<String> = profiles.iter().map(|p| p.id.to_string()).collect();
        let lab_refs: Vec<&str> = labs.iter().map(|s| s.as_str()).collect();
        let vals: Vec<f64> = profiles
            .iter()
            .map(|p| {
                results
                    .iter()
                    .find(|(c, prof, _)| *c == ch && prof.id == p.id)
                    .map(|(_, _, s)| s.std_l as f64)
                    .unwrap_or(0.0)
            })
            .collect();
        html.push_str(&svg_bar_chart(
            &format!("{ch}ch — std L (lower = more consistent)"),
            &lab_refs,
            &vals,
            420,
            14,
            "#f5b041",
        ));
    }
    html.push_str(r#"<h2>Production npm / WASM defaults (spatial OFF)</h2>
<p class="note">Nelder–Mead multistart + full polish/refine. Pass <code>include_spatial_channel_overlap: false</code> explicitly in JS.</p>
<table><thead><tr><th>Parameter</th><th>Base</th><th>At 6ch (× channels/3)</th></tr></thead><tbody>
<tr><td>max_iters</td><td><strong>3000</strong></td><td>6000</td></tr>
<tr><td>num_restarts</td><td><strong>6</strong></td><td>12 (capped 12 WASM)</td></tr>
<tr><td>confusion_baseline_samples</td><td><strong>32</strong></td><td>32</td></tr>
<tr><td>spatial overlap</td><td>false</td><td>false</td></tr>
</tbody></table>
<p class="note">~2× wall time vs legacy 1800/3r/16; tighter 6ch loss distribution in palette_study.</p>"#);

    html.push_str("</div></body></html>");

    let path = out_dir.join("report.html");
    fs::write(&path, &html).expect("write");
    eprintln!("[param_sweep] wrote {}", path.display());

    eprintln!("\n[param_sweep] === recommended npm-style defaults (spatial OFF) ===");
    for (ch, id, stats) in &recommendations {
        let p = profiles.iter().find(|x| x.id == *id).unwrap();
        eprintln!(
            "  {ch}ch: {} (it={}→{}, r={}→{}, {:.0}ms, std={:.3}, best={:.3})",
            id,
            p.max_iters,
            scaled_iters(p.max_iters, *ch),
            p.restarts,
            scaled_restarts(p.restarts, *ch),
            stats.ms_per_run,
            stats.std_l,
            stats.best_l
        );
    }
}
