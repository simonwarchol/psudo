// shader.wgsl

struct Floats {
    data: array<f32>
};

@group(0)
@binding(0)
var<storage, read_write> srgb_floats: Floats;


fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return select(color / 12.92, pow((color + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), color > vec3<f32>(0.04045));
}

fn linear_to_xyz(color: vec3<f32>) -> vec3<f32> {
    let R = color.r;
    let G = color.g;
    let B = color.b;

    let X = R * 0.4124 + G * 0.3576 + B * 0.1805;
    let Y = R * 0.2126 + G * 0.7152 + B * 0.0722;
    let Z = R * 0.0193 + G * 0.1192 + B * 0.9505;

    return vec3<f32>(X, Y, Z);
}

@compute
@workgroup_size(1, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x * 3u;
    let srgb = vec3<f32>(srgb_floats.data[index], srgb_floats.data[index + 1u], srgb_floats.data[index + 2u]);
    let linear_srgb = srgb_to_linear(srgb);
    let xyz = linear_to_xyz(linear_srgb);
    srgb_floats.data[index] = xyz.x;
    srgb_floats.data[index + 1u] = xyz.y;
    srgb_floats.data[index + 2u] = xyz.z;
}