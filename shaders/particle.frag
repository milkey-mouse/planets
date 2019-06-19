#version 450

layout(location = 0) out vec4 f_color;

void main() {
    f_color = vec4(1);

    /*float hue = mod((p_hue * 6.0), 6.0);
    float interp = 1.0 - abs(mod(hue, 2.0) - 1.0);

    if (0.0 <= hue && hue < 1.0) {
        f_color = vec4(1, interp, 0, 1);
    } else if (1.0 <= hue && hue < 2.0) {
        f_color = vec4(interp, 1, 0, 1);
    } else if (2.0 <= hue && hue < 3.0) {
        f_color = vec4(0, 1, interp, 1);
    } else if (3.0 <= hue && hue < 4.0) {
        f_color = vec4(0, interp, 1, 1);
    } else if (4.0 <= hue && hue < 5.0) {
        f_color = vec4(interp, 0, 1, 1);
    } else if (5.0 <= hue && hue < 6.0) {
        f_color = vec4(1, 0, interp, 1);
    } else {
        f_color = vec4(1, 1, 1, 1);
    }*/
}
