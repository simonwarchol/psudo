const mat3 M_LRGB_TO_XYZ = mat3(
0.4123908, 0.21263901, 0.01933082,
0.35758434, 0.71516868, 0.11919478,
0.18048079, 0.07219232, 0.95053215
);

vec3 lrgb_to_xyz(vec3 c) {
    return vec3(M_LRGB_TO_XYZ * c);
}
#pragma glslify: export(lrgb_to_xyz)
