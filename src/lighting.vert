#version 100
attribute vec3 position;
attribute vec4 normal;
attribute vec2 texcoord;
uniform mat4 Model;
uniform mat4 Projection;
uniform mat4 character_transform;
uniform mat4 character_normal;
uniform float shadow;
uniform float floor_y;
uniform vec3 light_position;
varying vec3 world;
varying vec3 n;
varying vec2 uv;
varying float surface_height;
void main() {
    world = (Model * character_transform * vec4(position, 1.0)).xyz;
    n = normalize(mat3(Model) * mat3(character_normal) * normal.xyz);
    uv = texcoord;
    surface_height = position.y;
    // Project the character onto the floor from the same point key light.
    if (shadow > 0.5) {
        vec3 light = light_position;
        float t = (floor_y + 0.006 - light.y) / (world.y - light.y);
        world = light + (world - light) * t;
    }
    gl_Position = Projection * vec4(world, 1.0);
}
