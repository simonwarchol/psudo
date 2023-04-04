// Adapted from https://observablehq.com/@mbostock/lab-and-rgb
const vec3 tristimulus_to_xyzd50 = (vec3(
0.96422, 1, 0.82521
));

const vec3 tristimulus_to_xyzd65 = (vec3(
0.95047, 1, 1.08883
));


const mat3 matrix_xyzd65_xyzd50 = (mat3(
1.0478112, 0.0228866, -0.0501270,
0.0295424, 0.9904844, -0.0170491,
-0.0092345, 0.0150436, 0.7521316
));

float f_1(float t) {
    if (t > 0.00885645167903563082) {
        return pow(t, 1.0/3.0);
    } else {
        return 7.78703703703703704 * t + 16.0/116.0;
    }
}

vec3 xyzd50_to_lab(vec3 xyz) {

    float fx = f_1(xyz.x / tristimulus_to_xyzd50.x);
    float fy = f_1(xyz.y / tristimulus_to_xyzd50.y);
    float fz = f_1(xyz.z / tristimulus_to_xyzd50.z);
    return vec3(
    116.0 * fy - 16.0,
    500.0 * (fx - fy),
    200.0 * (fy - fz)
    );
}
vec3 xyzd65_to_lab(vec3 xyz) {

    float fx = f_1(xyz.x / tristimulus_to_xyzd65.x);
    float fy = f_1(xyz.y / tristimulus_to_xyzd65.y);
    float fz = f_1(xyz.z / tristimulus_to_xyzd65.z);
    return vec3(
    116.0 * fy - 16.0,
    500.0 * (fx - fy),
    200.0 * (fy - fz)
    );
}

vec3 xyz_to_lab_d50(vec3 xyz) {
    vec3 xyzd50 = vec3(xyz * matrix_xyzd65_xyzd50);
    vec3 lab = xyzd50_to_lab(xyzd50);
    return lab;
}
vec3 xyz_to_lab(vec3 xyz) {
    vec3 lab = xyzd65_to_lab(xyz);
    return lab;
}

#pragma glslify: export(xyz_to_lab)
