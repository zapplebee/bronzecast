import React, { useEffect, useRef } from "react";
import * as THREE from "three";
import { MTLLoader } from "three/addons/loaders/MTLLoader.js";
import { OBJLoader } from "three/addons/loaders/OBJLoader.js";

const WIDTH = 960;
const HEIGHT = 540;
const FLOOR_Y = -2.41;

const LIGHTING_VERTEX = `
uniform mat4 character_transform;
uniform mat4 character_normal;
uniform float shadow;
uniform float floor_y;
uniform vec3 light_position;
varying vec3 world;
varying vec3 n;
varying vec2 vUv;
varying float surface_height;
void main() {
    world = (modelMatrix * character_transform * vec4(position, 1.0)).xyz;
    n = normalize(mat3(modelMatrix) * mat3(character_normal) * normal);
    vUv = uv;
    surface_height = position.y;
    if (shadow > 0.5) {
        float t = (floor_y + 0.006 - light_position.y) / (world.y - light_position.y);
        world = light_position + (world - light_position) * t;
    }
    gl_Position = projectionMatrix * viewMatrix * vec4(world, 1.0);
}`;

const LIGHTING_FRAGMENT = `
uniform sampler2D Texture;
uniform float shadow;
uniform float ground;
uniform float floor_y;
uniform vec3 light_position;
uniform vec3 light_target;
varying vec3 world;
varying vec3 n;
varying vec2 vUv;
varying float surface_height;
float coverage_threshold(vec2 pixel) {
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
    float to_aim = (light_target.y - light.y) / (world.y - light.y);
    vec2 footprint = (light + (world - light) * to_aim).xz;
    float beam_radius = length((footprint - light_target.xz) * vec2(0.85, 1.0));
    float pool = ground > 0.5
        ? 1.0 - smoothstep(3.2, 4.1, beam_radius)
        : 1.0 - smoothstep(3.45, 3.95, beam_radius);
    float threshold = ground > 0.5
        ? coverage_threshold(gl_FragCoord.xy)
        : coverage_threshold(vec2(0.0, surface_height * 32.0));
    if (pool <= threshold) {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    float lightness = diffuse;
    float shade = floor(smoothstep(0.12, 0.90, lightness) * 4.0) / 4.0;
    vec3 bone = mix(vec3(0.31, 0.35, 0.27), vec3(1.0, 1.0, 0.94), shade);
    bone *= texture2D(Texture, vUv).rgb;
    bone *= smoothstep(0.10, 0.23, lightness);
    vec3 floor_color = vec3(0.57, 0.55, 0.58);
    gl_FragColor = vec4(mix(bone, floor_color, ground), 1.0);
}`;

const NORMAL_VERTEX = `
uniform mat4 character_transform;
uniform mat4 character_normal;
varying vec3 n;
void main() {
    n = normalize(mat3(modelMatrix) * mat3(character_normal) * normal);
    vec4 world = modelMatrix * character_transform * vec4(position, 1.0);
    gl_Position = projectionMatrix * viewMatrix * world;
}`;

const NORMAL_FRAGMENT = `
varying vec3 n;
void main() {
    gl_FragColor = vec4(normalize(n) * 0.5 + 0.5, 1.0);
}`;

const POST_VERTEX = `
varying vec2 vUv;
void main() {
    vUv = uv;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`;

const POST_FRAGMENT = `
uniform sampler2D SceneTexture;
uniform sampler2D NormalTexture;
uniform vec2 resolution;
uniform float time;
varying vec2 vUv;
void main() {
    vec2 px = 1.0 / resolution;
    vec3 shaded = texture2D(SceneTexture, vUv).rgb;
    vec4 norm = texture2D(NormalTexture, vUv);
    vec4 nx = texture2D(NormalTexture, vUv + vec2(px.x, 0.0));
    vec4 nxm = texture2D(NormalTexture, vUv - vec2(px.x, 0.0));
    vec4 ny = texture2D(NormalTexture, vUv + vec2(0.0, px.y));
    vec4 nym = texture2D(NormalTexture, vUv - vec2(0.0, px.y));
    vec4 nx2 = texture2D(NormalTexture, vUv + vec2(2.0 * px.x, 0.0));
    vec4 nxm2 = texture2D(NormalTexture, vUv - vec2(2.0 * px.x, 0.0));
    vec4 ny2 = texture2D(NormalTexture, vUv + vec2(0.0, 2.0 * px.y));
    vec4 nym2 = texture2D(NormalTexture, vUv - vec2(0.0, 2.0 * px.y));
    float silhouette = max(max(abs(norm.a - nx.a), abs(norm.a - nxm.a)),
                           max(abs(norm.a - ny.a), abs(norm.a - nym.a)));
    float wide_silhouette = max(max(abs(norm.a - nx2.a), abs(norm.a - nxm2.a)),
                                max(abs(norm.a - ny2.a), abs(norm.a - nym2.a)));
    silhouette = max(silhouette, wide_silhouette * 0.58);
    float crease = max(length(norm.rgb - nx.rgb), length(norm.rgb - ny.rgb));
    float contour = max(silhouette, smoothstep(0.78, 1.05, crease) * norm.a * 0.35);
    float brightness = max(shaded.r, max(shaded.g, shaded.b));
    float line_mask = 0.72 + 0.28 * cos(6.2831853 * gl_FragCoord.y / 4.0);
    vec3 color = shaded;
    color *= 1.0 - contour * norm.a * 0.45;
    float rim = contour * (0.35 + 0.65 * smoothstep(0.35, 0.85, max(norm.g, ny.g)));
    color += vec3(0.40, 0.51, 0.30) * rim * 1.45
        * (1.0 - smoothstep(0.12, 0.40, brightness));
    gl_FragColor = vec4(color * line_mask, 1.0);
}`;

export function MogStage() {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const renderer = new THREE.WebGLRenderer({
      canvas: canvas.current!,
      antialias: false,
      alpha: false,
      powerPreference: "high-performance",
    });
    renderer.setPixelRatio(1);
    renderer.setSize(WIDTH, HEIGHT, false);
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.toneMapping = THREE.NoToneMapping;
    renderer.autoClear = false;

    const sceneTarget = new THREE.WebGLRenderTarget(WIDTH, HEIGHT, {
      format: THREE.RGBAFormat,
      minFilter: THREE.NearestFilter,
      magFilter: THREE.NearestFilter,
      depthBuffer: true,
      stencilBuffer: false,
    });
    sceneTarget.texture.colorSpace = THREE.NoColorSpace;
    const normalTarget = new THREE.WebGLRenderTarget(WIDTH, HEIGHT, {
      format: THREE.RGBAFormat,
      minFilter: THREE.NearestFilter,
      magFilter: THREE.NearestFilter,
      depthBuffer: true,
      stencilBuffer: false,
    });
    normalTarget.texture.colorSpace = THREE.NoColorSpace;

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(43, WIDTH / HEIGHT, 0.1, 120);
    camera.position.set(2.4, 1.9, -7.5);
    camera.lookAt(0, -0.15, 0);

    const commonUniforms = {
      shadow: { value: 0 },
      floor_y: { value: FLOOR_Y },
      light_position: { value: new THREE.Vector3() },
      light_target: { value: new THREE.Vector3() },
      character_transform: { value: new THREE.Matrix4() },
      character_normal: { value: new THREE.Matrix4() },
    };
    const lightingMaterial = (
      texture: THREE.Texture,
      ground: number,
      transformUniforms = commonUniforms,
    ) => new THREE.ShaderMaterial({
      vertexShader: LIGHTING_VERTEX,
      fragmentShader: LIGHTING_FRAGMENT,
      uniforms: {
        ...commonUniforms,
        ...transformUniforms,
        Texture: { value: texture },
        ground: { value: ground },
      },
      side: THREE.DoubleSide,
      depthTest: true,
      depthWrite: true,
    });

    const white = new THREE.DataTexture(
      new Uint8Array([255, 255, 255, 255]),
      1,
      1,
      THREE.RGBAFormat,
    );
    white.colorSpace = THREE.NoColorSpace;
    white.magFilter = THREE.NearestFilter;
    white.minFilter = THREE.NearestFilter;
    white.generateMipmaps = false;
    white.needsUpdate = true;

    const floorGeometry = new THREE.PlaneGeometry(16, 16);
    floorGeometry.rotateX(-Math.PI / 2);
    floorGeometry.translate(0, FLOOR_Y, 0);
    const floorMaterial = lightingMaterial(white, 1, {
      character_transform: { value: new THREE.Matrix4() },
      character_normal: { value: new THREE.Matrix4() },
    });
    const floor = new THREE.Mesh(floorGeometry, floorMaterial);
    scene.add(floor);

    const modelGroup = new THREE.Group();
    const modelMeshes: THREE.Mesh[] = [];
    scene.add(modelGroup);

    const normalScene = new THREE.Scene();
    const normalGroup = new THREE.Group();
    normalScene.add(normalGroup);
    const normalMaterial = new THREE.ShaderMaterial({
      vertexShader: NORMAL_VERTEX,
      fragmentShader: NORMAL_FRAGMENT,
      uniforms: {
        character_transform: commonUniforms.character_transform,
        character_normal: commonUniforms.character_normal,
      },
      side: THREE.DoubleSide,
      depthTest: true,
      depthWrite: true,
    });

    const postScene = new THREE.Scene();
    const postCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
    const postMaterial = new THREE.ShaderMaterial({
      vertexShader: POST_VERTEX,
      fragmentShader: POST_FRAGMENT,
      uniforms: {
        SceneTexture: { value: sceneTarget.texture },
        NormalTexture: { value: normalTarget.texture },
        resolution: { value: new THREE.Vector2(WIDTH, HEIGHT) },
        time: { value: 0 },
      },
      depthTest: false,
      depthWrite: false,
    });
    postScene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), postMaterial));

    const textures = new Set<THREE.Texture>([white]);
    const sourceMaterials = new Map<THREE.Material, THREE.ShaderMaterial>();
    const getLightingMaterial = (source: THREE.Material) => {
      const cached = sourceMaterials.get(source);
      if (cached) return cached;
      const map = (source as THREE.MeshPhongMaterial).map ?? white;
      textures.add(map);
      map.colorSpace = THREE.NoColorSpace;
      map.magFilter = THREE.NearestFilter;
      map.minFilter = THREE.NearestFilter;
      map.generateMipmaps = false;
      map.needsUpdate = true;
      const material = lightingMaterial(map, 0);
      sourceMaterials.set(source, material);
      return material;
    };

    const loader = new MTLLoader();
    loader.setPath("/assets/mog/").load("mog.mtl", (materials) => {
      materials.preload();
      new OBJLoader()
        .setMaterials(materials)
        .setPath("/assets/mog/")
        .load("mog.obj", (model) => {
          model.rotation.set(0, Math.PI, 0);
          model.updateMatrixWorld(true);
          const bounds = new THREE.Box3().setFromObject(model);
          const extent = bounds.getSize(new THREE.Vector3());
          const center = bounds.getCenter(new THREE.Vector3());
          const scale = Math.min(4.8 / extent.y, 3.8 / extent.x, 3.5 / extent.z);
          const normalize = new THREE.Matrix4()
            .makeTranslation(0, FLOOR_Y + 0.01, 0)
            .multiply(new THREE.Matrix4().makeScale(scale, scale, scale))
            .multiply(new THREE.Matrix4().makeTranslation(-center.x, -bounds.min.y, -center.z));

          model.traverse((object) => {
            if (!(object instanceof THREE.Mesh)) return;
            const geometry = object.geometry.clone();
            geometry.applyMatrix4(normalize.clone().multiply(object.matrixWorld));
            const materialsForMesh = Array.isArray(object.material)
              ? object.material.map(getLightingMaterial)
              : getLightingMaterial(object.material);
            const mesh = new THREE.Mesh(geometry, materialsForMesh);
            modelGroup.add(mesh);
            modelMeshes.push(mesh);
            normalGroup.add(new THREE.Mesh(geometry, normalMaterial));
          });
          window.__MOG_READY__ = true;
        }, undefined, (error) => console.error("Could not load Mog OBJ:", error));
    }, undefined, (error) => console.error("Could not load Mog materials:", error));

    const started = performance.now();
    let animationFrame = 0;
    let active = true;
    const render = (now: number) => {
      if (!active) return;
      const t = (now - started) / 1000;
      const hop = Math.abs(Math.sin(t * Math.PI / 2.8));
      const drift = Math.sin(t * Math.PI * 2 / 8.4);
      const squash = (1 - hop) ** 6;
      const characterTransform = new THREE.Matrix4()
        .makeTranslation(0.32 * drift, FLOOR_Y + 0.10 + 0.62 * hop, 0.08 * Math.sin(t * 0.9))
        .multiply(new THREE.Matrix4().makeRotationZ(-0.045 * drift))
        .multiply(new THREE.Matrix4().makeRotationY(0.16 * drift))
        .multiply(new THREE.Matrix4().makeScale(
          0.88 + 0.025 * squash,
          0.88 - 0.04 * squash,
          0.88 + 0.025 * squash,
        ))
        .multiply(new THREE.Matrix4().makeTranslation(0, -FLOOR_Y, 0));
      commonUniforms.character_transform.value.copy(characterTransform);
      commonUniforms.character_normal.value.copy(characterTransform).invert().transpose();

      const swing = Math.sin(t * Math.PI * 2 / 32 - 0.25);
      commonUniforms.light_position.value.set(22 * swing - 6, 22, -32);
      commonUniforms.light_target.value.set(7 * swing, 0.6, 0);

      renderer.setRenderTarget(sceneTarget);
      renderer.setClearColor(0x000000, 1);
      renderer.clear(true, true, true);
      floor.visible = true;
      modelGroup.visible = false;
      renderer.render(scene, camera);

      floor.visible = false;
      modelGroup.visible = true;
      commonUniforms.shadow.value = 1;
      renderer.render(scene, camera);
      commonUniforms.shadow.value = 0;
      renderer.render(scene, camera);

      renderer.setRenderTarget(normalTarget);
      renderer.setClearColor(0x000000, 0);
      renderer.clear(true, true, true);
      normalGroup.visible = true;
      renderer.render(normalScene, camera);
      normalGroup.visible = false;

      renderer.setRenderTarget(null);
      renderer.setClearColor(0x000000, 1);
      renderer.clear(true, true, true);
      postMaterial.uniforms.time.value = t;
      renderer.render(postScene, postCamera);
      animationFrame = requestAnimationFrame(render);
    };
    animationFrame = requestAnimationFrame(render);

    return () => {
      active = false;
      cancelAnimationFrame(animationFrame);
      renderer.dispose();
      sceneTarget.dispose();
      normalTarget.dispose();
      floorGeometry.dispose();
      postScene.traverse((object) => {
        if (object instanceof THREE.Mesh) object.geometry.dispose();
      });
      for (const mesh of modelMeshes) mesh.geometry.dispose();
      for (const material of sourceMaterials.values()) material.dispose();
      normalMaterial.dispose();
      postMaterial.dispose();
      for (const texture of textures) texture.dispose();
    };
  }, []);

  return <canvas className="mog-canvas" ref={canvas} width={WIDTH} height={HEIGHT} />;
}
