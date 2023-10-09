struct Input {
    data: array<f32>
};

struct Lengths {
    intensity_length: u32,
    colors_length: u32,
    colors: array<f32>
};


struct Output {
    data: array<vec4<f32>>
};

const SRGB_ALPHA: f32 = 0.055;

fn srgb_to_lrgb(color: vec3<f32>) -> vec3<f32> {
  // sRGB to linear RGB conversion formula
  let inv_gamma: f32 = 2.4;
  return linear_color;
}


fn linear_to_srgb(channel: f32) -> f32 {
  // Converts a single linear channel to sRGB
  let threshold: f32 = 0.0031308;
   if (channel <= threshold) {
        return 12.92 * channel;
    } else {
        return (1.0 + SRGB_ALPHA) * pow(channel, 1.0 / 2.4) - SRGB_ALPHA;
    }
}

const lrgb_xyz_matrix: mat3x3<f32> = mat3x3<f32>(
        vec3<f32>(0.4123908, 0.21263901, 0.01933082),
        vec3<f32>(0.35758434, 0.71516868, 0.11919478),
        vec3<f32>(0.18048079, 0.07219232, 0.95053215)
    );



fn lrgb_to_xyz(rgb: vec3<f32>) -> vec3<f32> {
    // Converts linear RGB to CIE XYZ
       return lrgb_xyz_matrix * rgb;
}



const xyz_lrgb_matrix: mat3x3<f32> = mat3x3<f32>(
        vec3<f32>(3.24096994, -0.96924364, 0.05563008),
        vec3<f32>(-1.53738318, 1.8759675, -0.20397696),
        vec3<f32>(-0.49861076, 0.04155506, 1.05697151)
    );



fn xyz_to_lrgb(xyz: vec3<f32>) -> vec3<f32> {
    // Converts linear RGB to CIE XYZ
       return xyz_lrgb_matrix * xyz;
}

fn lrgb_to_srgb(rgb: vec3<f32>) -> vec3<f32> {
  // Converts a linear RGB color to a sRGB color
  let srgb_color = vec3(
    linear_to_srgb(rgb.r),
    linear_to_srgb(rgb.g),
    linear_to_srgb(rgb.b)
  );
  return srgb_color;
}


fn xyz_to_oklab(xyz: vec3<f32>) -> vec3<f32> {
    let l: f32 = 0.8189330101 * xyz.x + 0.3618667424 * xyz.y - 0.1288597137 * xyz.z;
    let m: f32 = 0.0329845436 * xyz.x + 0.9293118715 * xyz.y + 0.0361456387 * xyz.z;
    let s: f32 = 0.0482003018 * xyz.x + 0.2643662691 * xyz.y + 0.6338517070 * xyz.z;
    let l_: f32 = pow(l, 1.0 / 3.0);
    let m_: f32 = pow(m, 1.0 / 3.0);
    let s_: f32 = pow(s, 1.0 / 3.0);
    let L: f32 = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_;
    let a: f32 = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_;
    let b: f32 = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_;

    return vec3<f32>(L, a, b);
}

fn oklab_to_xyz(lab: vec3<f32>) -> vec3<f32> {
    let L: f32 = lab.x;
    let a: f32 = lab.y;
    let b: f32 = lab.z;

    let l_: f32 = L + 0.3963377774 * a + 0.2158037573 * b;
    let m_: f32 = L - 0.1055613458 * a - 0.0638541728 * b;
    let s_: f32 = L - 0.0894841775 * a - 1.2914855480 * b;

    let l: f32 = l_ * l_ * l_;
    let m: f32 = m_ * m_ * m_;
    let s: f32 = s_ * s_ * s_;

    return vec3<f32>(
        1.2270138511 * l - 0.5577999807 * m + 0.2812561490 * s,
        -0.0405801784 * l + 1.1122568696 * m - 0.0716766787 * s,
        -0.0763812845 * l - 0.4214819784 * m + 1.5861632204 * s
    );
}

@group(0)
@binding(0)
var<storage, read> intensities: Input;



@group(0)
@binding(1)
var<storage, read> lengths: Lengths;

@group(0)
@binding(2)
var<storage, read_write> output: Output;

@compute
@workgroup_size(256, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    var pixel_color: vec3<f32> = vec3<f32>(0.0,0.0,0.0);
    for (var i : u32 = 0u; i < lengths.colors_length / 3u; i = i + 1u) {

        let color: vec4<f32> = vec4<f32>(lengths.colors[i * 3u], lengths.colors[i * 3u + 1u], lengths.colors[i * 3u + 2u], 0.0);
        let intensity = intensities.data[index * (lengths.colors_length / 3u) + i];
        let color_srgb = vec3<f32>(color.x, color.y, color.z);
        let color_lrgb = srgb_to_lrgb(color_srgb);
        let color_xyz = lrgb_to_xyz(color_lrgb);
        let color_oklab = xyz_to_oklab(color_xyz);
        let color_scaled_by_intensity = vec3<f32>(color_oklab.x * intensity, color_oklab.y * intensity, color_oklab.z * intensity);
        let back_to_xyz = oklab_to_xyz(color_scaled_by_intensity);
        pixel_color = vec3<f32>(pixel_color[0] + back_to_xyz[0], pixel_color[1] + back_to_xyz[1], pixel_color[2] + back_to_xyz[2]);
    }
    let srgb_result = vec3<f32>(lrgb_to_srgb(xyz_to_lrgb(pixel_color)));
    output.data[index] = vec4<f32>(srgb_result.x, srgb_result.y, srgb_result.z, 1.0);
    }
