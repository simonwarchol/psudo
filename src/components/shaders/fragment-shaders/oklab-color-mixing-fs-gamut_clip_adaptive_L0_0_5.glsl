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
uniform float channelsVisible;


#pragma glslify: lrgb_to_srgb = require(../conversion/lrgb_to_srgb);
#pragma glslify: srgb_to_lrgb = require(../conversion/srgb_to_lrgb);
#pragma glslify: xyz_to_lrgb = require(../conversion/xyz_to_lrgb);
#pragma glslify: lrgb_to_xyz = require(../conversion/lrgb_to_xyz);
#pragma glslify: xyz_to_oklab = require(../conversion/xyz_to_oklab);
#pragma glslify: oklab_to_xyz = require(../conversion/oklab_to_xyz);
#pragma glslify: gamut_clip_adaptive_L0_0_5 = require(../gamut_clipping/gamut_clip_adaptive_L0_0_5);



vec3 color_transfer(vec3 color, float intensity) {
    vec3 lrgb = srgb_to_lrgb(color);
    vec3 xyz = lrgb_to_xyz(lrgb);
    vec3 oklab = xyz_to_oklab(xyz);
    oklab = oklab * intensity;
    xyz = oklab_to_xyz(oklab);
    return xyz;
}


vec3 to_srgb(vec3 oklab){
    vec3 xyz = oklab_to_xyz(oklab);
    return lrgb_to_srgb(xyz_to_lrgb(xyz));
}



void mutate_color(inout vec3 rgb, float intensity0, float intensity1, float intensity2, float intensity3, float intensity4, float intensity5, vec2 vTexCoord){
    vec3 xyz = color_transfer(colors[0], intensity0);
    xyz += color_transfer(colors[1], intensity1);
    xyz += color_transfer(colors[2], intensity2);
    xyz += color_transfer(colors[3], intensity3);
    xyz += color_transfer(colors[4], intensity4);
    xyz += color_transfer(colors[5], intensity5);
    rgb = xyz_to_lrgb(xyz);
    rgb = gamut_clip_adaptive_L0_0_5(rgb, 0.05);
    rgb = lrgb_to_srgb(rgb);

}
