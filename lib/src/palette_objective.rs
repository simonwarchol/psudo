//! Objective-mode knobs for study A/B. Production stays [`PaletteObjectiveMode::MeanOnly`].

use std::cell::Cell;

/// Weight on `−min` pairwise C3 name distance when [`PaletteObjectiveMode::MinName`] is active.
pub const MIN_NAME_DISTANCE_WEIGHT: f32 = 1.0;

/// Target minimum OKLab Euclidean distance between channels ([`PaletteObjectiveMode::OklabSep`]).
/// Calibrated so red+pink (~0.09) is below the floor and red+blue (~0.35) is above it.
pub const MIN_OKLAB_DISTANCE: f64 = 0.20;

/// Scale for OKLab separation reward/penalty (analogous to [`crate::PERCEPTUAL_SCALE`] = 255 for sRGB).
/// Chosen so typical well-separated palettes (min Δ ≈ 0.35–0.50) keep term magnitude near today’s
/// `−min_rgb/255` (~−0.7…−1.0).
pub const OKLAB_PERCEPTUAL_SCALE: f64 = 0.50;

thread_local! {
    static OBJECTIVE_MODE: Cell<PaletteObjectiveMode> =
        const { Cell::new(PaletteObjectiveMode::MeanOnly) };
}

/// Which objective terms enter `L_tot`. Production uses [`MeanOnly`](Self::MeanOnly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum PaletteObjectiveMode {
    /// Mean C3 name + display-sRGB separation (current production).
    #[default]
    MeanOnly,
    /// Mean + `−w·min` pairwise C3 name distance (`w` = [`MIN_NAME_DISTANCE_WEIGHT`]).
    MinName,
    /// Mean C3 name + OKLab Euclidean separation (instead of display-sRGB).
    OklabSep,
}

impl PaletteObjectiveMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::MeanOnly => "total",
            Self::MinName => "min_name",
            Self::OklabSep => "oklab_sep",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::MeanOnly => "Mean C3 + sRGB sep (production)",
            Self::MinName => "Mean + min C3 name",
            Self::OklabSep => "Mean C3 + OKLab Euclidean sep",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "total" | "mean" | "mean_only" | "baseline" | "production" => Some(Self::MeanOnly),
            "min_name" | "minname" => Some(Self::MinName),
            "oklab_sep" | "oklabsep" | "oklab" => Some(Self::OklabSep),
            _ => None,
        }
    }

    pub fn min_name_weight(self) -> f32 {
        match self {
            Self::MinName => MIN_NAME_DISTANCE_WEIGHT,
            Self::MeanOnly | Self::OklabSep => 0.0,
        }
    }

    pub fn uses_oklab_separation(self) -> bool {
        matches!(self, Self::OklabSep)
    }
}

pub fn current_objective_mode() -> PaletteObjectiveMode {
    OBJECTIVE_MODE.with(|c| c.get())
}

pub fn current_min_name_weight() -> f32 {
    current_objective_mode().min_name_weight()
}

/// Run `f` with the given objective mode on this thread (Rayon workers must wrap too).
pub fn with_objective_mode<R>(mode: PaletteObjectiveMode, f: impl FnOnce() -> R) -> R {
    OBJECTIVE_MODE.with(|c| {
        let prev = c.replace(mode);
        let out = f();
        c.set(prev);
        out
    })
}

/// Convenience for tests that only flip the min-name weight (MeanOnly vs MinName).
pub fn with_min_name_weight<R>(weight: f32, f: impl FnOnce() -> R) -> R {
    let mode = if weight > 0.0 {
        PaletteObjectiveMode::MinName
    } else {
        PaletteObjectiveMode::MeanOnly
    };
    with_objective_mode(mode, f)
}
