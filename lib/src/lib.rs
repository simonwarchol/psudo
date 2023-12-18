mod utils;
use ndarray::{ Array1, Array2, Array3, s };
use web_sys::console;
use linfa::traits::Fit; // Import the Fit trait
use rand::seq::SliceRandom; // Import
use rand::thread_rng; // Import the RNG
use linfa_linear::LinearRegression;
use linfa::prelude::MultiTargetRegression;
use linfa::prelude::Predict; // Continue to include linfa prelude for other necessary traits and structures
use linfa_linear;
use ndarray::Axis;
use rayon::{ prelude::*, vec };
use linfa::prelude::*;
use linfa_clustering::{ GaussianMixtureModel };
use linfa::Dataset;
use statrs::distribution::{ Continuous, Normal };
use ndarray_stats::QuantileExt; // <-- Add this line
use palette::{ FromColor, Oklab, Srgb, Lab, Xyz };
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

fn preprocess_data(colors: &[u16], intensities: &[u16], contrast_limits: &[u16]) -> Array2<f32> {
    let num_channels = colors.len() / 3;
    let num_rows = intensities.len() / num_channels;
    let mut intensities_array = Array2::zeros((num_rows, num_channels));
    for channel in 0..num_channels {
        for row in 0..num_rows {
            let index = channel * num_rows + row;
            intensities_array[[row, channel]] = intensities[index];
        }
    }
    let mut intensities_array_float: Array2<f32> = Array2::zeros((num_rows, num_channels));

    for channel in 0..num_channels {
        for row in 0..num_rows {
            let index = channel * num_rows + row;
            if intensities[index] < contrast_limits[channel * 2] {
                intensities_array[[row, channel]] = contrast_limits[channel * 2];
            } else if intensities[index] > contrast_limits[channel * 2 + 1] {
                intensities_array[[row, channel]] = contrast_limits[channel * 2 + 1];
            }
            // Subtract the lower limit from the value
            intensities_array[[row, channel]] -= contrast_limits[channel * 2];
            intensities_array_float[[row, channel]] =
                (intensities_array[[row, channel]] as f32) /
                ((contrast_limits[channel * 2 + 1] - contrast_limits[channel * 2]) as f32);
        }
    }
    // Compute the sum of each row
    // let mut row_sums = Array1::zeros(num_rows);
    let mut indexes = Vec::new();
    for row in 0..num_rows {
        let mut row_sum = 0.0;
        for channel in 0..num_channels {
            row_sum += intensities_array_float[[row, channel]];
        }
        if row_sum > 0.3 {
            // print row index
            indexes.push(row);
        }
    }
    // println!("indexes: {:?}", indexes);
    // Shuffle the indexes
    let mut rng = thread_rng();
    indexes.shuffle(&mut rng);
    let mut rng = rand::thread_rng();
    // print length of indexes
    println!("indexes length: {:?}", indexes.len());
    indexes = indexes
        .iter()
        .take(5000)
        .map(|&x| x)
        .collect::<Vec<usize>>();

    // subsample intensities_array_float to the first 5000 indices
    let mut subsampled_array = Array2::zeros((indexes.len(), num_channels));
    for (i, &index) in indexes.iter().enumerate() {
        for channel in 0..num_channels {
            subsampled_array[[i, channel]] = intensities_array_float[[index, channel]];
        }
    }
    // print shape of subsampled_array
    println!("subsampled_array shape: {:?}", subsampled_array.shape());
    subsampled_array
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

fn min_euclidean_distance(matrix: &Array2<f64>) -> f64 {
    let len = matrix.nrows();
    let mut distance_sum = 0.0;
    let mut count = 0;
    let mut min_dist = f64::MAX;
    for i in 0..len {
        for j in i + 1..len {
            // Bind the temporary ArrayView to variables so they live long enough
            let a_view = matrix.row(i);
            let b_view = matrix.row(j);
            let a_slice = a_view.as_slice().unwrap();
            let b_slice = b_view.as_slice().unwrap();

            let this_dist = euclidean_distance(a_slice, b_slice);
            if this_dist < min_dist {
                min_dist = this_dist;
            }
            // count += 1;
        }
    }
    min_dist
}

// ///////////////////////////////////////// Optimization /////////////////////////////////////
struct Loss {
    rng: Arc<Mutex<Xoshiro256PlusPlus>>,
    locked_colors: Vec<bool>,
    intensity_array: Array2<f32>,
    luminance_values: Vec<f32>,
    avg_confusion: f32,
    excluded_colors_indices: Vec<f32>,
    c3_instance: c3::C3,
}

impl Loss {
    pub fn new(
        locked_colors: Vec<bool>,
        intensity_array: Array2<f32>,
        luminance_values: Vec<f32>,
        avg_confusion: f32,
        excluded_colors_indices: Vec<f32>,
        c3_instance: c3::C3
    ) -> Self {
        Self {
            rng: Arc::new(Mutex::new(Xoshiro256PlusPlus::from_entropy())),
            locked_colors: locked_colors,
            c3_instance: c3_instance,
            intensity_array: intensity_array,
            luminance_values: luminance_values,
            avg_confusion: avg_confusion,
            excluded_colors_indices: excluded_colors_indices,
        }
    }
}

fn term_loss(
    palette_terms: Vec<Vec<HashMap<&str, f64>>>,
    excluded_colors_indices: Vec<f32>
) -> f32 {
    let mut loss = 0.0;
    for term in palette_terms.iter() {
        for color in term {
            if excluded_colors_indices.contains(&(color["index"] as f32)) {
                loss += color["score"];
            }
        }
    }

    loss as f32
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
        let palette_terms = self.c3_instance.get_palette_terms(lab_palette.clone(), 10);

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
        let min_euclidean_distance = min_euclidean_distance(&oklab_palette);
        let term_loss = term_loss(palette_terms.clone(), self.excluded_colors_indices.clone());
        let confusion_loss = compute_confusion_loss(
            oklab_colors.to_vec(),
            self.intensity_array.clone(),
            self.avg_confusion
        );

        Ok(
            (-average_cosine_distance as f32) +
                (-min_euclidean_distance as f32) +
                (term_loss as f32) +
                0.1 * (confusion_loss as f32)
        )
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
        // log param.len()
        // Iterate over each color (assuming each color is represented by 3 values in the vector)
        for _ in 0..(temp.floor() as u64) + 1 {
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
                        // param_n[idx] = param_n[idx].clamp(-0.0, 1.0);
                        param_n[idx] = param_n[idx].clamp(
                            self.luminance_values[0],
                            self.luminance_values[1]
                        );
                    } else {
                        // Clamp the value to ensure it stays within bounds
                        param_n[idx] = param_n[idx].clamp(-0.4, 0.4);
                    }
                }
            }
        }
        // log param

        Ok(param_n)
    }
}
fn annealing(
    colors: &Vec<f32>,
    locked_colors: &Vec<bool>,
    intensity_array: Array2<f32>,
    luminance_values: &Vec<f32>,
    excluded_colors_indices: &Vec<f32>,
    c3_instance: c3::C3
) -> Result<Vec<f32>, Error> {
    let solver = SimulatedAnnealing::new(15.0)?;

    //
    let average_confusion = calculate_average_confusion(
        luminance_values,
        colors,
        intensity_array.clone()
    );
    // Randomize non locked colors
    let mut rng = thread_rng();
    let mut random_colors = Vec::new();
    for (i, &color) in colors.iter().enumerate() {
        if locked_colors[i / 3] {
            random_colors.push(color);
        } else {
            let val = if i % 3 == 0 {
                rng.gen_range(luminance_values[0]..luminance_values[1])
            } else {
                rng.gen_range(-0.4..0.4)
            };
            random_colors.push(val);
        }
    }

    let cost_function = Loss::new(
        locked_colors.clone(),
        intensity_array,
        luminance_values.clone(),
        average_confusion,
        excluded_colors_indices.clone(),
        c3_instance
    );
    // Optional: Define temperature function (defaults to `SATempFunc::TemperatureFast`)
    let res = Executor::new(cost_function, solver)
        .configure(|state| { state.param(random_colors.to_vec()).max_iters(6_000) })
        // Optional: Attach an observer
        .run()?;
    // Print loss from best_param
    console::log_1(&format!("loss: {:?}", res.state().best_cost).into());
    let best_param = res.state().get_best_param().unwrap().clone();
    // log best param
    console::log_1(&format!("best_param: {:?}", best_param).into());

    Ok(best_param) // Return the best parameters
}

#[wasm_bindgen]
pub fn optimize(
    colors: &[u16],
    locked_colors: &[u16],
    intensities: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>
) -> Vec<f32> {
    utils::set_panic_hook();
    let now = instant::Instant::now();
    // print luminance values
    // log excluded_colors

    // Convert to float
    let float_luminance_values: Vec<f32> = luminance_values
        .iter()
        .map(|&x| (x as f32) / 100.0)
        .collect::<Vec<f32>>();

    let float_color_map: Vec<f32> = colors
        .iter()
        .map(|&x| (x as f32) / 255.0)
        .collect::<Vec<f32>>();
    // Convert to Oklab
    let locked_colors_vec = locked_colors
        .iter()
        .map(|&x| x == 1)
        .collect::<Vec<bool>>();

    console::log_1(&format!("locked_colors_vec: {:?}", locked_colors_vec).into());

    let oklab_color_map: Vec<f32> = float_color_map
        .chunks(3)
        .map(|color| {
            let rgb = Srgb::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let oklab: Oklab = Oklab::from_color(rgb);
            vec![oklab.l, oklab.a, oklab.b]
        })
        .flatten()
        .collect::<Vec<f32>>();
    let intensity_array = preprocess_data(colors, intensities, contrast_limits);
    // Excluded colors list to list of indices
    let c3_instance = c3::C3::new();
    let mut excluded_colors_indices = Vec::new();
    for color in excluded_colors {
        let analyzed_color = c3_instance.get_term_index(&color);
        // if color is some, push
        if let Some(index) = analyzed_color {
            excluded_colors_indices.push(index as f32);
        }
    }

    console::log_1(&format!("excluded_colors_indices: {:?}", excluded_colors_indices).into());

    let optimized_colors = annealing(
        &oklab_color_map,
        &locked_colors_vec,
        intensity_array,
        &float_luminance_values,
        &excluded_colors_indices,
        c3_instance
    ).unwrap();
    let optimized_srgb = optimized_colors
        .chunks(3)
        .map(|color| {
            let okl = Oklab::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let rgb: Srgb = Srgb::from_color(okl);
            vec![rgb.red.clamp(0.0, 1.0), rgb.green.clamp(0.0, 1.0), rgb.blue.clamp(0.0, 1.0)]
        })
        .flatten()
        .collect::<Vec<f32>>();
    let elapsed = now.elapsed();
    console::log_1(&format!("annealing took: {:?}", elapsed).into());
    optimized_srgb
}

fn color_only_loss(
    param: &Vec<f32>,
    excluded_colors: Vec<String>
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
    // console log the lab palette
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
    let mut excluded_colors_indices = Vec::new();
    for color in excluded_colors {
        let analyzed_color = c3_instance.get_term_index(&color);
        // if color is some, push
        if let Some(index) = analyzed_color {
            excluded_colors_indices.push(index as f32);
        }
    }

    let term_loss_val = term_loss(
        c3_instance.get_palette_terms(lab_palette.clone(), 10),
        excluded_colors_indices
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

    //

    let average_cosine_distance = total_distance / (total_pairs as f64);
    let min_euclidean_distance = min_euclidean_distance(&oklab_palette);
    let mut loss_components = HashMap::new();
    loss_components.insert("name_distance".to_string(), -average_cosine_distance as f32);
    loss_components.insert("perceptural_distance".to_string(), -min_euclidean_distance as f32);
    loss_components.insert("term_loss".to_string(), term_loss_val as f32);
    Ok(loss_components)
}

#[wasm_bindgen]
pub fn calculate_palette_loss(
    intensities: &[u16],
    colors: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16],
    excluded_colors: Vec<String>
) -> JsValue {
    // Log all values
    let float_color_map: Vec<f32> = colors
        .iter()
        .map(|&x| (x as f32) / 255.0)
        .collect::<Vec<f32>>();

    // Convert to Oklab

    let float_luminance_values: Vec<f32> = luminance_values
        .iter()
        .map(|&x| (x as f32) / 100.0)
        .collect::<Vec<f32>>();

    let oklab_color_map: Vec<f32> = float_color_map
        .chunks(3)
        .map(|color| {
            let rgb = Srgb::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let oklab: Oklab = Oklab::from_color(rgb);
            vec![oklab.l, oklab.a, oklab.b]
        })
        .flatten()
        .collect::<Vec<f32>>();
    let mut loss: HashMap<String, f32> = color_only_loss(
        &oklab_color_map,
        excluded_colors
    ).unwrap();

    let confusion = optimize_for_confusion(
        intensities,
        colors,
        contrast_limits,
        &float_luminance_values
    );

    loss.insert("confusion".to_string(), confusion - (1.0 as f32));
    JsValue::from_serde(&loss).unwrap()
}

fn calculate_ols_msre(dataset: Dataset<f32, f32>) -> Result<f32, Box<dyn std::error::Error>> {
    let num_targets = dataset.targets().dim().1;
    let mut total_mse = 0.0;

    // Fit a model on each target individually
    for i in 0..num_targets {
        let target_column = dataset.targets().column(i).to_owned();

        let dataset_with_single_target = Dataset::new(
            dataset.records().to_owned(),
            target_column.clone()
        );
        let model = LinearRegression::new();
        let fitted_model = model.fit(&dataset_with_single_target)?;
        let predictions = fitted_model.predict(&dataset_with_single_target);

        let mse = (predictions - target_column)
            .mapv(|x| x.powi(2))
            .mean()
            .unwrap();
        println!("mse: {:?}", mse);
        total_mse += mse;
    }

    // Calculate Avg MSE
    let avg_mse = total_mse / (num_targets as f32);
    // Take log of avg_mse
    let sqrt_mse = avg_mse.sqrt() * 10.0;
    // let cbrt_mse = avg_mse.cbrt();
    Ok(sqrt_mse)
}

fn compute_confusion_loss(
    oklab_color_map: Vec<f32>,
    intensities_array_float: Array2<f32>,
    avg_confusion: f32
) -> f32 {
    let num_channels = intensities_array_float.shape()[1];
    let num_rows = intensities_array_float.shape()[0];
    // Psudocolor each channel by applying the color ramp
    // Create a 3d Array where the first dimension is the channel, the second dimension is the row, and the third dimension is the color
    let mut colored_array = Array3::zeros((num_channels, num_rows, 3));
    let mut mixed_array: Array2<f32> = Array2::zeros((num_rows, 3));
    for row in 0..num_rows {
        for channel in 0..num_channels {
            let index = channel * num_rows + row;
            let intensity_value = intensities_array_float[[row, channel]];
            let oklab_colored = Oklab::new(
                oklab_color_map[channel * 3] * intensity_value,
                oklab_color_map[channel * 3 + 1] * intensity_value,
                oklab_color_map[channel * 3 + 2] * intensity_value
            );
            let xyz: Xyz = Xyz::from_color(oklab_colored);
            colored_array[[channel, row, 0]] = xyz.x;
            colored_array[[channel, row, 1]] = xyz.y;
            colored_array[[channel, row, 2]] = xyz.z;
            mixed_array[[row, 0]] += xyz.x;
            mixed_array[[row, 1]] += xyz.y;
            mixed_array[[row, 2]] += xyz.z;
        }
        let okl = Oklab::from_color(
            Xyz::new(mixed_array[[row, 0]], mixed_array[[row, 1]], mixed_array[[row, 2]])
        );
        mixed_array[[row, 0]] = okl.l;
        mixed_array[[row, 1]] = okl.a;
        mixed_array[[row, 2]] = okl.b;
    }

    let dataset = Dataset::new(intensities_array_float, mixed_array);
    let rmse = calculate_ols_msre(dataset);
    // console::log_1(&format!("rmse: {:?}", rmse).into());
    rmse.unwrap() / avg_confusion
}

fn calculate_average_confusion(
    luminance_values: &Vec<f32>,
    colors: &Vec<f32>,
    intensities_array_float: Array2<f32>
) -> f32 {
    let mut rng = thread_rng();
    let mut total_confusion = 0.0;
    let mut num_samples = 0;
    for _ in 0..100 {
        let mut random_colors = Vec::new();
        for color in colors.chunks(3) {
            for (i, &val) in color.iter().enumerate() {
                let val = if i == 0 {
                    rng.gen_range(luminance_values[0]..luminance_values[1])
                } else {
                    rng.gen_range(-0.4..0.4)
                };
                random_colors.push(val);
            }
        }
        let confusion = compute_confusion_loss(random_colors, intensities_array_float.clone(), 1.0);
        total_confusion += confusion;
        num_samples += 1;
    }
    let avg_confusion = total_confusion / (num_samples as f32);
    // console::log_1(&format!("luminance_values: {:?}", luminance_values).into());
    // console::log_1(&format!("avg_confusion: {:?}", avg_confusion).into());
    avg_confusion
}

fn optimize_for_confusion(
    intensities: &[u16],
    colors: &[u16],
    contrast_limits: &[u16],
    float_luminance_values: &Vec<f32>
) -> f32 {
    let num_channels = colors.len() / 3;
    let mut num_rows = intensities.len() / num_channels;
    let intensities_array_float: Array2<f32> = preprocess_data(
        colors,
        intensities,
        contrast_limits
    );

    num_rows = intensities_array_float.shape()[0];

    // Print first 10 rows in 0th channel
    let float_color_map: Vec<f32> = colors
        .iter()
        .map(|&x| (x as f32) / 255.0)
        .collect::<Vec<f32>>();

    let oklab_color_map: Vec<f32> = float_color_map
        .chunks(3)
        .map(|color| {
            let rgb = Srgb::new(color[0] as f32, color[1] as f32, color[2] as f32);
            let oklab: Oklab = Oklab::from_color(rgb);
            vec![oklab.l, oklab.a, oklab.b]
        })
        .flatten()
        .collect::<Vec<f32>>();
    println!("color_map: {:?}", oklab_color_map);
    let avg_confusion: f32 = calculate_average_confusion(
        float_luminance_values,
        &oklab_color_map,
        intensities_array_float.clone()
    );

    let rmse = compute_confusion_loss(oklab_color_map, intensities_array_float, avg_confusion);
    rmse
}

#[wasm_bindgen]
pub fn optimize_in_lens(
    intensities: &[u16],
    colors: &[u16],
    contrast_limits: &[u16],
    luminance_values: &[u16]
) -> f32 {
    // console log colors

    let float_luminance_values: Vec<f32> = luminance_values
        .iter()
        .map(|&x| (x as f32) / 100.0)
        .collect::<Vec<f32>>();

    optimize_for_confusion(intensities, colors, contrast_limits, &float_luminance_values)
}
