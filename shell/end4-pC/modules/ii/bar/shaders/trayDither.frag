#version 440
// A tray icon in two inks: each pixel's lightness against a 4x4 Bayer
// threshold picks `light` or `dark`; pixels under half alpha are dropped.
// Build: qsb --qt6 -o trayDither.frag.qsb trayDither.frag

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec4 light;
    vec4 dark;
    vec2 cells;
};

layout(binding = 1) uniform sampler2D source;

float bayer2(vec2 a) {
    a = floor(a);
    return fract(a.x / 2.0 + a.y * a.y * 0.75);
}

float bayer4(vec2 a) {
    return bayer2(0.5 * a) * 0.25 + bayer2(a);
}

void main() {
    vec2 cell = floor(qt_TexCoord0 * cells);
    vec4 c = texture(source, (cell + 0.5) / cells);
    if (c.a < 0.5) {
        fragColor = vec4(0.0);
        return;
    }
    vec3 rgb = c.rgb / c.a;
    float lightness = dot(rgb, vec3(0.299, 0.587, 0.114));
    // the inks arrive premultiplied
    vec4 ink = lightness > bayer4(cell) + 1.0 / 32.0 ? light : dark;
    fragColor = ink * qt_Opacity;
}
