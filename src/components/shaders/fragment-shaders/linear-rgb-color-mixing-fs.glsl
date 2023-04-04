// lens bounds for ellipse
uniform float majorLensAxis;
uniform float minorLensAxis;
uniform vec2 lensCenter;

// lens uniforms
uniform bool lensEnabled;
uniform int lensSelection;
uniform vec3 lensBorderColor;
uniform float lensBorderRadius;

// color palette
uniform vec3 colors[6];
uniform int channelsVisible;


#pragma glslify: srgb_to_lrgb = require(../conversion/srgb_to_lrgb);
#pragma glslify: lrgb_to_srgb = require(../conversion/lrgb_to_srgb);
//#pragma glslify: lrgb_to_xyz = require(../conversion/lrgb_to_xyz);
//#pragma glslify: xyz_to_lrgb = require(../conversion/xyz_to_lrgb);
#pragma glslify: lrgb_to_hsl = require(../conversion/lrgb_to_hsl);
#pragma glslify: hsl_to_lrgb = require(../conversion/hsl_to_lrgb);


vec3 color_transfer(vec3 color, float intensity) {
    vec3 c = srgb_to_lrgb(color);
    c = lrgb_to_hsl(c);
    c.z = c.z * intensity;
    c = hsl_to_lrgb(c);
    return c;
}

vec3 color_transfer_srgb(vec3 color, float intensity) {
    vec3 lrgb = color_transfer(color, intensity);
    return lrgb_to_srgb(lrgb);
}

vec3 to_srgb(vec3 lrgb){
    return lrgb_to_srgb(lrgb);
}


void mutate_color(inout vec3 rgb, float intensity0, float intensity1, float intensity2, float intensity3, float intensity4, float intensity5, vec2 vTexCoord){
    vec3 lrgb = color_transfer(colors[0], intensity0);
    lrgb += color_transfer(colors[1], intensity1);
    lrgb += color_transfer(colors[2], intensity2);
    lrgb += color_transfer(colors[3], intensity3);
    lrgb += color_transfer(colors[4], intensity4);
    lrgb += color_transfer(colors[5], intensity5);
    rgb = lrgb_to_srgb(lrgb);
}
