//// via https://www.shadertoy.com/view/Mdlfzf , https://www.shadertoy.com/view/Mdlfzf
//
//vec3 rgb2xyz (in vec3 rgb) {
//    float r = rgb.r;
//    float g = rgb.g;
//    float b = rgb.b;
//
//    r = r > 0.04045 ? pow(((r + 0.055) / 1.055), 2.4) : (r / 12.92);
//    g = g > 0.04045 ? pow(((g + 0.055) / 1.055), 2.4) : (g / 12.92);
//    b = b > 0.04045 ? pow(((b + 0.055) / 1.055), 2.4) : (b / 12.92);
//
//    float x = (r * 0.4124) + (g * 0.3576) + (b * 0.1805);
//    float y = (r * 0.2126) + (g * 0.7152) + (b * 0.0722);
//    float z = (r * 0.0193) + (g * 0.1192) + (b * 0.9505);
//
//    vec3 xyz = vec3(
//    (r * 0.4124) + (g * 0.3576) + (b * 0.1805) * 100.0,
//    (r * 0.2126) + (g * 0.7152) + (b * 0.0722) * 100.0,
//    (r * 0.0193) + (g * 0.1192) + (b * 0.9505) * 100.0
//    );
//    return (xyz);
//}
//
//vec3 xyz2lab (in vec3 xyz) {
//    float x = xyz.x / 95.047;
//    float y = xyz.y / 100.0;
//    float z = xyz.z / 108.883;
//
//    x = x > 0.008856 ? pow(x, 1.0 / 3.0) : (7.787 * x) + (16.0 / 116.0);
//    y = y > 0.008856 ? pow(y, 1.0 / 3.0) : (7.787 * y) + (16.0 / 116.0);
//    z = z > 0.008856 ? pow(z, 1.0 / 3.0) : (7.787 * z) + (16.0 / 116.0);
//
//    vec3 lab = vec3((116.0 * y) - 16.0, 500.0 * (x - y), 200.0 * (y - z));
//    return (lab);
//}
//
//vec3 rgb2lab(in vec3 rgb) {
//    vec3 xyz = rgb2xyz(rgb);
//    vec3 lab = xyz2lab(xyz);
//    return (lab);
//}
//
//vec3 xyz2rgb (in vec3 xyz) {
//    float x = xyz.x / 100.0;
//    float y = xyz.y / 100.0;
//    float z = xyz.z / 100.0;
//
//
//    float r = (x *  3.2406) + (y * -1.5372) + (z * -0.4986);
//    float g = (x * -0.9689) + (y *  1.8758) + (z *  0.0415);
//    float b = (x *  0.0557) + (y * -0.2040) + (z *  1.0570);
//
//    r = r > 0.0031308 ? ((1.055 * pow(r, 1.0 / 2.4)) - 0.055) : r * 12.92;
//    g = g > 0.0031308 ? ((1.055 * pow(g, 1.0 / 2.4)) - 0.055) : g * 12.92;
//    b = b > 0.0031308 ? ((1.055 * pow(b, 1.0 / 2.4)) - 0.055) : b * 12.92;
//
//    r = min(max(0.0, r), 1.0);
//    g = min(max(0.0, g), 1.0);
//    b = min(max(0.0, b), 1.0);
//
//    return (vec3(r, g, b));
//}
//
//vec3 lab2xyz (in vec3 lab) {
//    float l = lab.x;
//    float a = lab.y;
//    float b = lab.z;
//
//    float y = (l + 16.0) / 116.0;
//    float x = a / 500.0 + y;
//    float z = y - b / 200.0;
//
//    float y2 = pow(y, 3.0);
//    float x2 = pow(x, 3.0);
//    float z2 = pow(z, 3.0);
//
//    y = y2 > 0.008856 ? y2 : (y - 16.0 / 116.0) / 7.787;
//    x = x2 > 0.008856 ? x2 : (x - 16.0 / 116.0) / 7.787;
//    z = z2 > 0.008856 ? z2 : (z - 16.0 / 116.0) / 7.787;
//
//    x *= 95.047;
//    y *= 100.0;
//    z *= 108.883;
//
//    return (vec3(x, y, z));
//}
//vec3 lab2rgb (in vec3 lab) {
//    vec3 xyz = lab2xyz(lab);
//    vec3 rgb = xyz2rgb(xyz);
//    return (rgb);
//}
//
//const vec3 lab0 = vec3(20.0, 100.0, -50.0);
//const vec3 lab1 = vec3(100.0, 50.0, -50.0);
//#pragma glslify: export(lab2rgb)
//#pragma glslify: export(rgb2lab)
//#pragma glslify: export(rgb2xyz)
//#pragma glslify: export(xyz2rgb)
//
