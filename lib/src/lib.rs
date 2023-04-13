use kolor::{Color, spaces};
use wasm_bindgen::prelude::*;
use web_sys::console;
use rust_c3::C3;
// use kolor::Rgb;
mod utils;
// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// let c_3 = c3::C3::new();
// static blah: f64 = 10.0;

#[wasm_bindgen]
extern {
    fn alert(s: &str);
}


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
pub fn color_to_oklab(color: &[f32]) -> Vec<f32> {
    let srgb = Color::srgb(0.35, 0.75, 0.8);
    let mut oklab = srgb.to(spaces::OKLAB).value;
    let v32_oklab = vec![oklab.x, oklab.y, oklab.z];
    v32_oklab
}

#[wasm_bindgen]
pub fn color_test(color: f32) -> f32 {
    console::log_1(&"Hello from Rust!".into());
    let c3_instance = C3::new();
    1.0
}
