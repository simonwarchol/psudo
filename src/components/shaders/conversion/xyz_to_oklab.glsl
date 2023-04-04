
float cbrt(float x) {
    return sign(x) * pow(abs(x), 1.0 / 3.0);
}

vec3 xyz_to_oklab(vec3 c) {
    float l = 0.8189330101 * c.x + 0.3618667424 * c.y - 0.1288597137 * c.z;
    float m = 0.0329845436 * c.x + 0.9293118715 * c.y + 0.0361456387 * c.z;
    float s = 0.0482003018 * c.x + 0.2643662691 * c.y + 0.6338517070 * c.z;

    float l_ = cbrt(l);
    float m_ = cbrt(m);
    float s_ = cbrt(s);

    float L = 0.2104542553*l_ + 0.7936177850*m_ - 0.0040720468*s_;
    float a = 1.9779984951*l_ - 2.4285922050*m_ + 0.4505937099*s_;
    float b = 0.0259040371*l_ + 0.7827717662*m_ - 0.8086757660*s_;

    return vec3(L, a, b);
}




#pragma glslify: export(xyz_to_oklab)
