#version 440
// A 2x2 checker of one ink over nothing, in whole device pixels, on the same
// scene-wide grid as the clearing.
// Build: qsb --qt6 -o checker.frag.qsb checker.frag

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec4 ink;
    vec2 size;
    vec2 origin;
    float cell;
};

void main() {
    vec2 pixel = floor(qt_TexCoord0 * size);
    vec2 square = floor((pixel + origin) / cell);
    bool on = mod(square.x + square.y, 2.0) > 0.5;
    // the colour arrives premultiplied
    fragColor = on ? ink * qt_Opacity : vec4(0.0);
}
