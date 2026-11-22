#version 440
// Dither's two-tone checker: `ink` in the top-right and bottom-left squares
// of each tile, `ground` in the others, counted from the item's own top-left
// corner, in whole device pixels.
// Build: qsb --qt6 -o dither.frag.qsb dither.frag

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec4 ink;
    vec4 ground;
    // the item's size, and one square (half a tile), in device pixels
    vec2 size;
    float cell;
};

void main() {
    vec2 square = floor(floor(qt_TexCoord0 * size) / cell);
    bool on = mod(square.x + square.y, 2.0) > 0.5;
    // colours arrive premultiplied
    fragColor = (on ? ink : ground) * qt_Opacity;
}
