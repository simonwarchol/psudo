use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryInto;
use std::future::Future;
use linfa::prelude::*;
use linfa_clustering::{ GaussianMixtureModel };
use linfa::Dataset;
use statrs::distribution::{ Continuous, Normal };

use ndarray::Axis;
use ndarray_stats::QuantileExt;
use bytemuck::{Pod, Zeroable};
use console_error_panic_hook;
use js_sys;
use js_sys::Promise;
use kolor::{Color, spaces};
// use rust_c3::C3;
// use kolor::Rgb;
use ndarray::{Array1, Array2,};
use rayon::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures;
use wasm_bindgen_futures::JsFuture;
use web_sys::console;

mod utils;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

mod c3;
// mod compute;
// mod sa;


#[wasm_bindgen]
extern {
    fn alert(s: &str);
}


// #[wasm_bindgen]
// pub async fn call_wgpu_stuff_v2(shader: &str) -> Vec<f32> {
//     console::log_1(&"Hello from Rust!".into());
//     let result = compute::wgpu_stuff_v2(shader).await;
//     console::log_1(&"Hello from Rust!".into());
//     result.to_vec()
// }
//

// #[wasm_bindgen]
// pub async fn mix_and_color(intensities: &[f32], colors: &[f32]) -> Result<JsValue, JsValue> {
//     let result_slice = compute::mix_and_color(include_str!("compute/mix_and_color.wgsl"), intensities.to_vec(), colors.to_vec()).await;
//     Ok(serde_wasm_bindgen::to_value(&result_slice)?)
// }

// #[wasm_bindgen]
// pub async fn (intensities: &[f32]) -> Result<JsValue, JsValue> {
//     console_error_panic_hook::set_once();
//     let result_vec = compute::optimize(intensities.to_vec());
//     Ok(serde_wasm_bindgen::to_value(&result_vec.unwrap())?)
// }


// #[wasm_bindgen]
// pub async fn saturated_penalty(intensities: &[f32], colors: &[f32]) -> Result<JsValue, JsValue> {
//     let result_slice = compute::saturated_pixel_penalty(include_str!("compute/saturated.wgsl"), intensities.to_vec(), colors.to_vec()).await;
//     let saturated_sum: Vec<f32> = vec![result_slice.par_iter().sum()];
//     Ok(serde_wasm_bindgen::to_value(&saturated_sum)?)
// }

// #[wasm_bindgen]
// pub async fn test() -> Result<JsValue, JsValue> {
//     let result_slice_future = async { sa::anneal() };
//     let result_slice = result_slice_future.await;
//     Ok(serde_wasm_bindgen::to_value(&result_slice)?)
// }


// #[wasm_bindgen]
// pub async fn diff_penalty(intensities: &[f32], colors: &[f32]) -> Result<JsValue, JsValue> {
//     let result_slice = compute::diff_penalty(include_str!("compute/diff.wgsl"), &intensities.to_vec(), colors.to_vec()).await;
//     let diff_sum: Vec<f32> = vec![result_slice.par_iter().sum()];
//     Ok(serde_wasm_bindgen::to_value(&diff_sum)?)
// }


#[wasm_bindgen]
pub fn greet() {
    alert("Hello, psudo-analysis!");
}

// add console logging function


#[wasm_bindgen]
pub fn add_one(x: i16) -> i16 {
    x + 1
}

#[wasm_bindgen]
pub fn apply_ramp_function(array: &[u16], contrast_limits: &[u16]) -> Vec<f32> {
    let mut result = Vec::new();
    let min = contrast_limits[0] as f32;
    let max = contrast_limits[1] as f32;
    let range = max - min;
    for i in array {
        let value = *i as f32;
        if value < min {
            result.push(0.0);
        } else if value > max {
            result.push(1.0);
        } else {
            result.push((value - min) / range);
        }
    }
    result
}


#[wasm_bindgen]
pub fn channel_gmm(array: &[u16])-> Vec<f32> {
    console::log_1(&"Starting GMM!".into());
    // let vals = array.par_iter().map(|&x| x as f32).collect::<Vec<f32>>();
    // let vals_log: Array1<f32> = vals
    //     .iter()
    //     .filter(|&&x| x > 0.0)
    //     .map(|&x| x.ln())
    //     .collect::<Array1<f32>>();

    // console::log_1(&"Logarithmized!".into());

    // let dataset = Dataset::from(vals_log.insert_axis(Axis(1)));
    // let gmm = GaussianMixtureModel::params(3)
    //     .n_runs(1)
    //     .tolerance(1e-6)
    //     .max_n_iterations(1000)
    //     .fit(&dataset)
    //     .expect("GMM fitting");
    // console::log_1(&"Fitted!".into());
    // let means = gmm.means();
    // let covariances = gmm.covariances();
    // let weights = gmm.weights();

    // let flattend_means = means.view().into_shape((means.len(),)).unwrap().to_owned().into_raw_vec();
    // let mut indexed_values: Vec<(usize, f32)> = flattend_means
    //     .iter()
    //     .enumerate()
    //     .map(|(i, &val)| (i, val))
    //     .collect();
    // indexed_values.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    // // Extract indices from the sorted pairs.
    // let (_i0, i1, i2) = (indexed_values[0].0, indexed_values[1].0, indexed_values[2].0);
    // let (mean1, mean2) = (flattend_means[i1], flattend_means[i2]);
    // let (std1, std2) = (covariances[[i1, 0, 0]].sqrt(), covariances[[i2, 0, 0]].sqrt());
    // // Python code to implement
    // let x = Array1::linspace(mean1, mean2, 50);
    // let norm1 = Normal::new(mean1 as f64, std1 as f64).unwrap();
    // let norm2 = Normal::new(mean2 as f64, std2 as f64).unwrap();
    // let y1 = x.mapv(|v| norm1.pdf(v as f64) * (weights[i1] as f64));
    // let y2 = x.mapv(|v| norm2.pdf(v as f64) * (weights[i2] as f64));
    // let lmax = mean2 + 2.0 * std2;
    // // Calculate the differences between y1 and y2, take their absolute values, and get the index of the minimum value
    // let differences = (&y1 - &y2).mapv(|val| val.abs());
    // let mut min_diff_index: usize = 0;
    // let mut min_diff_value = f64::MAX;
    // for (i, &diff) in differences.iter().enumerate() {
    //     if diff < min_diff_value {
    //         min_diff_index = i;
    //         min_diff_value = diff;
    //     }
    // }
    // let mut lmin = x[min_diff_index];
    // // Apply the given condition
    // if lmin >= mean2 {
    //     lmin = mean2 - 2.0 * std2;
    // }
    // let vals_array = Array1::from(vals);

    // let vmin = f32::max(lmin.exp(), f32::max(*vals_array.min().unwrap(), 0.0));
    // let vmax = f32::min(lmax.exp(), *vals_array.max().unwrap());
    // return vec![vmin, vmax];
    return vec![0.1, 0.2];
}

#[wasm_bindgen]
pub fn psudocolor_to_rgb(intensities: &[f32], rgb_colors: &[u16]) -> Vec<f32> {
    // Iterate over every 3 values in colors
    let mut result = Vec::new();
    // Iterate over every 3 values in colorfs
    let oklab_color = rgb_to_oklab_helper([rgb_colors[0] as f32 / 255.0, rgb_colors[1] as f32 / 255.0, rgb_colors[2] as f32 / 255.0].as_ref());
    // // Convert to Array2<f32>
    let oklab_color_array: Array2<f32> = Array2::from_shape_vec((3, 1), oklab_color).unwrap();
    // Multiply color by intensity
    let intensity_array: Array2<f32> = Array2::from_shape_vec((intensities.len(), 1), Vec::from(intensities)).unwrap();
    let colored_array = intensity_array.dot(&oklab_color_array.t());
    println!("Colored Array: {:?}", colored_array.shape());
    // Multiply these matrices
    for row in colored_array.rows() {
        //     // Convert to Vec<f32>
        let v32: Vec<f32> = row.to_vec();
        //     // Push to result
        let srgb_color: Vec<f32> = oklab_to_rgb_helper(v32.as_ref());
        result.push(srgb_color[0]);
        result.push(srgb_color[1]);
        result.push(srgb_color[2]);
    }
    result
}


#[wasm_bindgen]
pub fn rgb_to_oklab(color: &[f32]) -> Vec<f32> {
    let oklabVersion: Vec<f32> = rgb_to_oklab_helper(color);
    oklabVersion
}


fn rgb_to_oklab_helper(color: &[f32]) -> Vec<f32> {
    let srgb = Color::srgb(color[0], color[1], color[2]);
    let mut oklab = srgb.to(spaces::OKLAB).value;
    vec![oklab.x, oklab.y, oklab.z]
}


#[wasm_bindgen]
pub fn oklab_to_rgb(color: &[f32]) -> Vec<f32> {
    let rgbVersion: Vec<f32> = oklab_to_rgb_helper(color);
    rgbVersion
}

fn oklab_to_rgb_helper(color: &[f32]) -> Vec<f32> {
    let oklab = Color::new(color[0], color[1], color[2], spaces::OKLAB);
    let mut oklab = oklab.to(spaces::ENCODED_SRGB).value;
    let v32_oklab = vec![oklab.x, oklab.y, oklab.z];
    v32_oklab
}


fn oklab_to_xyz_helper(color: &[f32]) -> Vec<f32> {
    let oklab = Color::new(color[0], color[1], color[2], spaces::OKLAB);
    let mut oklab = oklab.to(spaces::CIE_XYZ).value;
    let v32_xyz = vec![oklab.x, oklab.y, oklab.z];
    v32_xyz
}


#[wasm_bindgen]
pub fn color_test(color: f32) -> String {
    console::log_1(&"Hello from Rust!".into());
    let c3_instance = c3::C3::new();
    let palette = Array2::from_shape_vec((3, 3), vec![
        60.32273214, 98.2353325, -60.84232404,
        79.42618245, -1.22650957, -19.14108948,
        66.88027726, 43.42296322, 71.85391542,
    ]).unwrap();
    let analyzed_palette: Vec<HashMap<&str, f64>> = c3_instance.analyze_palette(palette.clone());
    // JSONIY this
    let json = serde_json::to_string(&analyzed_palette).unwrap();
    json
}



