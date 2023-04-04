vec3 oklab_to_xyz(vec3 c) {
    float L = c.x;
    float a = c.y;
    float b = c.z;

    float l_ = L + 0.3963377774 * a + 0.2158037573 * b;
    float m_ = L - 0.1055613458 * a - 0.0638541728 * b;
    float s_ = L - 0.0894841775 * a - 1.2914855480 * b;

    float l = l_*l_*l_;
    float m = m_*m_*m_;
    float s = s_*s_*s_;

    return vec3(
    +1.2270138511 * l - 0.5577999807 * m + 0.2812561490 * s,
    -0.0405801784 * l + 1.1122568696 * m - 0.0716766787 * s,
    -0.0763812845 * l - 0.4214819784 * m + 1.5861632204 * s
    );
}

#pragma glslify: export(oklab_to_xyz)
