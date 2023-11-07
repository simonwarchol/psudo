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

#[wasm_bindgen]
pub fn ln(array: &[u16]) -> Vec<f32> {
    let array_vec = array.to_vec();
    let vals = array_vec
        .par_iter()
        .map(|&x| x as f32)
        .collect::<Vec<f32>>();
    // take a random sample of 1000 values
    // Iterate over vals, if value is 0 or nan, make it 0, otherwise take the log

    let vals_log = vals
        .iter()
        .map(|&x| {
            if x <= 0.0 || x.is_nan() { 0.0 } else { x.ln() }
        })
        .collect::<Array1<f32>>();
    return vals_log.to_vec();
}

#[wasm_bindgen]
pub fn channel_gmm(array: &[u16]) -> Vec<f32> {
    console::log_1(&"Starting GMM".into());
    let sampled_array = if array.len() > 20_000 {
        let mut rng = rand::thread_rng();
        array.choose_multiple(&mut rng, 20_000).cloned().collect::<Vec<_>>()
    } else {
        array.to_vec()
    };

    let vals = sampled_array
        .par_iter()
        .map(|&x| x as f32)
        .collect::<Vec<f32>>();
    // take a random sample of 1000 values
    // Iterate over vals, if value is 0 or nan, make it 0, otherwise take the log

    let vals_log = vals
        .iter()
        .map(|&x| {
            if x <= 0.0 || x.is_nan() { 0.0 } else { x.ln() }
        })
        .collect::<Array1<f32>>();

    // let min_log_val = vals_log.min().unwrap();
    // let max_log_val = vals_log.max().unwrap();
    // console::log_1(
    //     &format!("Log-transformed data - Min: {}, Max: {}", min_log_val, max_log_val).into()
    // );
    // let mut print_str = String::new();
    // for val in vals_log.iter() {
    //     print_str += &val.to_string();
    //     print_str.push(',');
    // }
    // console::log_1(&JsValue::from_str(&print_str));

    let dataset = Dataset::from(vals_log.insert_axis(Axis(1)));

    console::log_1(&"Created Dataset!".into());
    let gmm_result = GaussianMixtureModel::params(3)
        .n_runs(10)
        .tolerance(1e-4)
        .max_n_iterations(500)
        .init_method(linfa_clustering::GmmInitMethod::Random)
        .fit(&dataset);

    let gmm = match gmm_result {
        Ok(g) => g,
        Err(e) => {
            let error_message = format!("GMM fitting error: {:?}", e);
            console::log_1(&error_message.into());
            panic!("GMM fitting failed!");
        }
    };
    let means = gmm.means();
    let covariances = gmm.covariances();
    let weights = gmm.weights();

    let flattend_means = means.view().into_shape((means.len(),)).unwrap().to_owned().into_raw_vec();
    let mut indexed_values: Vec<(usize, f32)> = flattend_means
        .iter()
        .enumerate()
        .map(|(i, &val)| (i, val))
        .collect();
    indexed_values.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    // Extract indices from the sorted pairs.
    let (_i0, i1, i2) = (indexed_values[0].0, indexed_values[1].0, indexed_values[2].0);
    let (mean1, mean2) = (flattend_means[i1], flattend_means[i2]);
    let (std1, std2) = (covariances[[i1, 0, 0]].sqrt(), covariances[[i2, 0, 0]].sqrt());
    // Python code to implement
    let x = Array1::linspace(mean1, mean2, 50);
    let norm1 = Normal::new(mean1 as f64, std1 as f64).unwrap();
    let norm2 = Normal::new(mean2 as f64, std2 as f64).unwrap();
    let y1 = x.mapv(|v| norm1.pdf(v as f64) * (weights[i1] as f64));
    let y2 = x.mapv(|v| norm2.pdf(v as f64) * (weights[i2] as f64));
    let lmax = mean2 + 2.0 * std2;
    // Calculate the differences between y1 and y2, take their absolute values, and get the index of the minimum value
    let differences = (&y1 - &y2).mapv(|val| val.abs());
    let mut min_diff_index: usize = 0;
    let mut min_diff_value = f64::MAX;
    for (i, &diff) in differences.iter().enumerate() {
        if diff < min_diff_value {
            min_diff_index = i;
            min_diff_value = diff;
        }
    }
    let mut lmin = x[min_diff_index];
    // Apply the given condition
    if lmin >= mean2 {
        lmin = mean2 - 2.0 * std2;
    }
    let vals_array = Array1::from(vals);

    let vmin = f32::max(lmin.exp(), f32::max(*vals_array.min().unwrap(), 0.0));
    let vmax = f32::min(lmax.exp(), *vals_array.max().unwrap());
    return vec![vmin, vmax];
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

fn color_conversion_test() -> () {
    let my_rgb = Srgb::new(0.1, 0.0, 0.0);

    let mut my_okl = Oklab::from_color(my_rgb);
    println!("my_okl: {:?}", my_okl.l);
}

#[wasm_bindgen]
pub fn test_color_distance(array: &[u16]) -> f32 {
    // Run this 100 times and take the average
    let mut total_distance = 0.0;
    for _ in 0..100 {
        total_distance += evaluate_palette(array);
    }
    total_distance / 100.0
}

// ///////////////////////////////////////// Optimization /////////////////////////////////////
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
        // let init_param = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        // let intensities = vec![1.0, 2.0, 3.0];
        // let future_result = async {
        //     vec![1.0]
        // };
        // // Console log
        // let result: Vec<f32> = futures::executor::block_on(future_result);

        // let future_result = diff_penalty(include_str!("diff.wgsl"), &intensities, init_param.clone());

        // let future_result = diff_penalty(include_str!("diff.wgsl"), &self.intensities, param.clone());
        // let result: Vec<f32> = futures::executor::block_on(future_result);
        // let result: Vec<f32> = future_result.block_on();
        // let result_sum: f32 = result.par_iter().sum();
        // let mean: f32 = result_sum / result.len() as f32;
        // Ok(0.1)
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
        .configure(|state| { state.param(intensities.to_vec()).max_iters(10_000) })
        .run()?;
    // Print result
    let best_param = res.state().get_best_param().unwrap().clone();

    Ok(best_param.clone()) // Return the best parameters
}

#[wasm_bindgen]
pub fn optimize(colors: &[u16]) -> Vec<f32> {
    utils::set_panic_hook();

    let float_color_map: Vec<f32> = colors
        .iter()
        .map(|&x| (x as f32) / 255.0)
        .collect::<Vec<f32>>();
    let optimized_colors = annealing(&float_color_map).unwrap();
    optimized_colors
}

// #[wasm_bindgen]
// pub fn test_optimize() -> Vec<f32> {
//     let array = [
//         127, 201, 127, 190, 174, 212, 253, 192, 134, 255, 255, 153, 56, 108, 176, 240, 2, 127,
//     ];
//     let array_vec = array.to_vec();
//     let vals = array_vec
//         .par_iter()
//         .map(|&x| x as f32)
//         .collect::<Vec<f32>>();
//     // take a random sample of 1000 values
//     // Iterate over vals, if value is 0 or nan, make it 0, otherwise take the log

//     let vals_log = vals
//         .iter()
//         .map(|&x| {
//             if x <= 0.0 || x.is_nan() { 0.0 } else { x.ln() }
//         })
//         .collect::<Array1<f32>>();
//     return vals_log.to_vec();
// }
