use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryInto;
use std::future::Future;

use bytemuck::{Pod, Zeroable};
use kolor::{Color, spaces};
// use rust_c3::C3;
// use kolor::Rgb;
use ndarray::{Array1, Array2, Axis};
use wasm_bindgen::prelude::*;
use web_sys::console;

use js_sys;
use js_sys::Promise;
use wasm_bindgen_futures;
use wasm_bindgen_futures::JsFuture;

mod utils;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

mod c3;
mod compute;


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

#[wasm_bindgen]
pub async fn get_from_js() -> Result<JsValue, JsValue> {
    let result_slice = compute::wgpu_stuff_v2("shader").await;
    Ok(serde_wasm_bindgen::to_value(&result_slice)?)
}

// #[wasm_bindgen]
// pub async fn get_from_test() -> Result<JsValue, JsValue> {
//     let result_slice = test_compute::run().await;
//     let result_slice_str = result_slice.iter().map(|x| x.to_string()).collect::<Vec<String>>();
//     Ok(serde_wasm_bindgen::to_value(&result_slice)?)
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
pub fn optimize_palette(intensities: &[f32], rgb_colors: &[u16]) -> f32 {
    // Sum of rgb_colors
    let mut sum = 0.0;
    for i in rgb_colors {
        sum += *i as f32;
    }
    sum
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
