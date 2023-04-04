// via https://github.com/tobspr/GLSL-Color-Spaces/blob/master/ColorSpaces.inc.glsl
const float SRGB_ALPHA = 0.055;
// Converts a single srgb channel to rgb
float srgb_to_linear(float channel) {
    if (channel <= 0.04045)
    return channel / 12.92;
    else
    return pow((channel + SRGB_ALPHA) / (1.0 + SRGB_ALPHA), 2.4);
}
// Converts a srgb color to a linear rgb color (exact, not approximated)
vec3 srgb_to_lrgb(vec3 srgb) {
    return vec3(
    srgb_to_linear(srgb.r),
    srgb_to_linear(srgb.g),
    srgb_to_linear(srgb.b)
    );
}
#pragma glslify: export(srgb_to_lrgb)
