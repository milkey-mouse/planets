#version 450

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 velocity;

void main() {
    gl_PointSize = 5;
    gl_Position = vec4(position, 0.0, 1.0);
}
