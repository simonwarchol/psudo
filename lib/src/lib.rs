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
    // console::log_1(&"Starting GMM".into());
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

    let dataset = Dataset::from(vals_log.insert_axis(Axis(1)));

    // console::log_1(&"Created Dataset!".into());
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

// pub fn evaluate_palette(rgb_colors: &[u16]) -> f32 {
//     //
//     let c3_instance = c3::C3::new();

//     let mut cielab_colors: Vec<Vec<f32>> = Vec::new();
//     let mut oklab_colors: Vec<Vec<f32>> = Vec::new();
//     for color in rgb_colors.chunks(3) {
//         let rgb = Srgb::new(
//             (color[0] as f32) / 255.0,
//             (color[1] as f32) / 255.0,
//             (color[2] as f32) / 255.0
//         );
//         let lab: Lab = Lab::from_color(rgb);
//         cielab_colors.push(vec![lab.l, lab.a, lab.b]);
//         let oklab: Oklab = Oklab::from_color(rgb);
//         oklab_colors.push(vec![oklab.l, oklab.a, oklab.b]);
//     }
//     let mut lab_palette = Array2::from_shape_vec(
//         (rgb_colors.len() / 3, 3),
//         cielab_colors
//             .iter()
//             .flatten()
//             .map(|&x| x as f64)
//             .collect::<Vec<f64>>()
//     ).unwrap();
//     let mut oklab_palette = Array2::from_shape_vec(
//         (rgb_colors.len() / 3, 3),
//         oklab_colors
//             .iter()
//             .flatten()
//             .map(|&x| x as f64)
//             .collect::<Vec<f64>>()
//     ).unwrap();
//     let analyzed_palette: Vec<HashMap<&str, f64>> = c3_instance.analyze_palette(
//         lab_palette.clone()
//     );
//     let cosine_matrix = c3_instance.compute_color_name_distance_matrix(analyzed_palette);
//     // Calculate average distance between colors
//     let mut total_distance = 0.0;
//     let mut total_pairs = 0;
//     // Iterate over lower triangle of matrix
//     for i in 0..cosine_matrix.shape()[0] {
//         for j in 0..i {
//             total_distance += cosine_matrix[[i, j]];
//             total_pairs += 1;
//         }
//     }
//     let average_cosine_distance = total_distance / (total_pairs as f64);
//     let avergae_euc_distance = average_pairwise_euclidean_distance(&lab_palette);
//     (average_cosine_distance as f32) + (avergae_euc_distance as f32)
// }

fn color_conversion_test() -> () {
    let my_rgb = Srgb::new(0.1, 0.0, 0.0);

    let mut my_okl = Oklab::from_color(my_rgb);
    println!("my_okl: {:?}", my_okl.l);
}

// ///////////////////////////////////////// Optimization /////////////////////////////////////
struct Loss {
    rng: Arc<Mutex<Xoshiro256PlusPlus>>,
    c3_instance: c3::C3,
    locked_colors: Vec<bool>,
}

impl Loss {
    pub fn new(locked_colors: Vec<bool>) -> Self {
        Self {
            rng: Arc::new(Mutex::new(Xoshiro256PlusPlus::from_entropy())),
            locked_colors: locked_colors,
            c3_instance: c3::C3::new(),
        }
    }
}

impl CostFunction for Loss {
    type Param = Vec<f32>;
    type Output = f32;

    fn cost(&self, param: &Self::Param) -> Result<Self::Output, Error> {
        let mut cielab_colors: Vec<Vec<f32>> = Vec::new();
        let oklab_colors = param;
        for color in param.chunks(3) {
            let okl = Oklab::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let lab: Lab = Lab::from_color(okl);
            cielab_colors.push(vec![lab.l, lab.a, lab.b]);
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

        // Iterate over each color (assuming each color is represented by 3 values in the vector)
        for color_idx in 0..param.len() / 3 {
            if self.locked_colors[color_idx] {
                continue; // Skip this color if it is locked
            }

            // Modify each component of the color
            for i in 0..3 {
                let idx = color_idx * 3 + i; // Calculate the index in the parameter vector
                let val = rng.sample(Uniform::new_inclusive(-0.1, 0.1));
                param_n[idx] += val;
                // Scale luminance between 0.5 and 1
                if i == 0 {
                    param_n[idx] = (param_n[idx] / 2.0 + 0.5).clamp(0.5, 1.0);
                } else {
                    // Clamp the value to ensure it stays within bounds
                    param_n[idx] = param_n[idx].clamp(-0.4, 0.4);
                }
            }
        }

        Ok(param_n)
    }
}
fn annealing(colors: &Vec<f32>, locked_colors: &Vec<bool>) -> Result<Vec<f32>, Error> {
    let solver = SimulatedAnnealing::new(15.0)?;
    let cost_function = Loss::new(locked_colors.clone());
    // Optional: Define temperature function (defaults to `SATempFunc::TemperatureFast`)
    let res = Executor::new(cost_function, solver)
        .configure(|state| { state.param(colors.to_vec()).max_iters(10_000) })
        // Optional: Attach an observer
        .run()?;
    // Print result
    let best_param = res.state().get_best_param().unwrap().clone();

    Ok(best_param) // Return the best parameters
}

#[wasm_bindgen]
pub fn optimize(colors: &[u16], locked_colors: &[u8]) -> Vec<f32> {
    utils::set_panic_hook();

    let float_color_map: Vec<f32> = colors
        .iter()
        .map(|&x| (x as f32) / 255.0)
        .collect::<Vec<f32>>();
    // Convert to Oklab
    let locked_colors_vec = locked_colors
        .iter()
        .map(|&x| x == 1)
        .collect::<Vec<bool>>();

    let oklab_color_map: Vec<f32> = float_color_map
        .chunks(3)
        .map(|color| {
            let rgb = Srgb::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let oklab: Oklab = Oklab::from_color(rgb);
            vec![oklab.l, oklab.a, oklab.b]
        })
        .flatten()
        .collect::<Vec<f32>>();
    let optimized_colors = annealing(&oklab_color_map, &locked_colors_vec).unwrap();
    let optimized_srgb = optimized_colors
        .chunks(3)
        .map(|color| {
            let okl = Oklab::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let rgb: Srgb = Srgb::from_color(okl);
            vec![rgb.red.clamp(0.0, 1.0), rgb.green.clamp(0.0, 1.0), rgb.blue.clamp(0.0, 1.0)]
        })
        .flatten()
        .collect::<Vec<f32>>();
    optimized_srgb
}

fn annealing_cost(
    param: &Vec<f32>,
) -> Result<HashMap<String, f32>, Error> {
    let mut cielab_colors: Vec<Vec<f32>> = Vec::new();
    let oklab_colors = param;
    let c3_instance = c3::C3::new();
    for color in oklab_colors.chunks(3) {
        let okl = Oklab::new(color[0] as f32, color[1] as f32, color[2] as f32);
        let lab: Lab = Lab::from_color(okl);
        cielab_colors.push(vec![lab.l, lab.a, lab.b]);
    }
    let mut lab_palette = Array2::from_shape_vec(
        (oklab_colors.len() / 3, 3),
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
    let avergae_euc_distance = average_pairwise_euclidean_distance(&oklab_palette);
    let mut loss_components = HashMap::new();
    loss_components.insert("perceptural_distance".to_string(), -average_cosine_distance as f32);
    loss_components.insert("name_distance".to_string(), -avergae_euc_distance as f32);

    Ok(loss_components)
}

#[wasm_bindgen]
pub fn calculate_palette_loss(colors: &[u16]) -> JsValue {
    let float_color_map: Vec<f32> = colors
        .iter()
        .map(|&x| (x as f32) / 255.0)
        .collect::<Vec<f32>>();
    // Convert to Oklab


    let oklab_color_map: Vec<f32> = float_color_map
        .chunks(3)
        .map(|color| {
            let rgb = Srgb::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let oklab: Oklab = Oklab::from_color(rgb);
            vec![oklab.l, oklab.a, oklab.b]
        })
        .flatten()
        .collect::<Vec<f32>>();
    let loss = annealing_cost(&oklab_color_map);
    match loss {
        Ok(loss_components) => JsValue::from_serde(&loss_components).unwrap(),
        Err(_) => JsValue::from_str("Error calculating loss"),
    }
}
