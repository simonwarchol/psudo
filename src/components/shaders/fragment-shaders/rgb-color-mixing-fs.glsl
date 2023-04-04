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

vec3 color_transfer(vec3 color, float intensity) {
    return color * intensity;
}

vec3 color_transfer_srgb(vec3 color, float intensity) {
    return color_transfer(color, intensity);
}



void mutate_color(inout vec3 rgb, float intensity0, float intensity1, float intensity2, float intensity3, float intensity4, float intensity5, vec2 vTexCoord){
    rgb += color_transfer(colors[0], intensity0);
    rgb += color_transfer(colors[1], intensity1);
    rgb += color_transfer(colors[2], intensity2);
    rgb += color_transfer(colors[3], intensity3);
    rgb += color_transfer(colors[4], intensity4);
    rgb += color_transfer(colors[5], intensity5);
}
