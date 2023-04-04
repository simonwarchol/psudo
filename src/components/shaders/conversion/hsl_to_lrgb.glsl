vec3 saturate(vec3 c) {
    return clamp(c, 0., 1.);
}

vec3 hue_to_lrgb(float hue){
    hue=fract(hue);
    return saturate(vec3(
    abs(hue*6.-3.)-1.,
    2.-abs(hue*6.-2.),
    2.-abs(hue*6.-4.)
    ));
}

vec3 hsl_to_lrgb(vec3 hsl){
    if (hsl.y==0.){
        return vec3(hsl.z);//Luminance.
    } else {
        float b;
        if (hsl.z<.5){
            b=hsl.z*(1.+hsl.y);
        } else {
            b=hsl.z+hsl.y-hsl.y*hsl.z;
        }
        float a=2.*hsl.z-b;
        return a+hue_to_lrgb(hsl.x)*(b-a);
    }
}
#pragma glslify: export(hsl_to_lrgb)
