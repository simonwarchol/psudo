// Adapted from https://observablehq.com/@mbostock/lab-and-rgb
const vec3 tristimulus_to_xyzd50 = (vec3(
0.96422, 1, 0.82521
));

const vec3 tristimulus_to_xyzd65 = (vec3(
0.95047, 1, 1.08883
));


const mat3 xyzd50_lrgb = (mat3(
3.1338561, -1.6168667, -0.4906146,
-0.9787684, 1.9161415, 0.0334540,
0.0719453, -0.2289914, 1.4052427
));

const mat3 matrix_xyzd50_xyzd65 = (mat3(
0.9555766, -0.0230393, 0.0631636,
-0.0282895, 1.0099416, 0.0210077,
0.0122982, -0.0204830, 1.3299098
));

float f_2(float t) {
    if (t > 0.20689655172) {
        return pow(t, 3.0);
    } else {
        return 0.12841854934601665 * (t - 4.0/29.0);
    }
}

vec3 _lab_to_xyzd50(vec3 lab) {
    float fl = (lab.x + 16.0) / 116.0;
    float fa =  lab.y / 500.0;
    float fb =  lab.z / 200.0;
    vec3 xyzd50 = vec3(
    tristimulus_to_xyzd50.x * f_2(fl + fa),
    tristimulus_to_xyzd50.y * f_2(fl),
    tristimulus_to_xyzd50.z * f_2(fl - fb)
    );
    return xyzd50;
}

vec3 lab_to_xyzd65(vec3 lab) {
    float fl = (lab.x + 16.0) / 116.0;
    float fa =  lab.y / 500.0;
    float fb =  lab.z / 200.0;
    vec3 xyzd65 = vec3(
    tristimulus_to_xyzd65.x * f_2(fl + fa),
    tristimulus_to_xyzd65.y * f_2(fl),
    tristimulus_to_xyzd65.z * f_2(fl - fb)
    );
    return xyzd65;
}

vec3 lab_to_xyzd50(vec3 lab) {
    vec3 xyzd50 = _lab_to_xyzd50(lab);
    vec3 xyzd65 = matrix_xyzd50_xyzd65 * xyzd50;
    return xyzd65;
}
vec3 lab_to_xyz(vec3 lab) {
    vec3 xyzd65 = lab_to_xyzd65(lab);
    return xyzd65;
}

#pragma glslify: export(lab_to_xyz)

