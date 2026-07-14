//! Argmin solver dispatch for palette OKLab optimization (native benchmarks / tooling).

use crate::{
    annealing, enforce_channel_saturation,
    evaluate_palette_objective_breakdown_with_excluded_set, polish_oklab_palette,
    random_initial_oklab, refine_oklab_palette, sa_initial_oklab_for_restart, Loss,
    PaletteObjectiveBreakdown,
};
use argmin::core::{CostFunction, Error, Executor, Gradient, State};
use argmin::solver::gradientdescent::SteepestDescent;
use argmin::solver::linesearch::{condition::ArmijoCondition, BacktrackingLineSearch};
use argmin::solver::neldermead::NelderMead;
use argmin::solver::particleswarm::ParticleSwarm;
use argmin::solver::quasinewton::LBFGS;
use ndarray::Array2;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::HashSet;
use std::sync::Arc;

use crate::c3;

/// Global search methods comparable under the same iteration budget (see [`scaled_solver_iters`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PaletteArgminSolver {
    SimulatedAnnealing,
    NelderMead,
    ParticleSwarm,
    SteepestDescent,
    LBfgs,
    /// Coordinate polish from the input palette only (no global search).
    PolishOnly,
}

impl PaletteArgminSolver {
    pub const ALL: &'static [PaletteArgminSolver] = &[
        PaletteArgminSolver::SimulatedAnnealing,
        PaletteArgminSolver::NelderMead,
        PaletteArgminSolver::ParticleSwarm,
        PaletteArgminSolver::SteepestDescent,
        PaletteArgminSolver::LBfgs,
        PaletteArgminSolver::PolishOnly,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::SimulatedAnnealing => "sa",
            Self::NelderMead => "nelder_mead",
            Self::ParticleSwarm => "pso",
            Self::SteepestDescent => "steepest_descent",
            Self::LBfgs => "lbfgs",
            Self::PolishOnly => "polish_only",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SimulatedAnnealing => "Simulated annealing",
            Self::NelderMead => "Nelder–Mead",
            Self::ParticleSwarm => "Particle swarm",
            Self::SteepestDescent => "Steepest descent (FD grad)",
            Self::LBfgs => "L-BFGS (FD grad)",
            Self::PolishOnly => "Polish only (no global search)",
        }
    }

    /// Gradient-based solvers use fewer outer iterations (each step is ~O(dim) cost evals).
    pub fn uses_gradient(self) -> bool {
        matches!(
            self,
            Self::SteepestDescent | Self::LBfgs
        )
    }
}

/// Tunable parameters for [`run_palette_argmin_solver`] (benchmarks / hyperparameter sweeps).
#[derive(Clone, Debug, Default)]
pub struct PaletteSolverParams {
    /// If set, use this as argmin `max_iters` instead of [`scaled_solver_iters`].
    pub argmin_max_iters: Option<u32>,
    /// PSO swarm size (default scales with channel count).
    pub pso_num_particles: Option<usize>,
    /// SA initial temperature (native SA path only).
    pub sa_initial_temp: Option<f32>,
    /// Nelder–Mead simplex axis perturbation scale (default 1.0).
    pub nm_perturb_scale: f32,
    /// Skip hue-spread init; use random saturated OKLab (rescue restarts at 6ch).
    pub force_random_nm_init: bool,
    /// L-BFGS history length (default 7).
    pub lbfgs_history: usize,
}

/// Map a nominal SA iteration budget to per-solver `max_iters` for the argmin executor.
pub fn scaled_solver_iters(solver: PaletteArgminSolver, sa_budget: u32) -> u32 {
    let b = sa_budget.max(1);
    if solver.uses_gradient() {
        (b / 8).max(40)
    } else if solver == PaletteArgminSolver::ParticleSwarm {
        (b / 4).max(80)
    } else if solver == PaletteArgminSolver::NelderMead {
        (b / 2).max(100)
    } else {
        b
    }
}

fn clamp_param_component(
    param: &mut [f32],
    idx: usize,
    luminance_values: &[f32]
) {
    let comp = idx % 3;
    if comp == 0 {
        param[idx] = param[idx].clamp(luminance_values[0], luminance_values[1]);
    } else {
        param[idx] = param[idx].clamp(-0.4, 0.4);
    }
}

fn build_oklab_bounds(n_channels: usize, luminance_values: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let mut lb = Vec::with_capacity(n_channels * 3);
    let mut ub = Vec::with_capacity(n_channels * 3);
    for _ in 0..n_channels {
        lb.extend([luminance_values[0], -0.4, -0.4]);
        ub.extend([luminance_values[1], 0.4, 0.4]);
    }
    (lb, ub)
}

fn build_nelder_mead_simplex(
    start: &[f32],
    locked_colors: &[bool],
    luminance_values: &[f32],
    seed: u64,
    perturb_scale: f32
) -> Vec<Vec<f32>> {
    let s = perturb_scale.max(0.25);
    let n = start.len();
    let mut simplex = vec![start.to_vec()];
    let mut rng = StdRng::seed_from_u64(seed);
    for i in 0..n {
        let ch = i / 3;
        let comp = i % 3;
        let mut v = start.to_vec();
        if locked_colors[ch] {
            let delta = match comp {
                0 => 0.02 * s,
                _ => 0.03 * s,
            };
            v[i] += delta;
        } else {
            let delta = match comp {
                0 => 0.05 * s,
                _ => 0.08 * s,
            };
            v[i] += delta;
        }
        clamp_param_component(&mut v, i, luminance_values);
        enforce_channel_saturation(&mut v, ch, &mut rng);
        simplex.push(v);
    }
    simplex
}

type PaletteLineSearch =
    BacktrackingLineSearch<Vec<f32>, Vec<f32>, ArmijoCondition<f32>, f32>;

fn palette_linesearch() -> PaletteLineSearch {
    BacktrackingLineSearch::new(ArmijoCondition::new(1e-4f32).expect("armijo c"))
}

fn make_loss(
    locked_colors: &[bool],
    intensity_arc: Arc<Array2<f32>>,
    luminance_values: &[f32],
    avg_confusion: f32,
    spatial_w: f32,
    excluded_colors_indices: &[f32],
    color_name_indices: &[f32],
    c3_instance: Arc<c3::C3>,
    rng_seed: Option<u64>
) -> Loss {
    Loss::new(
        locked_colors.to_vec(),
        intensity_arc,
        luminance_values.to_vec(),
        avg_confusion,
        spatial_w,
        excluded_colors_indices.to_vec(),
        color_name_indices.to_vec(),
        c3_instance,
        rng_seed,
    )
}

impl Gradient for Loss {
    type Param = Vec<f32>;
    type Gradient = Vec<f32>;

    fn gradient(&self, param: &Self::Param) -> Result<Self::Gradient, Error> {
        let eps = 5e-4f32;
        let f0 = self.cost(param)?;
        let mut g = vec![0.0f32; param.len()];
        for i in 0..param.len() {
            let ch = i / 3;
            if self.locked_colors[ch] {
                continue;
            }
            let mut p_plus = param.clone();
            let mut p_minus = param.clone();
            p_plus[i] += eps;
            p_minus[i] -= eps;
            clamp_param_component(&mut p_plus, i, &self.luminance_values);
            clamp_param_component(&mut p_minus, i, &self.luminance_values);
            let f_plus = self.cost(&p_plus)?;
            let f_minus = self.cost(&p_minus)?;
            g[i] = (f_plus - f_minus) / (2.0 * eps);
        }
        let _ = f0;
        Ok(g)
    }
}

/// Run one global-search pass; caller applies polish/refine.
#[allow(clippy::too_many_arguments)]
pub fn run_palette_argmin_solver(
    solver: PaletteArgminSolver,
    start_oklab: &[f32],
    locked_colors: &[bool],
    intensity_arc: Arc<Array2<f32>>,
    luminance_values: &[f32],
    excluded_colors_indices: &[f32],
    color_name_indices: &[f32],
    c3_instance: Arc<c3::C3>,
    sa_budget_iters: u32,
    init_seed: u64,
    anneal_rng_seed: u64,
    confusion_baseline_samples: u32,
    include_spatial_channel_overlap: bool,
    precomputed_avg_confusion: Option<f32>,
    params: &PaletteSolverParams,
    // Multistart index; restart 0 always uses the RGB-primary spread init.
    restart: u32,
) -> Result<(Vec<f32>, f32), Error> {
    if solver == PaletteArgminSolver::PolishOnly {
        return Ok((start_oklab.to_vec(), f32::NAN));
    }

    let max_iters = params
        .argmin_max_iters
        .unwrap_or_else(|| scaled_solver_iters(solver, sa_budget_iters));
    let n_channels = start_oklab.len() / 3;

    if solver == PaletteArgminSolver::SimulatedAnnealing {
        return annealing(
            start_oklab,
            locked_colors,
            Arc::clone(&intensity_arc),
            luminance_values,
            excluded_colors_indices,
            color_name_indices,
            c3_instance,
            max_iters,
            confusion_baseline_samples,
            Some(init_seed),
            Some(anneal_rng_seed),
            None,
            include_spatial_channel_overlap,
            precomputed_avg_confusion,
            None,
            params.sa_initial_temp,
        );
    }

    let spatial_w = if include_spatial_channel_overlap {
        crate::SPATIAL_CONFUSION_WEIGHT
    } else {
        0.0
    };
    let avg_confusion = precomputed_avg_confusion.unwrap_or(1.0);

    let mut rng = StdRng::seed_from_u64(init_seed);
    let start_param = if params.force_random_nm_init {
        random_initial_oklab(start_oklab, locked_colors, luminance_values, &mut rng)
    } else {
        sa_initial_oklab_for_restart(
            start_oklab,
            locked_colors,
            luminance_values,
            init_seed,
            restart,
            &mut rng,
        )
    };

    let cost_function = make_loss(
        locked_colors,
        Arc::clone(&intensity_arc),
        luminance_values,
        avg_confusion,
        spatial_w,
        excluded_colors_indices,
        color_name_indices,
        c3_instance,
        Some(anneal_rng_seed),
    );

    let max_iters_u64 = max_iters as u64;

    match solver {
        PaletteArgminSolver::NelderMead => {
            let simplex = build_nelder_mead_simplex(
                &start_param,
                locked_colors,
                luminance_values,
                init_seed.wrapping_add(0xBEEF),
                params.nm_perturb_scale,
            );
            let nm = NelderMead::new(simplex);
            let res = Executor::new(cost_function, nm)
                .configure(|state| state.max_iters(max_iters_u64))
                .run()?;
            let best_param = res.state().get_best_param().unwrap().clone();
            let best_cost = res.state().get_best_cost();
            Ok((best_param, best_cost))
        }
        PaletteArgminSolver::ParticleSwarm => {
            let (lb, ub) = build_oklab_bounds(n_channels, luminance_values);
            let particles = params
                .pso_num_particles
                .unwrap_or_else(|| (12 + n_channels * 3).clamp(20, 48));
            let pso = ParticleSwarm::new((lb, ub), particles);
            let res = Executor::new(cost_function, pso)
                .configure(|state| state.max_iters(max_iters_u64))
                .run()?;
            let particle = res.state().get_best_param().unwrap().clone();
            Ok((particle.position, particle.cost))
        }
        PaletteArgminSolver::SteepestDescent => {
            let sd = SteepestDescent::new(palette_linesearch());
            let res = Executor::new(cost_function, sd)
                .configure(|state| state.param(start_param).max_iters(max_iters_u64))
                .run()?;
            let best_param = res.state().get_best_param().unwrap().clone();
            let best_cost = res.state().get_best_cost();
            Ok((best_param, best_cost))
        }
        PaletteArgminSolver::LBfgs => {
            let lbfgs = LBFGS::new(palette_linesearch(), params.lbfgs_history.max(2));
            let res = Executor::new(cost_function, lbfgs)
                .configure(|state| state.param(start_param).max_iters(max_iters_u64))
                .run()?;
            let best_param = res.state().get_best_param().unwrap().clone();
            let best_cost = res.state().get_best_cost();
            Ok((best_param, best_cost))
        }
        PaletteArgminSolver::SimulatedAnnealing | PaletteArgminSolver::PolishOnly => unreachable!(),
    }
}

/// Study-style postprocess: one polish + refine on the best candidate.
#[allow(clippy::too_many_arguments)]
pub fn study_postprocess_oklab(
    oklab: &mut Vec<f32>,
    locked_colors: &[bool],
    luminance_values: &[f32],
    c3: &c3::C3,
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32],
    base_seed: u64
) {
    polish_oklab_palette(
        oklab,
        locked_colors,
        luminance_values,
        c3,
        intensity_arc,
        avg_confusion,
        spatial_w,
        excluded_set,
        color_name_indices,
        false,
    );
    refine_oklab_palette(
        oklab,
        locked_colors,
        luminance_values,
        c3,
        intensity_arc,
        avg_confusion,
        spatial_w,
        excluded_set,
        color_name_indices,
        base_seed.wrapping_add(0xA11CE),
    );
}

/// Recompute breakdown after optimization (for benchmarks).
pub fn objective_total_for_oklab(
    c3: &c3::C3,
    oklab: &[f32],
    intensity_arc: &Arc<Array2<f32>>,
    avg_confusion: f32,
    spatial_w: f32,
    excluded_set: &HashSet<usize>,
    color_name_indices: &[f32]
) -> PaletteObjectiveBreakdown {
    evaluate_palette_objective_breakdown_with_excluded_set(
        c3,
        oklab,
        intensity_arc,
        avg_confusion,
        spatial_w,
        excluded_set,
        color_name_indices,
    )
}
