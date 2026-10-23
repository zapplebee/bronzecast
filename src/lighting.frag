#version 100
precision highp float;
varying vec3 world;
varying vec3 n;
varying vec2 uv;
varying float surface_height;
uniform sampler2D Texture;
uniform float shadow;
uniform float ground;
uniform float floor_y;
uniform vec3 light_position;
uniform vec3 light_target;
float coverage_threshold(vec2 pixel) {
    // Bit-reversed row ranks distribute coverage across eight horizontal rows.
    // No X dependence: no column grid, crosshatch, or repeating vertical seams.
    float row = mod(floor(pixel.y), 8.0);
    float rank = 4.0 * mod(row, 2.0)
        + 2.0 * mod(floor(row / 2.0), 2.0) + floor(row / 4.0);
    return (rank + 0.5) / 8.0;
}
void main() {
    if (shadow > 0.5) { gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0); return; }
    vec3 light = light_position;
    vec3 direction = normalize(light - world);
    float diffuse = max(dot(normalize(n), direction), 0.0);
    // Project each surface point along the light ray to the beam's aim plane.
    // With an oblique source, head/body/feet intersect different parts of the
    // cone; the ground footprint falls behind the character rather than under it.
    float to_aim = (light_target.y - light.y) / (world.y - light.y);
    vec2 footprint = (light + (world - light) * to_aim).xz;
    float beam_radius = length((footprint - light_target.xz) * vec2(0.85, 1.0));
    // Ground is a broad projected footprint; the foreground character has a
    // tighter transition and finer surface-attached coverage, not the same
    // screen-space stencil pasted over both planes.
    float pool = ground > 0.5
        ? 1.0 - smoothstep(3.2, 4.1, beam_radius)
        : 1.0 - smoothstep(3.45, 3.95, beam_radius);
    float threshold = ground > 0.5
        ? coverage_threshold(gl_FragCoord.xy)
        : coverage_threshold(vec2(0.0, surface_height * 32.0));
    // Spotlight falloff changes coverage, never the surviving pixels' color.
    // Ordered coverage is stable between frames, unlike random pixel dropout.
    if (pool <= threshold) {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    float lightness = diffuse;
    float shade = floor(smoothstep(0.12, 0.90, lightness) * 4.0) / 4.0;
    vec3 bone = mix(vec3(0.31, 0.35, 0.27), vec3(1.0, 1.0, 0.94), shade);
    bone *= texture2D(Texture, uv).rgb;
    bone *= smoothstep(0.10, 0.23, lightness);
    vec3 floor_color = vec3(0.57, 0.55, 0.58);
    gl_FragColor = vec4(mix(bone, floor_color, ground), 1.0);
}
