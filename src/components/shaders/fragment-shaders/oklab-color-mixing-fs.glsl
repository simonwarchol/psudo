// lens bounds for ellipse
uniform float majorLensAxis;
uniform float minorLensAxis;
uniform vec2 lensCenter;

// lens uniforms
uniform bool lensEnabled;
// uniform int lensSelection;
uniform vec3 lensBorderColor;
uniform float lensOpacity;
uniform float lensBorderRadius;

// color palette
uniform vec3 colors[6];
uniform float channelsVisible;
uniform int lensSelection[6];


bool frag_in_lens_bounds(vec2 vTexCoord) {
  // Check membership in what is (not visually, but effectively) an ellipse.
  // Since the fragment space is a unit square and the real coordinates could be longer than tall,
  // to get a circle visually we have to treat the check as that of an ellipse to get the effect of a circle.

  // Check membership in ellipse.
  return pow((lensCenter.x - vTexCoord.x) / majorLensAxis, 2.) + pow((lensCenter.y - vTexCoord.y) / minorLensAxis, 2.) < (1. - lensBorderRadius);
}

bool frag_on_lens_bounds(vec2 vTexCoord) {
  // Same as the above, except this checks the boundary.

  float ellipseDistance = pow((lensCenter.x - vTexCoord.x) / majorLensAxis, 2.) + pow((lensCenter.y - vTexCoord.y) / minorLensAxis, 2.);

  // Check membership on "bourndary" of ellipse.
  return ellipseDistance <= 1. && ellipseDistance >= (1. - lensBorderRadius);
}


#pragma glslify: lrgb_to_srgb = require(../conversion/lrgb_to_srgb);
#pragma glslify: srgb_to_lrgb = require(../conversion/srgb_to_lrgb);
//
#pragma glslify: xyz_to_lrgb = require(../conversion/xyz_to_lrgb);
#pragma glslify: lrgb_to_xyz = require(../conversion/lrgb_to_xyz);
#pragma glslify: xyz_to_oklab = require(../conversion/xyz_to_oklab);
#pragma glslify: oklab_to_xyz = require(../conversion/oklab_to_xyz);
//


vec3 color_transfer(vec3 color, float intensity, vec2 vTexCoord, int channelIndex) {
    vec3 lrgb = srgb_to_lrgb(color);
    vec3 xyz = lrgb_to_xyz(lrgb);
    vec3 oklab = xyz_to_oklab(xyz);
    vec3 blendedWithBlack = oklab * intensity;
    xyz = oklab_to_xyz(blendedWithBlack);

    bool isFragInLensBounds = frag_in_lens_bounds(vTexCoord);
    bool inLensAndUseLens = lensEnabled && isFragInLensBounds;
    bool channelsInLens = !(lensSelection[0] == 0 && lensSelection[1] == 0 
                       && lensSelection[2] == 0 && lensSelection[3] == 0 
                       && lensSelection[4] == 0 && lensSelection[5] == 0);
    inLensAndUseLens = inLensAndUseLens && channelsInLens;
    bool isSelectedChannel = lensSelection[channelIndex] != 0;

    // When the lens is disabled, and it's the selected channel, we want full intensity
    bool useSelectedChannelDirectly = !lensEnabled && isSelectedChannel;

    float factorOutside = 1.0 - float(isSelectedChannel);
    float factorInside = isSelectedChannel ? lensOpacity : (1.0 - lensOpacity);

    // Factor for selected channel when the lens is disabled
    float factorSelectedChannel = 1.0;

    // Choose the appropriate factor based on our conditions
    float factor = inLensAndUseLens ? factorInside : (useSelectedChannelDirectly ? factorSelectedChannel : factorOutside);

    return factor * xyz;
}

vec3 color_transfer_srgb(vec3 color, float intensity, vec2 vTexCoord, int channelIndex) {
    vec3 xyz = color_transfer(color, intensity, vTexCoord, channelIndex); // Fixed the function call
    return lrgb_to_srgb(xyz_to_lrgb(xyz));
}


vec3 to_srgb(vec3 oklab){
    vec3 xyz = oklab_to_xyz(oklab);
    return lrgb_to_srgb(xyz_to_lrgb(xyz));
}




void mutate_color(inout vec3 rgb, float intensity0, float intensity1, float intensity2, float intensity3, float intensity4, float intensity5, vec2 vTexCoord){
    vec3 xyz = color_transfer(colors[0], intensity0, vTexCoord, 0);
    xyz += color_transfer(colors[1], intensity1, vTexCoord, 1);
    xyz += color_transfer(colors[2], intensity2, vTexCoord, 2);
    xyz += color_transfer(colors[3], intensity3, vTexCoord, 3); // Fixed the typo here
    xyz += color_transfer(colors[4], intensity4, vTexCoord, 4);
    xyz += color_transfer(colors[5], intensity5, vTexCoord, 5);
    rgb = xyz_to_lrgb(xyz);
    rgb = lrgb_to_srgb(rgb);
}
