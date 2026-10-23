#version 100
attribute vec3 position;
attribute vec4 normal;
uniform mat4 Model;
uniform mat4 Projection;
uniform mat4 character_transform;
uniform mat4 character_normal;
varying vec3 n;
void main() {
    n = normalize(mat3(Model) * mat3(character_normal) * normal.xyz);
    gl_Position = Projection * Model * character_transform * vec4(position, 1.0);
}
