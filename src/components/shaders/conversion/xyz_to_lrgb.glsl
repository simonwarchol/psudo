const mat3 M_XYZ_TO_LRGB = mat3(
3.24096994, -0.96924364, 0.05563008,
-1.53738318, 1.8759675, -0.20397696,
-0.49861076, 0.04155506, 1.05697151
);


vec3 xyz_to_lrgb(vec3 c) {
    return M_XYZ_TO_LRGB * c;
}
#pragma glslify: export(xyz_to_lrgb)
