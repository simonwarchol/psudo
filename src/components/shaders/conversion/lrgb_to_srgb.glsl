// via https://github.com/tobspr/GLSL-Color-Spaces/blob/master/ColorSpaces.inc.glsl
const float SRGB_ALPHA = 0.055;
// Converts a single linear channel to srgb
float linear_to_srgb(float channel) {
    if (channel <= 0.0031308) {
        return 12.92 * channel;
    } else {
        return (1.0 + SRGB_ALPHA) * pow(channel, 1.0 / 2.4) - SRGB_ALPHA;
    }
}
// Converts a linear rgb color to a srgb color (exact, not approximated)
vec3 lrgb_to_srgb(vec3 rgb) {
    return vec3(
    linear_to_srgb(rgb.r),
    linear_to_srgb(rgb.g),
    linear_to_srgb(rgb.b)
    );
}



#pragma glslify: export(lrgb_to_srgb)

