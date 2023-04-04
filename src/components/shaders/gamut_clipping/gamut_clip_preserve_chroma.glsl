#pragma glslify: xyz_to_lrgb = require(../conversion/xyz_to_lrgb);
#pragma glslify: lrgb_to_xyz = require(../conversion/lrgb_to_xyz);
#pragma glslify: xyz_to_oklab = require(../conversion/xyz_to_oklab);
#pragma glslify: oklab_to_xyz = require(../conversion/oklab_to_xyz);
#pragma glslify: find_gamut_intersection = require(./find_gamut_intersection);



vec3 gamut_clip_preserve_chroma(vec3 rgb)
{
    if (rgb.r < 1.0 && rgb.g < 1.0 && rgb.b < 1.0 && rgb.r > 0.0 && rgb.g > 0.0 && rgb.b > 0.0)
    return rgb;

    vec3 xyz = lrgb_to_xyz(rgb);
    vec3 lab = xyz_to_oklab(xyz);

    float L = lab.x;
    float eps = 0.00001;
    float C = max(eps, sqrt(lab.y * lab.y + lab.z * lab.z));
    float a_ = lab.y / C;
    float b_ = lab.z / C;

    float L0 = clamp(L, 0.0, 1.0);

    float t = find_gamut_intersection(a_, b_, L, C, L0);
    float L_clipped = L0 * (1.0 - t) + t * L;
    float C_clipped = t * C;

    xyz = oklab_to_xyz(vec3(L_clipped, C_clipped * a_, C_clipped * b_));
    rgb = xyz_to_lrgb(xyz);
    return rgb;
}
#pragma glslify: export(gamut_clip_preserve_chroma)


