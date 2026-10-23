use macroquad::prelude::*;
mod characters;
mod notifications;
use std::{
    io::Write,
    process::{Child, ChildStdin, Command, Stdio},
};

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;
const STREAM_FPS: f32 = 12.0;

const VERTEX_SHADER: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
uniform mat4 Model;
uniform mat4 Projection;
varying vec2 uv;
void main() {
    uv = texcoord;
    gl_Position = Projection * Model * vec4(position, 1.0);
}"#;

const FRAGMENT_SHADER: &str = r#"#version 100
precision highp float;
varying vec2 uv;
uniform sampler2D SceneTexture;
uniform sampler2D NormalTexture;
uniform vec2 resolution;
uniform float time;
void main() {
    vec2 px = 1.0 / resolution;
    vec2 raster = uv;
    vec3 shaded = texture2D(SceneTexture, raster).rgb;
    vec4 norm = texture2D(NormalTexture, raster);
    vec4 nx = texture2D(NormalTexture, raster + vec2(px.x, 0.0));
    vec4 ny = texture2D(NormalTexture, raster + vec2(0.0, px.y));
    float silhouette = max(abs(norm.a - nx.a), abs(norm.a - ny.a));
    float crease = max(length(norm.rgb - nx.rgb), length(norm.rgb - ny.rgb));
    float contour = max(silhouette, smoothstep(0.78, 1.05, crease) * norm.a * 0.35);
    float brightness = max(shaded.r, max(shaded.g, shaded.b));
    // Smooth, evenly spaced bands survive video/display resampling better than
    // hard one-pixel black rows. Apply after all shading/contours below so no
    // surface feature overwrites or thickens a scanline.
    float line_mask = 0.72 + 0.28 * cos(6.2831853 * gl_FragCoord.y / 4.0);
    vec3 color = shaded;
    // Sparse olive contours survive darkness; bright surfaces retain black creases.
    color *= 1.0 - contour * norm.a * 0.45;
    float rim = contour * (0.35 + 0.65 * smoothstep(0.35, 0.85, max(norm.g, ny.g)));
    color += vec3(0.40, 0.49, 0.29) * rim * (1.0 - smoothstep(0.12, 0.40, brightness));
    gl_FragColor = vec4(color * line_mask, 1.0);
}"#;

fn window_conf() -> Conf {
    Conf {
        window_title: "BRONZECAST // live encoder".into(),
        window_width: 960,
        window_height: 600,
        window_resizable: true,
        ..Default::default()
    }
}

fn start_encoder() -> Result<(Child, ChildStdin), String> {
    std::fs::create_dir_all("stream").map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file("stream/live.m3u8");
    let mut child = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "warning",
            "-y",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            "960x540",
            "-framerate",
            "12",
            "-i",
            "-",
            "-vf",
            "vflip",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
            "-pix_fmt",
            "yuv420p",
            "-g",
            "24",
            "-sc_threshold",
            "0",
            "-f",
            "hls",
            "-hls_time",
            "2",
            "-hls_list_size",
            "6",
            "-hls_flags",
            "delete_segments+append_list+omit_endlist",
            "stream/live.m3u8",
        ])
        .stdin(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("could not start ffmpeg (install ffmpeg and add it to PATH): {e}"))?;
    let stdin = child.stdin.take().ok_or("could not open ffmpeg input")?;
    Ok((child, stdin))
}

fn start_web_server(feed: notifications::SharedFeed, selection: characters::SharedSelection) {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async {
            use axum::Router;
            use tower_http::{
                cors::CorsLayer, services::ServeDir, set_header::SetResponseHeaderLayer,
            };

            let app = Router::new()
                .route(
                    "/api/notifications",
                    axum::routing::get(notifications::list).post(notifications::post),
                )
                .with_state(feed)
                .merge(characters::routes(selection))
                .nest_service(
                    "/stream",
                    Router::new()
                        .fallback_service(ServeDir::new("stream"))
                        .layer(SetResponseHeaderLayer::overriding(
                            axum::http::header::CACHE_CONTROL,
                            axum::http::HeaderValue::from_static("no-store"),
                        ))
                        .layer(CorsLayer::permissive()),
                )
                .nest_service("/assets/characters", ServeDir::new("characters"))
                .fallback_service(ServeDir::new("web"));
            let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
                .await
                .expect("bind port 8080");
            println!("BRONZECAST web player: http://localhost:8080");
            if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
                if socket.connect("192.0.2.1:80").is_ok() {
                    if let Ok(addr) = socket.local_addr() {
                        println!("LAN player: http://{}:8080", addr.ip());
                    }
                }
            }
            axum::serve(listener, app)
                .await
                .expect("serve video player");
        });
    });
}

#[macroquad::main(window_conf)]
async fn main() {
    macroquad::window::gl_set_drawcall_buffer_capacity(65_535, 65_535);
    let (loaded_characters, selection) = match characters::load_all() {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    let material = load_material(
        ShaderSource::Glsl {
            vertex: VERTEX_SHADER,
            fragment: FRAGMENT_SHADER,
        },
        MaterialParams {
            uniforms: vec![
                UniformDesc::new("time", UniformType::Float1),
                UniformDesc::new("resolution", UniformType::Float2),
            ],
            textures: vec!["SceneTexture".to_owned(), "NormalTexture".to_owned()],
            ..Default::default()
        },
    )
    .expect("shader compile");
    let lighting = load_material(
        ShaderSource::Glsl {
            vertex: include_str!("lighting.vert"),
            fragment: include_str!("lighting.frag"),
        },
        MaterialParams {
            pipeline_params: miniquad::PipelineParams {
                depth_test: miniquad::Comparison::LessOrEqual,
                depth_write: true,
                ..Default::default()
            },
            uniforms: vec![
                UniformDesc::new("shadow", UniformType::Float1),
                UniformDesc::new("ground", UniformType::Float1),
                UniformDesc::new("floor_y", UniformType::Float1),
                UniformDesc::new("light_position", UniformType::Float3),
                UniformDesc::new("light_target", UniformType::Float3),
                UniformDesc::new("character_transform", UniformType::Mat4),
                UniformDesc::new("character_normal", UniformType::Mat4),
            ],
            ..Default::default()
        },
    )
    .expect("lighting shader compile");
    let normal_material = load_material(
        ShaderSource::Glsl {
            vertex: include_str!("normals.vert"),
            fragment: include_str!("normals.frag"),
        },
        MaterialParams {
            uniforms: vec![
                UniformDesc::new("character_transform", UniformType::Mat4),
                UniformDesc::new("character_normal", UniformType::Mat4),
            ],
            pipeline_params: miniquad::PipelineParams {
                depth_test: miniquad::Comparison::LessOrEqual,
                depth_write: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .expect("normal shader compile");
    let normal_target = render_target_ex(
        WIDTH,
        HEIGHT,
        RenderTargetParams {
            depth: true,
            ..Default::default()
        },
    );
    normal_target.texture.set_filter(FilterMode::Nearest);
    material.set_texture("NormalTexture", normal_target.texture.clone());
    let floor_y = characters::FLOOR_Y;
    let floor = Mesh {
        vertices: [(-8.0, -8.0), (8.0, -8.0), (8.0, 8.0), (-8.0, 8.0)]
            .map(|(x, z)| Vertex {
                position: vec3(x, floor_y, z),
                uv: Vec2::ZERO,
                color: WHITE.into(),
                normal: vec4(0.0, 1.0, 0.0, 0.0),
            })
            .to_vec(),
        indices: vec![0, 2, 1, 0, 3, 2],
        texture: None,
    };
    lighting.set_uniform("floor_y", floor_y);
    let scene_target = render_target_ex(
        WIDTH,
        HEIGHT,
        RenderTargetParams {
            depth: true,
            ..Default::default()
        },
    );
    scene_target.texture.set_filter(FilterMode::Nearest);
    let output_target = render_target(WIDTH, HEIGHT);
    output_target.texture.set_filter(FilterMode::Nearest);
    material.set_texture("SceneTexture", scene_target.texture.clone());
    let (mut encoder, mut input) = match start_encoder() {
        Ok(v) => v,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    let feed = notifications::SharedFeed::default();
    start_web_server(feed.clone(), selection.clone());
    let started = get_time() as f32;
    let mut frame = 0u64;
    let mut next_encode_at = 0.0f32;

    loop {
        let character =
            &loaded_characters[characters::active_index(&loaded_characters, &selection)];
        let t = get_time() as f32 - started;
        // One hop every 2.8 seconds, with a longer side-to-side flight loop.
        // Feet are the pivot, and the slightly smaller silhouette leaves headroom.
        let hop = (t * std::f32::consts::PI / 2.8).sin().abs();
        let drift = (t * std::f32::consts::TAU / 8.4).sin();
        let squash = (1.0 - hop).powi(6);
        let scale = vec3(
            0.88 + 0.025 * squash,
            0.88 - 0.04 * squash,
            0.88 + 0.025 * squash,
        );
        let bank = Quat::from_rotation_z(-0.045 * drift) * Quat::from_rotation_y(0.16 * drift);
        let animated_transform = Mat4::from_translation(vec3(
            0.32 * drift,
            floor_y + 0.10 + 0.62 * hop,
            0.08 * (t * 0.9).sin(),
        )) * Mat4::from_quat(bank)
            * Mat4::from_scale(scale)
            * Mat4::from_translation(vec3(0.0, -floor_y, 0.0));
        let character_transform = if character.spec.bounce {
            animated_transform
        } else {
            Mat4::IDENTITY
        };
        let character_normal = character_transform.inverse().transpose();
        // A distant overhead pendulum: one full out-and-back sweep every 32 seconds.
        // Sinusoidal motion slows naturally at the ends; the source remains far
        // enough away that rays are almost parallel across the character.
        // A low, forward key rakes across Mog instead of washing down from above.
        let swing = (t * std::f32::consts::TAU / 32.0 - 0.25).sin();
        lighting.set_uniform("light_position", vec3(22.0 * swing - 6.0, 22.0, -32.0));
        lighting.set_uniform("light_target", vec3(7.0 * swing, 0.6, 0.0));
        let camera = Camera3D {
            position: vec3(2.4, 1.9, -7.5),
            target: vec3(0.0, -0.15, 0.0),
            up: vec3(0.0, 1.0, 0.0),
            fovy: 43.0_f32.to_radians(),
            viewport: Some((0, 0, 440, 420)),
            aspect: Some(440.0 / 420.0),
            render_target: Some(scene_target.clone()),
            ..Default::default()
        };
        set_camera(&camera);
        clear_background(BLACK);
        gl_use_material(&lighting);
        lighting.set_uniform("ground", 1.0f32);
        lighting.set_uniform("shadow", 0.0f32);
        lighting.set_uniform("character_transform", Mat4::IDENTITY);
        lighting.set_uniform("character_normal", Mat4::IDENTITY);
        draw_mesh(&floor);
        lighting.set_uniform("character_transform", character_transform);
        lighting.set_uniform("character_normal", character_normal);
        lighting.set_uniform("ground", 0.0f32);
        lighting.set_uniform("shadow", 1.0f32);
        for mesh in &character.meshes {
            draw_mesh(mesh);
        }
        lighting.set_uniform("shadow", 0.0f32);
        for mesh in &character.meshes {
            draw_mesh(mesh);
        }
        gl_use_default_material();

        // A separate normal/silhouette image keeps contours visible without
        // inventing ambient surface light when the spotlight has moved away.
        let normal_camera = Camera3D {
            render_target: Some(normal_target.clone()),
            ..camera
        };
        set_camera(&normal_camera);
        clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
        gl_use_material(&normal_material);
        normal_material.set_uniform("character_transform", character_transform);
        normal_material.set_uniform("character_normal", character_normal);
        for mesh in &character.meshes {
            draw_mesh(mesh);
        }
        gl_use_default_material();

        let post_camera = Camera2D {
            render_target: Some(output_target.clone()),
            ..Camera2D::from_display_rect(Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32))
        };
        set_camera(&post_camera);
        clear_background(BLACK);
        gl_use_material(&material);
        material.set_uniform("time", t);
        material.set_uniform("resolution", vec2(WIDTH as f32, HEIGHT as f32));
        draw_texture_ex(
            &scene_target.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(WIDTH as f32, HEIGHT as f32)),
                flip_y: true,
                ..Default::default()
            },
        );
        gl_use_default_material();
        notifications::draw(&feed, t, &character.spec.name);

        set_default_camera();
        clear_background(BLACK);
        draw_texture_ex(
            &output_target.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height() - 32.0)),
                flip_y: true,
                ..Default::default()
            },
        );
        draw_text(
            &format!(
                "BRONZECAST // LIVE ENCODING // FRAME {frame:06} // {} FPS",
                get_fps()
            ),
            16.0,
            screen_height() - 10.0,
            13.0,
            Color::from_rgba(130, 150, 180, 255),
        );
        next_frame().await;
        if t >= next_encode_at {
            let image = output_target.texture.get_texture_data();
            if let Err(error) = input.write_all(&image.bytes) {
                eprintln!("ffmpeg stream ended: {error}");
                break;
            }
            next_encode_at += 1.0 / STREAM_FPS;
            frame += 1;
        }
    }
    drop(input);
    let _ = encoder.kill();
}
