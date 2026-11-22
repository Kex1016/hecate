#version 440
// A free widget's clearing: the background thins out under the widget. Inside
// 64% of the ellipse it is `fill` (base at 94%), from there to 84% a 2x2
// checker of `base`, past that nothing. Everything is decided per checker
// square, on a grid shared by the whole scene, so the edge is made of whole
// squares and two clearings that overlap line up instead of adding up.
// Build: qsb --qt6 -o clearing.frag.qsb clearing.frag

layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec4 fill;
    vec4 base;
    // the item's size and scene position, in device pixels
    vec2 size;
    vec2 origin;
    // one checker square, in device pixels
    float cell;
    // where the solid middle and the checker end, as a share of the radius
    float solid;
    float edge;
};

void main() {
    vec2 pixel = floor(qt_TexCoord0 * size);
    vec2 square = floor((pixel + origin) / cell);
    vec2 centre = ((square + 0.5) * cell - origin) / size;
    float r = length((centre - 0.5) * 2.0);
    bool on = mod(square.x + square.y, 2.0) > 0.5;
    vec4 ink = vec4(0.0);
    if (r < solid)
        ink = on ? base : fill;
    else if (r < edge && on)
        ink = base;
    // colours arrive premultiplied
    fragColor = ink * qt_Opacity;
}
