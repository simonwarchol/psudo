mod utils;
use ndarray::{ Array1, Array2, s };
use web_sys::console;
use ndarray::Axis;
use rayon::{ prelude::*, vec };
use linfa::prelude::*;
use linfa_clustering::{ GaussianMixtureModel };
use linfa::Dataset;
use statrs::distribution::{ Continuous, Normal };
use ndarray_stats::QuantileExt; // <-- Add this line
use rand::seq::SliceRandom;
use palette::{ FromColor, Oklab, Srgb, Lab };
use std::sync::{ Arc, Mutex };
use rand_xoshiro::Xoshiro256PlusPlus;
use argmin::core::{ CostFunction, Error, Executor, Operator, OptimizationResult, State };
use argmin::solver::simulatedannealing::{ Anneal, SATempFunc };
use argmin::solver::simulatedannealing::SimulatedAnnealing;
use rand::Rng;

use rand::distributions::Uniform;
use rand_xoshiro::rand_core::SeedableRng;
use std::collections::HashMap;

use wasm_bindgen::prelude::*;

mod c3;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, lib!");
}



fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(ai, bi)| (ai - bi).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn average_pairwise_euclidean_distance(matrix: &Array2<f64>) -> f64 {
    let len = matrix.nrows();
    let mut distance_sum = 0.0;
    let mut count = 0;

    for i in 0..len {
        for j in i + 1..len {
            // Bind the temporary ArrayView to variables so they live long enough
            let a_view = matrix.row(i);
            let b_view = matrix.row(j);
            let a_slice = a_view.as_slice().unwrap();
            let b_slice = b_view.as_slice().unwrap();

            distance_sum += euclidean_distance(a_slice, b_slice);
            count += 1;
        }
    }

    if count > 0 {
        distance_sum / (count as f64)
    } else {
        0.0
    }
}

pub fn evaluate_palette(rgb_colors: &[u16]) -> f32 {
    //
    let c3_instance = c3::C3::new();

    let mut cielab_colors: Vec<Vec<f32>> = Vec::new();
    let mut oklab_colors: Vec<Vec<f32>> = Vec::new();
    for color in rgb_colors.chunks(3) {
        let rgb = Srgb::new(
            (color[0] as f32) / 255.0,
            (color[1] as f32) / 255.0,
            (color[2] as f32) / 255.0
        );
        let lab: Lab = Lab::from_color(rgb);
        cielab_colors.push(vec![lab.l, lab.a, lab.b]);
        let oklab: Oklab = Oklab::from_color(rgb);
        oklab_colors.push(vec![oklab.l, oklab.a, oklab.b]);
    }
    let mut lab_palette = Array2::from_shape_vec(
        (rgb_colors.len() / 3, 3),
        cielab_colors
            .iter()
            .flatten()
            .map(|&x| x as f64)
            .collect::<Vec<f64>>()
    ).unwrap();
    let mut oklab_palette = Array2::from_shape_vec(
        (rgb_colors.len() / 3, 3),
        oklab_colors
            .iter()
            .flatten()
            .map(|&x| x as f64)
            .collect::<Vec<f64>>()
    ).unwrap();
    let analyzed_palette: Vec<HashMap<&str, f64>> = c3_instance.analyze_palette(
        lab_palette.clone()
    );
    let cosine_matrix = c3_instance.compute_color_name_distance_matrix(analyzed_palette);
    // Calculate average distance between colors
    let mut total_distance = 0.0;
    let mut total_pairs = 0;
    // Iterate over lower triangle of matrix
    for i in 0..cosine_matrix.shape()[0] {
        for j in 0..i {
            total_distance += cosine_matrix[[i, j]];
            total_pairs += 1;
        }
    }
    let average_cosine_distance = total_distance / (total_pairs as f64);
    let avergae_euc_distance = average_pairwise_euclidean_distance(&lab_palette);
    (average_cosine_distance as f32) + (avergae_euc_distance as f32)
}

struct Loss {
    rng: Arc<Mutex<Xoshiro256PlusPlus>>,
    intensities: Vec<f32>,
    c3_instance: c3::C3,
}

impl Loss {
    pub fn new(intensities: Vec<f32>) -> Self {
        Self {
            rng: Arc::new(Mutex::new(Xoshiro256PlusPlus::from_entropy())),
            intensities,
            c3_instance: c3::C3::new(),
        }
    }
}

impl CostFunction for Loss {
    type Param = Vec<f32>;
    type Output = f32;

    fn cost(&self, param: &Self::Param) -> Result<Self::Output, Error> {
        let mut cielab_colors: Vec<Vec<f32>> = Vec::new();
        let mut oklab_colors: Vec<Vec<f32>> = Vec::new();
        for color in param.chunks(3) {
            let rgb = Srgb::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let lab: Lab = Lab::from_color(rgb);
            cielab_colors.push(vec![lab.l, lab.a, lab.b]);
            let oklab: Oklab = Oklab::from_color(rgb);
            oklab_colors.push(vec![oklab.l, oklab.a, oklab.b]);
        }
        let mut lab_palette = Array2::from_shape_vec(
            (param.len() / 3, 3),
            cielab_colors
                .iter()
                .flatten()
                .map(|&x| x as f64)
                .collect::<Vec<f64>>()
        ).unwrap();
        let mut oklab_palette = Array2::from_shape_vec(
            (param.len() / 3, 3),
            oklab_colors
                .iter()
                .flatten()
                .map(|&x| x as f64)
                .collect::<Vec<f64>>()
        ).unwrap();
        let analyzed_palette: Vec<HashMap<&str, f64>> = self.c3_instance.analyze_palette(
            lab_palette.clone()
        );
        let cosine_matrix = self.c3_instance.compute_color_name_distance_matrix(analyzed_palette);
        // Calculate average distance between colors
        let mut total_distance = 0.0;
        let mut total_pairs = 0;
        // Iterate over lower triangle of matrix
        for i in 0..cosine_matrix.shape()[0] {
            for j in 0..i {
                total_distance += cosine_matrix[[i, j]];
                total_pairs += 1;
            }
        }
        let average_cosine_distance = total_distance / (total_pairs as f64);
        let avergae_euc_distance = average_pairwise_euclidean_distance(&oklab_palette);
        Ok((-average_cosine_distance as f32) + (-avergae_euc_distance as f32))
    }
}

impl Anneal for Loss {
    type Param = Vec<f32>;
    type Output = Vec<f32>;
    type Float = f32;

    /// Anneal a parameter vector
    fn anneal(&self, param: &Vec<f32>, temp: f32) -> Result<Vec<f32>, Error> {
        let mut param_n = param.clone();
        let mut rng = self.rng.lock().unwrap();
        let distr = Uniform::from(0..param.len());
        // Perform modifications to a degree proportional to the current temperature `temp`.
        for _ in 0..(temp.floor() as u64) + 1 {
            // Compute random index of the parameter vector using the supplied random number
            // generator.
            let idx = rng.sample(distr);
            // Compute random number in [0.1, 0.1].
            let val = rng.sample(Uniform::new_inclusive(-0.1, 0.1));
            // modify previous parameter value at random position `idx` by `val`
            param_n[idx] += val;

            // check if bounds are violated. If yes, project onto bound.
            param_n[idx] = param_n[idx].clamp(0.0, 1.0);
        }
        Ok(param_n)
    }
}

fn annealing(intensities: &Vec<f32>) -> Result<Vec<f32>, Error> {
    let solver = SimulatedAnnealing::new(15.0)?;
    let cost_function = Loss::new(intensities.clone());
    // Optional: Define temperature function (defaults to `SATempFunc::TemperatureFast`)
    let res = Executor::new(cost_function, solver)
        .configure(|state| { state.param(intensities.to_vec()).max_iters(1_000) })
        // Optional: Attach an observer
        .run()?;
    // Print result
    let best_param = res.state().get_best_param().unwrap().clone();

    Ok(best_param) // Return the best parameters
}

#[wasm_bindgen]
pub fn optimize() {
    let color_map = &[
        127, 201, 127, 190, 174, 212, 253, 192, 134, 255, 255, 153, 56, 108, 176, 240, 2, 127,
    ];
    // Convert to float, divide each val by 255.0
    let float_color_map: Vec<f32> = color_map
        .iter()
        .map(|&x| (x as f32) / 255.0)
        .collect::<Vec<f32>>();
    println!("color_map: {:?}", float_color_map);
    let best_params = annealing(&float_color_map).unwrap();
    println!("Best parameter: {:?}", best_params);
}
