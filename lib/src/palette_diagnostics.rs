//! Post-hoc palette diagnostics, lexicographic selection, and Glasbey-style seeding.
//!
//! Used by the study harness; production objective / NM path stay unchanged.

use crate::c3::{self, RelatedTerm};
use crate::palette_eval::{
    fill_c3_labs_from_oklab, fill_display_srgb255, project_oklab_through_display,
};
use crate::{
    channel_srgb_saturation, oklab_chroma, PaletteObjectiveBreakdown, DEFAULT_MIN_OKLAB_CHROMA,
    MIN_DISPLAY_RGB_DISTANCE, MIN_SRGB_SATURATION,
};
use palette::{FromColor, Lab, Oklab, Srgb};
use rust_c3::C3_TERM_STRS;
use serde::Serialize;
use std::cmp::Ordering;

const C3_TERM_LIMIT: usize = 10;

/// Soft-bad earth / muddy C3 terms (selection / diagnostics only).
pub const EARTH_TONE_TERMS: &[&str] = &[
    "brown",
    "tan",
    "beige",
    "khaki",
    "mustard",
    "olive",
    "umber",
    "sepia",
    "ochre",
    "rust",
    "bronze",
    "darkbrown",
    "lightbrown",
    "yellowbrown",
    "orangebrown",
    "redbrown",
    "burntsienna",
    "terracotta",
    "sand",
    "taupe",
    "flesh",
    "skin",
    "armygreen",
    "sage",
    "puke",
    "mustardyellow",
    "goldenrod",
];

/// One restart / candidate with full quality diagnostics.
#[derive(Clone, Debug, Serialize)]
pub struct PaletteCandidateDiagnostics {
    pub total_loss: f32,
    pub mean_c3_name_distance: f64,
    pub min_c3_name_distance: f64,
    pub p10_c3_name_distance: f64,
    pub worst_name_pair: Option<(usize, usize)>,
    pub min_display_rgb_distance: f32,
    pub worst_rgb_pair: Option<(usize, usize)>,
    pub min_oklab_distance: f64,
    pub min_oklch_hue_gap_deg: f64,
    pub dominant_c3_names: Vec<String>,
    pub dominant_c3_scores: Vec<f64>,
    pub coarse_name_families: Vec<String>,
    pub duplicate_family_pair_count: usize,
    pub bad_name_pair_count: usize,
    pub earth_term_mass: f64,
    pub excluded_term_mass: f64,
    pub min_srgb_saturation: f32,
    pub min_oklab_chroma: f32,
    /// Sorted pairwise C3 name distances (ascending) for LexV2.
    pub pairwise_c3_distances_asc: Vec<f64>,
}

/// Multistart outcome retained for study selection / reporting.
#[derive(Clone, Debug, Serialize)]
pub struct RestartRecord {
    pub restart_id: u32,
    pub oklab: Vec<f32>,
    pub diagnostics: PaletteCandidateDiagnostics,
}

/// Final fold mode over a restart pool (study / experiment only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteSelectionMode {
    TotalLoss,
    LexV1,
    LexV2,
    NameTail,
}

impl PaletteSelectionMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::TotalLoss => "total",
            Self::LexV1 => "lex_v1",
            Self::LexV2 => "lex_v2",
            Self::NameTail => "name_tail",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TotalLoss => "Total loss",
            Self::LexV1 => "Lex V1",
            Self::LexV2 => "Lex V2",
            Self::NameTail => "Name-tail",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "total" | "total_loss" | "ltot" => Some(Self::TotalLoss),
            "lex_v1" | "lexv1" | "lex1" => Some(Self::LexV1),
            "lex_v2" | "lexv2" | "lex2" => Some(Self::LexV2),
            "name_tail" | "nametail" => Some(Self::NameTail),
            _ => None,
        }
    }
}

/// Coarse color-name family for duplicate detection.
pub fn coarse_name_family(term: &str) -> &'static str {
    let t = term.to_ascii_lowercase();
    // Order matters: more specific substrings before broad families.
    if matches!(
        t.as_str(),
        "white"
            | "grey"
            | "gray"
            | "black"
            | "silver"
            | "lightgrey"
            | "darkgrey"
            | "offwhite"
            | "cream"
            | "slate"
    ) || t.contains("grey")
        || t.contains("gray")
    {
        return "achromatic_family";
    }
    if matches!(
        t.as_str(),
        "red"
            | "pink"
            | "magenta"
            | "rose"
            | "maroon"
            | "burgundy"
            | "crimson"
            | "hotpink"
            | "salmon"
            | "coral"
            | "brickred"
            | "darkred"
            | "brightred"
            | "lightred"
            | "bloodred"
            | "deepred"
            | "wine"
            | "fuchsia"
            | "neonpink"
            | "palepink"
            | "lightpink"
            | "darkpink"
            | "pinkred"
            | "dustyrose"
            | "brick"
    ) || t.contains("pink")
        || t.ends_with("red") && !t.contains("green") && !t.contains("orange")
    {
        return "red_family";
    }
    if matches!(
        t.as_str(),
        "orange"
            | "brown"
            | "tan"
            | "beige"
            | "copper"
            | "bronze"
            | "rust"
            | "umber"
            | "sepia"
            | "peach"
            | "burntorange"
            | "darkorange"
            | "lightorange"
            | "lightbrown"
            | "darkbrown"
            | "redbrown"
            | "orangebrown"
            | "yellowbrown"
            | "terracotta"
            | "burntsienna"
            | "sand"
            | "taupe"
            | "flesh"
            | "skin"
            | "orangered"
            | "redorange"
    ) || t.contains("brown")
        || t.contains("orange") && !t.contains("yellow")
    {
        return "orange_brown_family";
    }
    if matches!(
        t.as_str(),
        "yellow"
            | "gold"
            | "mustard"
            | "khaki"
            | "ochre"
            | "goldenrod"
            | "paleyellow"
            | "lightyellow"
            | "darkyellow"
            | "mustardyellow"
            | "brightyellow"
            | "yelloworange"
            | "cream"
    ) || t.contains("yellow") && !t.contains("green")
    {
        return "yellow_family";
    }
    if matches!(
        t.as_str(),
        "lime" | "limegreen" | "yellowgreen" | "chartreuse" | "greenyellow" | "neongreen"
    ) {
        return "lime_green_family";
    }
    if matches!(
        t.as_str(),
        "olive" | "armygreen" | "sage" | "mossgreen" | "puke" | "khaki"
    ) {
        // olive-like mud sits with green for duplicate warnings vs true green
        return "true_green_family";
    }
    if matches!(
        t.as_str(),
        "green"
            | "darkgreen"
            | "lightgreen"
            | "brightgreen"
            | "forestgreen"
            | "seafoamgreen"
            | "seagreen"
            | "palegreen"
            | "mint"
            | "grassgreen"
            | "kellygreen"
            | "peagreen"
            | "pastelgreen"
            | "leafgreen"
            | "applegreen"
            | "springgreen"
            | "huntergreen"
            | "bluegreen"
            | "greenblue"
    ) || (t.contains("green") && !t.contains("yellow") && !t.contains("lime"))
    {
        return "true_green_family";
    }
    if matches!(
        t.as_str(),
        "cyan"
            | "teal"
            | "turquoise"
            | "aqua"
            | "aquamarine"
            | "seafoam"
            | "darkteal"
            | "lightteal"
            | "darkturquoise"
            | "lightturquoise"
            | "oceanblue"
            | "seablue"
    ) || t.contains("teal")
        || t.contains("cyan")
        || t.contains("turquoise")
    {
        return "cyan_teal_family";
    }
    if matches!(
        t.as_str(),
        "blue"
            | "navy"
            | "indigo"
            | "azure"
            | "navyblue"
            | "darkblue"
            | "lightblue"
            | "skyblue"
            | "royalblue"
            | "brightblue"
            | "babyblue"
            | "paleblue"
            | "deepblue"
            | "periwinkle"
            | "cerulean"
            | "slateblue"
            | "steelblue"
            | "midnightblue"
            | "mediumblue"
            | "electricblue"
            | "cornflowerblue"
            | "robinseggblue"
    ) || t.contains("blue") && !t.contains("green") && !t.contains("purple")
    {
        return "blue_family";
    }
    if matches!(
        t.as_str(),
        "purple"
            | "violet"
            | "lavender"
            | "lilac"
            | "mauve"
            | "plum"
            | "eggplant"
            | "indigo"
            | "lightpurple"
            | "darkpurple"
            | "brightpurple"
            | "royalpurple"
            | "neonpurple"
            | "palepurple"
            | "bluepurple"
            | "purpleblue"
            | "pinkpurple"
            | "blueviolet"
    ) || t.contains("purple")
        || t.contains("violet")
        || t.contains("lavender")
    {
        return "purple_family";
    }
    "unknown_family"
}

#[inline]
pub fn is_earth_tone_term(term: &str) -> bool {
    let t = term.to_ascii_lowercase();
    EARTH_TONE_TERMS.iter().any(|&e| e == t)
        || t.contains("brown")
        || t.contains("olive")
        || matches!(
            t.as_str(),
            "tan" | "beige" | "khaki" | "mustard" | "ochre" | "rust"
        )
}

fn earth_term_mass_from_terms(terms: &[RelatedTerm]) -> f64 {
    terms
        .iter()
        .filter(|t| {
            let name = C3_TERM_STRS.get(t.index).copied().unwrap_or("");
            is_earth_tone_term(name)
        })
        .map(|t| t.score)
        .sum()
}

fn excluded_term_mass_from_terms(terms: &[RelatedTerm]) -> f64 {
    const EXCL: &[&str] = &[
        "grey",
        "white",
        "lightgrey",
        "darkgrey",
        "offwhite",
        "greyblue",
        "greygreen",
        "bluegrey",
        "lightbluegrey",
        "black",
        "silver",
    ];
    terms
        .iter()
        .filter(|t| {
            let name = C3_TERM_STRS.get(t.index).copied().unwrap_or("");
            EXCL.iter().any(|&e| e == name)
        })
        .map(|t| t.score)
        .sum()
}

#[inline]
fn rgb_pair_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    (dr * dr + dg * dg + db * db).sqrt()
}

#[inline]
fn oklab_pair_distance(a: &[f32], b: &[f32]) -> f64 {
    let dl = (a[0] - b[0]) as f64;
    let da = (a[1] - b[1]) as f64;
    let db = (a[2] - b[2]) as f64;
    (dl * dl + da * da + db * db).sqrt()
}

fn oklch_hue_deg(a: f32, b: f32) -> Option<f64> {
    let c = oklab_chroma(a, b);
    if c < 1e-4 {
        return None;
    }
    Some((b as f64).atan2(a as f64).to_degrees().rem_euclid(360.0))
}

fn min_circular_hue_gap_deg(hues: &[f64]) -> f64 {
    if hues.len() < 2 {
        return 360.0;
    }
    let mut sorted = hues.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mut min_gap = f64::INFINITY;
    for w in sorted.windows(2) {
        min_gap = min_gap.min(w[1] - w[0]);
    }
    let wrap = 360.0 - (sorted.last().unwrap() - sorted.first().unwrap());
    min_gap.min(wrap)
}

/// Percentile (0–1) of a sorted ascending slice.
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = ((sorted.len() as f64 - 1.0) * p.clamp(0.0, 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

pub fn min_c3_name_distance(distances: &[f64]) -> f64 {
    distances.iter().copied().fold(f64::INFINITY, f64::min)
}

pub fn p10_c3_name_distance(sorted_asc: &[f64]) -> f64 {
    percentile_sorted(sorted_asc, 0.10)
}

pub fn sorted_c3_name_distances_ascending(pairwise: &[(usize, usize, f64)]) -> Vec<f64> {
    let mut v: Vec<f64> = pairwise.iter().map(|(_, _, d)| *d).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v
}

/// Count unordered pairs that share the same coarse family.
pub fn duplicate_family_pair_count(families: &[String]) -> usize {
    let n = families.len();
    let mut count = 0;
    for i in 0..n {
        for j in (i + 1)..n {
            if families[i] == families[j] && families[i] != "unknown_family" {
                count += 1;
            }
        }
    }
    count
}

/// Compute diagnostics for one OKLab palette. Not on the hot evaluation path.
pub fn compute_diagnostics(
    c3: &c3::C3,
    oklab_flat: &[f32],
    breakdown: &PaletteObjectiveBreakdown,
) -> PaletteCandidateDiagnostics {
    let n = oklab_flat.len() / 3;
    let mut c3_labs = Vec::new();
    let mut display_rgb = Vec::new();
    fill_c3_labs_from_oklab(oklab_flat, &mut c3_labs);
    fill_display_srgb255(oklab_flat, &mut display_rgb);

    let mut samples = Vec::new();
    let mut palette_terms = Vec::new();
    c3.fill_palette_c3(&c3_labs, C3_TERM_LIMIT, &mut samples, &mut palette_terms);

    let pairwise = c3.pairwise_color_name_distances(&samples);
    let pairwise_asc = sorted_c3_name_distances_ascending(&pairwise);
    let mean_c3 = if pairwise.is_empty() {
        0.0
    } else {
        pairwise.iter().map(|(_, _, d)| d).sum::<f64>() / pairwise.len() as f64
    };
    let min_c3 = min_c3_name_distance(&pairwise_asc);
    let p10_c3 = p10_c3_name_distance(&pairwise_asc);
    let worst_name_pair = pairwise
        .iter()
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|&(i, j, _)| (i, j));

    let mut min_rgb = f64::INFINITY;
    let mut worst_rgb = None;
    for i in 0..n {
        for j in (i + 1)..n {
            let d = rgb_pair_distance(display_rgb[i], display_rgb[j]);
            if d < min_rgb {
                min_rgb = d;
                worst_rgb = Some((i, j));
            }
        }
    }
    if !min_rgb.is_finite() {
        min_rgb = 0.0;
    }

    let mut min_oklab = f64::INFINITY;
    for i in 0..n {
        let ai = &oklab_flat[i * 3..i * 3 + 3];
        for j in (i + 1)..n {
            let aj = &oklab_flat[j * 3..j * 3 + 3];
            min_oklab = min_oklab.min(oklab_pair_distance(ai, aj));
        }
    }
    if !min_oklab.is_finite() {
        min_oklab = 0.0;
    }

    let mut projected = Vec::with_capacity(oklab_flat.len());
    project_oklab_through_display(oklab_flat, &mut projected);
    let mut hues = Vec::new();
    for ch in projected.chunks(3) {
        if let Some(h) = oklch_hue_deg(ch[1], ch[2]) {
            hues.push(h);
        }
    }
    let min_hue_gap = min_circular_hue_gap_deg(&hues);

    let mut dominant_names = Vec::with_capacity(n);
    let mut dominant_scores = Vec::with_capacity(n);
    let mut families = Vec::with_capacity(n);
    let mut earth_mass = 0.0f64;
    let mut excl_mass = 0.0f64;
    for terms in &palette_terms {
        if let Some(top) = terms.first() {
            let name = C3_TERM_STRS
                .get(top.index)
                .copied()
                .unwrap_or("unknown")
                .to_string();
            dominant_scores.push(top.score);
            families.push(coarse_name_family(&name).to_string());
            dominant_names.push(name);
        } else {
            dominant_names.push("unknown".to_string());
            dominant_scores.push(0.0);
            families.push("unknown_family".to_string());
        }
        earth_mass += earth_term_mass_from_terms(terms);
        excl_mass += excluded_term_mass_from_terms(terms);
    }

    // Pool-free bad-pair count uses a conservative absolute floor; LexV1 may override with pool Q1.
    let bad_name_pair_count = pairwise.iter().filter(|(_, _, d)| *d < 0.15).count();

    PaletteCandidateDiagnostics {
        total_loss: breakdown.total,
        mean_c3_name_distance: mean_c3,
        min_c3_name_distance: if min_c3.is_finite() { min_c3 } else { 0.0 },
        p10_c3_name_distance: p10_c3,
        worst_name_pair,
        min_display_rgb_distance: if breakdown.min_display_rgb_distance > 0.0 {
            breakdown.min_display_rgb_distance
        } else {
            min_rgb as f32
        },
        worst_rgb_pair: worst_rgb,
        min_oklab_distance: min_oklab,
        min_oklch_hue_gap_deg: min_hue_gap,
        dominant_c3_names: dominant_names,
        dominant_c3_scores: dominant_scores,
        coarse_name_families: families.clone(),
        duplicate_family_pair_count: duplicate_family_pair_count(&families),
        bad_name_pair_count,
        earth_term_mass: earth_mass,
        excluded_term_mass: excl_mass,
        min_srgb_saturation: breakdown.min_srgb_saturation,
        min_oklab_chroma: breakdown.min_oklab_chroma,
        pairwise_c3_distances_asc: pairwise_asc,
    }
}

fn earth_term_bucket(mass: f64) -> usize {
    if mass < 0.15 {
        0
    } else if mass < 0.30 {
        1
    } else if mass < 0.50 {
        2
    } else {
        3
    }
}

fn rgb_deficit_bucket(min_rgb: f32) -> usize {
    let thr = MIN_DISPLAY_RGB_DISTANCE as f32;
    if min_rgb >= thr {
        0
    } else if min_rgb >= thr * 0.9 {
        1
    } else {
        2
    }
}

/// Lower-quartile of per-candidate *minimum* pairwise C3 distances in the pool.
pub fn pool_bad_name_threshold(pool: &[RestartRecord]) -> f64 {
    let mut mins: Vec<f64> = pool
        .iter()
        .map(|r| r.diagnostics.min_c3_name_distance)
        .collect();
    if mins.is_empty() {
        return 0.15;
    }
    mins.sort_by(|a, b| a.total_cmp(b));
    let q1 = percentile_sorted(&mins, 0.25);
    // Keep a sane floor so the threshold is not degenerate on tiny pools.
    q1.max(0.05).min(0.35)
}

fn bad_name_pair_count_with_threshold(diag: &PaletteCandidateDiagnostics, thr: f64) -> usize {
    diag.pairwise_c3_distances_asc
        .iter()
        .filter(|&&d| d < thr)
        .count()
}

fn cmp_f64_asc(a: f64, b: f64) -> Ordering {
    a.total_cmp(&b)
}

fn cmp_f64_desc(a: f64, b: f64) -> Ordering {
    b.total_cmp(&a)
}

/// Select best restart index for `mode`. Deterministic tie-breaks favor lower restart_id.
pub fn select_best_restart(pool: &[RestartRecord], mode: PaletteSelectionMode) -> Option<usize> {
    if pool.is_empty() {
        return None;
    }
    let thr = pool_bad_name_threshold(pool);
    let mut best_i = 0usize;
    for i in 1..pool.len() {
        if cmp_candidates(pool, best_i, i, mode, thr) == Ordering::Greater {
            best_i = i;
        }
    }
    Some(best_i)
}

fn cmp_candidates(
    pool: &[RestartRecord],
    a: usize,
    b: usize,
    mode: PaletteSelectionMode,
    thr: f64,
) -> Ordering {
    let da = &pool[a].diagnostics;
    let db = &pool[b].diagnostics;
    let ord = match mode {
        PaletteSelectionMode::TotalLoss => cmp_f64_asc(da.total_loss as f64, db.total_loss as f64),
        PaletteSelectionMode::LexV1 => da
            .duplicate_family_pair_count
            .cmp(&db.duplicate_family_pair_count)
            .then_with(|| {
                bad_name_pair_count_with_threshold(da, thr)
                    .cmp(&bad_name_pair_count_with_threshold(db, thr))
            })
            .then_with(|| {
                earth_term_bucket(da.earth_term_mass).cmp(&earth_term_bucket(db.earth_term_mass))
            })
            .then_with(|| {
                rgb_deficit_bucket(da.min_display_rgb_distance)
                    .cmp(&rgb_deficit_bucket(db.min_display_rgb_distance))
            })
            .then_with(|| cmp_f64_desc(da.min_c3_name_distance, db.min_c3_name_distance))
            .then_with(|| cmp_f64_desc(da.p10_c3_name_distance, db.p10_c3_name_distance))
            .then_with(|| {
                cmp_f64_desc(
                    da.min_display_rgb_distance as f64,
                    db.min_display_rgb_distance as f64,
                )
            })
            .then_with(|| cmp_f64_asc(da.total_loss as f64, db.total_loss as f64)),
        PaletteSelectionMode::LexV2 => {
            da.duplicate_family_pair_count
                .cmp(&db.duplicate_family_pair_count)
                .then_with(|| {
                    // Lexicographic on sorted pairwise distances: prefer larger distances at worst pairs.
                    let na = da.pairwise_c3_distances_asc.len();
                    let nb = db.pairwise_c3_distances_asc.len();
                    let n = na.max(nb);
                    let mut o = Ordering::Equal;
                    for k in 0..n {
                        let va = da.pairwise_c3_distances_asc.get(k).copied().unwrap_or(0.0);
                        let vb = db.pairwise_c3_distances_asc.get(k).copied().unwrap_or(0.0);
                        o = cmp_f64_desc(va, vb);
                        if o != Ordering::Equal {
                            break;
                        }
                    }
                    o
                })
                .then_with(|| cmp_f64_asc(da.earth_term_mass, db.earth_term_mass))
                .then_with(|| {
                    cmp_f64_desc(
                        da.min_display_rgb_distance as f64,
                        db.min_display_rgb_distance as f64,
                    )
                })
                .then_with(|| cmp_f64_asc(da.total_loss as f64, db.total_loss as f64))
        }
        PaletteSelectionMode::NameTail => da
            .duplicate_family_pair_count
            .cmp(&db.duplicate_family_pair_count)
            .then_with(|| cmp_f64_desc(da.min_c3_name_distance, db.min_c3_name_distance))
            .then_with(|| cmp_f64_desc(da.p10_c3_name_distance, db.p10_c3_name_distance))
            .then_with(|| {
                cmp_f64_desc(
                    da.min_display_rgb_distance as f64,
                    db.min_display_rgb_distance as f64,
                )
            })
            .then_with(|| cmp_f64_asc(da.total_loss as f64, db.total_loss as f64)),
    };
    ord.then_with(|| pool[a].restart_id.cmp(&pool[b].restart_id))
}

// ---------------------------------------------------------------------------
// Glasbey-style farthest-first seeding
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SeedCandidate {
    pub oklab: [f32; 3],
    pub display_rgb: [f32; 3],
    pub oklch: [f32; 3], // L, C, h_rad
    pub dominant_c3_name: String,
    pub dominant_c3_score: f64,
    pub color_index: usize,
    pub coarse_family: String,
    pub earth_term_mass: f64,
    pub srgb_saturation: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct GlasbeyDistanceWeights {
    pub w_oklab: f64,
    pub w_name: f64,
    pub w_rgb: f64,
    pub w_family: f64,
    pub w_earth: f64,
}

impl Default for GlasbeyDistanceWeights {
    fn default() -> Self {
        Self {
            w_oklab: 1.0,
            w_name: 1.5,
            w_rgb: 0.75,
            w_family: 0.5,
            w_earth: 0.5,
        }
    }
}

fn oklab_to_oklch(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    let c = oklab_chroma(a, b);
    let h = b.atan2(a);
    (l, c, h)
}

fn oklch_to_oklab(l: f32, c: f32, h: f32) -> (f32, f32, f32) {
    (l, c * h.cos(), c * h.sin())
}

fn in_gamut_display_rgb(l: f32, a: f32, b: f32) -> Option<[f32; 3]> {
    let okl = Oklab::new(l, a, b);
    let rgb: Srgb = Srgb::from_color(okl);
    // Reject severe out-of-gamut (palette crate clips; treat near-clip as OK if sat/chroma pass).
    if !rgb.red.is_finite() || !rgb.green.is_finite() || !rgb.blue.is_finite() {
        return None;
    }
    let r = rgb.red.clamp(0.0, 1.0);
    let g = rgb.green.clamp(0.0, 1.0);
    let b_ = rgb.blue.clamp(0.0, 1.0);
    // Round-trip error: if clipped hard, chroma collapses — sat check catches mush.
    Some([r, g, b_])
}

/// Generate in-gamut, saturated OKLCh candidates on a coarse L/C/h grid.
pub fn generate_feasible_seed_candidates(
    c3: &c3::C3,
    luminance_values: &[f32],
) -> Vec<SeedCandidate> {
    let l_lo = luminance_values.get(0).copied().unwrap_or(0.50);
    let l_hi = luminance_values.get(1).copied().unwrap_or(0.92);
    let l_grid = [0.48f32, 0.56, 0.64, 0.72];
    let c_grid = [0.16f32, 0.20, 0.24, 0.28, 0.32];
    let hue_step_deg = 10.0f32;
    let mut out = Vec::with_capacity(4 * 5 * (360.0 / hue_step_deg) as usize);

    for &l0 in &l_grid {
        let l = l0.clamp(l_lo, l_hi);
        for &chroma in &c_grid {
            if chroma < DEFAULT_MIN_OKLAB_CHROMA {
                continue;
            }
            let mut deg = 0.0f32;
            while deg < 360.0 {
                let h = deg.to_radians();
                let (ol, oa, ob) = oklch_to_oklab(l, chroma, h);
                let Some(rgb) = in_gamut_display_rgb(ol, oa, ob) else {
                    deg += hue_step_deg;
                    continue;
                };
                let sat = channel_srgb_saturation(ol, oa, ob);
                if sat < MIN_SRGB_SATURATION {
                    deg += hue_step_deg;
                    continue;
                }
                let lab: Lab = Lab::from_color(Oklab::new(ol, oa, ob));
                let (sample, terms) = c3.related_terms_for_lab(
                    [lab.l as f64, lab.a as f64, lab.b as f64],
                    C3_TERM_LIMIT,
                );
                let (dom_name, dom_score) = terms
                    .first()
                    .map(|t| {
                        (
                            C3_TERM_STRS.get(t.index).copied().unwrap_or("unknown"),
                            t.score,
                        )
                    })
                    .unwrap_or(("unknown", 0.0));
                if matches!(
                    dom_name,
                    "grey" | "white" | "black" | "lightgrey" | "darkgrey" | "offwhite" | "silver"
                ) {
                    deg += hue_step_deg;
                    continue;
                }
                let earth = earth_term_mass_from_terms(&terms);
                let (cl, cc, ch) = oklab_to_oklch(ol, oa, ob);
                out.push(SeedCandidate {
                    oklab: [ol, oa, ob],
                    display_rgb: rgb,
                    oklch: [cl, cc, ch],
                    dominant_c3_name: dom_name.to_string(),
                    dominant_c3_score: dom_score,
                    color_index: sample.c,
                    coarse_family: coarse_name_family(dom_name).to_string(),
                    earth_term_mass: earth,
                    srgb_saturation: sat,
                });
                deg += hue_step_deg;
            }
        }
    }
    out
}

fn composite_seed_distance(
    c3: &c3::C3,
    a: &SeedCandidate,
    b: &SeedCandidate,
    w: &GlasbeyDistanceWeights,
) -> f64 {
    let d_oklab = oklab_pair_distance(&a.oklab, &b.oklab) / 0.5; // ~normalize
    let d_name = 1.0 - c3.color_cosine(a.color_index, b.color_index);
    let d_rgb = {
        let ar = [
            (a.display_rgb[0] * 255.0) as f64,
            (a.display_rgb[1] * 255.0) as f64,
            (a.display_rgb[2] * 255.0) as f64,
        ];
        let br = [
            (b.display_rgb[0] * 255.0) as f64,
            (b.display_rgb[1] * 255.0) as f64,
            (b.display_rgb[2] * 255.0) as f64,
        ];
        rgb_pair_distance(ar, br) / 255.0
    };
    let family_bonus = if a.coarse_family != b.coarse_family {
        1.0
    } else {
        0.0
    };
    let earth_pen = (a.earth_term_mass + b.earth_term_mass) * 0.5;
    w.w_oklab * d_oklab + w.w_name * d_name + w.w_rgb * d_rgb + w.w_family * family_bonus
        - w.w_earth * earth_pen
}

/// Farthest-first categorical seed palette in OKLab.
pub fn glasbey_like_seed_palette(
    n: usize,
    candidates: &[SeedCandidate],
    c3: &c3::C3,
    weights: &GlasbeyDistanceWeights,
) -> Vec<f32> {
    assert!(n >= 1);
    if candidates.is_empty() {
        // Fallback: RGB-primary-ish OKLab
        let mut out = Vec::with_capacity(n * 3);
        for i in 0..n {
            let angle = std::f32::consts::TAU * (i as f32) / (n as f32);
            let (l, a, b) = oklch_to_oklab(0.58, 0.24, angle);
            out.extend_from_slice(&[l, a, b]);
        }
        return out;
    }

    // First pick: maximize mean distance to all others among non-earth, high-score candidates.
    let mut best_first = 0usize;
    let mut best_score = f64::NEG_INFINITY;
    for (i, c) in candidates.iter().enumerate() {
        if c.earth_term_mass > 0.25 {
            continue;
        }
        let mut mean = 0.0;
        for (j, d) in candidates.iter().enumerate() {
            if i == j {
                continue;
            }
            mean += composite_seed_distance(c3, c, d, weights);
        }
        mean /= (candidates.len() - 1).max(1) as f64;
        let score = mean + c.dominant_c3_score + c.srgb_saturation as f64;
        if score > best_score {
            best_score = score;
            best_first = i;
        }
    }
    if !best_score.is_finite() {
        // All candidates were earthy — pick max saturation.
        best_first = candidates
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.srgb_saturation
                    .partial_cmp(&b.srgb_saturation)
                    .unwrap_or(Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
    }

    let mut selected = vec![best_first];
    while selected.len() < n {
        let mut best_j = None;
        let mut best_min_d = f64::NEG_INFINITY;
        let mut best_tie = (f64::INFINITY, 0.0f32, 0.0f64); // earth, sat, score
        for (j, cand) in candidates.iter().enumerate() {
            if selected.contains(&j) {
                continue;
            }
            let min_d = selected
                .iter()
                .map(|&i| composite_seed_distance(c3, &candidates[i], cand, weights))
                .fold(f64::INFINITY, f64::min);
            let tie = (
                cand.earth_term_mass,
                -cand.srgb_saturation,
                -cand.dominant_c3_score,
            );
            let better = min_d > best_min_d + 1e-12
                || ((min_d - best_min_d).abs() <= 1e-12
                    && (tie.0 < best_tie.0
                        || (tie.0 == best_tie.0
                            && (tie.1 < best_tie.1
                                || (tie.1 == best_tie.1 && tie.2 < best_tie.2)))));
            if better {
                best_min_d = min_d;
                best_j = Some(j);
                best_tie = tie;
            }
        }
        if let Some(j) = best_j {
            selected.push(j);
        } else {
            break;
        }
    }

    let mut out = Vec::with_capacity(n * 3);
    for &idx in &selected {
        out.extend_from_slice(&candidates[idx].oklab);
    }
    // If we couldn't fill n, pad with hue-ring around last.
    while out.len() / 3 < n {
        let k = out.len() / 3;
        let angle = std::f32::consts::TAU * (k as f32) / (n as f32) + 0.3;
        let (l, a, b) = oklch_to_oklab(0.58, 0.22, angle);
        out.extend_from_slice(&[l, a, b]);
    }
    out
}

/// Init mode for study / experiment pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteInitMode {
    Current,
    GlasbeyV1,
    Mixed,
}

impl PaletteInitMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::GlasbeyV1 => "glasbey_v1",
            Self::Mixed => "mixed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "current" | "default" => Some(Self::Current),
            "glasbey_v1" | "glasbey" | "glasbeyv1" => Some(Self::GlasbeyV1),
            "mixed" => Some(Self::Mixed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_pink_share_family() {
        assert_eq!(coarse_name_family("red"), "red_family");
        assert_eq!(coarse_name_family("pink"), "red_family");
        assert_eq!(
            duplicate_family_pair_count(&[
                "red_family".into(),
                "red_family".into(),
                "blue_family".into()
            ]),
            1
        );
    }

    #[test]
    fn yellow_gold_and_green_olive_families() {
        assert_eq!(coarse_name_family("yellow"), "yellow_family");
        assert_eq!(coarse_name_family("gold"), "yellow_family");
        assert_eq!(coarse_name_family("green"), "true_green_family");
        assert_eq!(coarse_name_family("olive"), "true_green_family");
    }

    #[test]
    fn earth_tone_detects_brown() {
        assert!(is_earth_tone_term("brown"));
        assert!(is_earth_tone_term("olive"));
        assert!(!is_earth_tone_term("cyan"));
    }

    #[test]
    fn lex_total_picks_lower_loss() {
        let a = RestartRecord {
            restart_id: 0,
            oklab: vec![0.5, 0.1, 0.0],
            diagnostics: dummy_diag(-1.0, 0.5, 0),
        };
        let b = RestartRecord {
            restart_id: 1,
            oklab: vec![0.5, -0.1, 0.0],
            diagnostics: dummy_diag(-3.0, 0.2, 0),
        };
        let pool = vec![a, b];
        assert_eq!(
            select_best_restart(&pool, PaletteSelectionMode::TotalLoss),
            Some(1)
        );
    }

    #[test]
    fn lex_v1_prefers_no_duplicate_family() {
        let mut good = dummy_diag(-2.0, 0.4, 0);
        good.duplicate_family_pair_count = 0;
        good.min_c3_name_distance = 0.3;
        let mut bad = dummy_diag(-4.0, 0.5, 1);
        bad.duplicate_family_pair_count = 1;
        bad.min_c3_name_distance = 0.05;
        let pool = vec![
            RestartRecord {
                restart_id: 0,
                oklab: vec![],
                diagnostics: bad,
            },
            RestartRecord {
                restart_id: 1,
                oklab: vec![],
                diagnostics: good,
            },
        ];
        assert_eq!(
            select_best_restart(&pool, PaletteSelectionMode::LexV1),
            Some(1)
        );
    }

    fn dummy_diag(total: f32, min_c3: f64, dup: usize) -> PaletteCandidateDiagnostics {
        PaletteCandidateDiagnostics {
            total_loss: total,
            mean_c3_name_distance: min_c3 + 0.1,
            min_c3_name_distance: min_c3,
            p10_c3_name_distance: min_c3,
            worst_name_pair: None,
            min_display_rgb_distance: 200.0,
            worst_rgb_pair: None,
            min_oklab_distance: 0.2,
            min_oklch_hue_gap_deg: 40.0,
            dominant_c3_names: vec![],
            dominant_c3_scores: vec![],
            coarse_name_families: vec![],
            duplicate_family_pair_count: dup,
            bad_name_pair_count: 0,
            earth_term_mass: 0.0,
            excluded_term_mass: 0.0,
            min_srgb_saturation: 0.8,
            min_oklab_chroma: 0.2,
            pairwise_c3_distances_asc: vec![min_c3, min_c3 + 0.1, min_c3 + 0.2],
        }
    }

    #[test]
    fn seed_grid_respects_floors() {
        let c3 = c3::C3::new();
        let cands = generate_feasible_seed_candidates(&c3, &[0.50, 0.92]);
        assert!(!cands.is_empty());
        for c in &cands {
            assert!(c.oklch[1] + 1e-4 >= DEFAULT_MIN_OKLAB_CHROMA);
            assert!(c.srgb_saturation + 1e-4 >= MIN_SRGB_SATURATION);
        }
        let seed = glasbey_like_seed_palette(6, &cands, &c3, &GlasbeyDistanceWeights::default());
        assert_eq!(seed.len(), 18);
    }
}
