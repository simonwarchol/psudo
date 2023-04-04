#pragma glslify: find_cusp = require(./find_cusp);

const float FLT_MAX = 3.402823466e+38;

// Finds intersection of the line defined by
// L = L0 * (1 - t) + t * L1;
// C = t * C1;
// a and b must be normalized so a^2 + b^2 == 1
float find_gamut_intersection(float a, float b, float L1, float C1, float L0)
{
    // Find the cusp of the gamut triangle
    vec2 cusp = find_cusp(a, b);
    float cuspL = cusp.x;
    float cuspC = cusp.y;

    // Find the intersection for upper and lower half seprately
    float t;
    if (((L1 - L0) * cuspC - (cuspL - L0) * C1) <= 0.){
        // Lower half

        t = cuspC * L0 / (C1 * cuspL + cuspC * (L0 - L1));
    } else {
        // Upper half

        // First intersect with triangle
        t = cuspC * (L0 - 1.0) / (C1 * (cuspL - 1.0) + cuspC * (L0 - L1));

        // Then one step Halley's method
        {
            float dL = L1 - L0;
            float dC = C1;

            float k_l = +0.3963377774 * a + 0.2158037573 * b;
            float k_m = -0.1055613458 * a - 0.0638541728 * b;
            float k_s = -0.0894841775 * a - 1.2914855480 * b;

            float l_dt = dL + dC * k_l;
            float m_dt = dL + dC * k_m;
            float s_dt = dL + dC * k_s;

            // If higher accuracy is required, 2 or 3 iterations of the following block can be used:
            {
                float L = L0 * (1.0 - t) + t * L1;
                float C = t * C1;

                float l_ = L + C * k_l;
                float m_ = L + C * k_m;
                float s_ = L + C * k_s;

                float l = l_ * l_ * l_;
                float m = m_ * m_ * m_;
                float s = s_ * s_ * s_;

                float ldt = 3.0 * l_dt * l_ * l_;
                float mdt = 3.0 * m_dt * m_ * m_;
                float sdt = 3.0 * s_dt * s_ * s_;

                float ldt2 = 6.0 * l_dt * l_dt * l_;
                float mdt2 = 6.0 * m_dt * m_dt * m_;
                float sdt2 = 6.0 * s_dt * s_dt * s_;

                float r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s - 1.0;
                float r1 = 4.0767416621 * ldt - 3.3077115913 * mdt + 0.2309699292 * sdt;
                float r2 = 4.0767416621 * ldt2 - 3.3077115913 * mdt2 + 0.2309699292 * sdt2;

                float u_r = r1 / (r1 * r1 - 0.5 * r * r2);
                float t_r = -r * u_r;

                float g = -1.2681437731 * l + 2.6097574011 * m - 0.3413193965 * s - 1.0;
                float g1 = -1.2681437731 * ldt + 2.6097574011 * mdt - 0.3413193965 * sdt;
                float g2 = -1.2681437731 * ldt2 + 2.6097574011 * mdt2 - 0.3413193965 * sdt2;

                float u_g = g1 / (g1 * g1 - 0.5 * g * g2);
                float t_g = -g * u_g;

                float b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s - 1.0;
                float b1 = -0.0041960863 * ldt - 0.7034186147 * mdt + 1.7076147010 * sdt;
                float b2 = -0.0041960863 * ldt2 - 0.7034186147 * mdt2 + 1.7076147010 * sdt2;

                float u_b = b1 / (b1 * b1 - 0.5 * b * b2);
                float t_b = -b * u_b;

                t_r = u_r >= 0.0 ? t_r : FLT_MAX;
                t_g = u_g >= 0.0 ? t_g : FLT_MAX;
                t_b = u_b >= 0.0 ? t_b : FLT_MAX;

                t += min(t_r, min(t_g, t_b));
            }
        }
    }

    return t;
}

#pragma glslify: export(find_gamut_intersection)
