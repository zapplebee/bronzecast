use crate::stickers::{Appearance, Kind, StickerView};
use chrono::Local;
use macroquad::prelude::*;
use std::collections::{HashMap, HashSet};

const STICKER_SIZE: (f32, f32) = (260.0, 140.0);
const LAYER_WIDTH: u32 = 280;
const LAYER_HEIGHT: u32 = 220;
const PAD: f32 = 10.0;
const SAFE_HALF_WIDTH: f32 = 92.0;
const SAFE_HALF_HEIGHT: f32 = 38.0;

fn ink() -> Color {
    Color::from_rgba(42, 36, 28, 255)
}

fn pale_ink() -> Color {
    Color::from_rgba(255, 248, 225, 255)
}

fn green() -> Color {
    Color::from_rgba(72, 139, 64, 255)
}

fn shadow() -> Color {
    Color::from_rgba(20, 18, 14, 75)
}

fn paper() -> Color {
    Color::from_rgba(255, 253, 246, 255)
}

fn oval(cx: f32, cy: f32, rx: f32, ry: f32, fill: Color) {
    draw_ellipse(cx + 5.0, cy + 7.0, rx + 7.0, ry + 7.0, 0.0, shadow());
    draw_ellipse(cx, cy, rx + 6.0, ry + 6.0, 0.0, paper());
    draw_ellipse(cx, cy, rx, ry, 0.0, fill);
}

fn rounded_body(cx: f32, cy: f32, half_w: f32, half_h: f32, radius: f32, fill: Color) {
    let x = cx - half_w;
    let y = cy - half_h;
    let width = half_w * 2.0;
    let height = half_h * 2.0;
    draw_rectangle(
        x + radius + 5.0,
        y + 7.0,
        width - 2.0 * radius,
        height,
        shadow(),
    );
    draw_circle(x + radius + 5.0, cy + 7.0, radius, shadow());
    draw_circle(x + width - radius + 5.0, cy + 7.0, radius, shadow());
    draw_rectangle(x + radius, y, width - 2.0 * radius, height, paper());
    draw_circle(x + radius, cy, radius, paper());
    draw_circle(x + width - radius, cy, radius, paper());
    draw_rectangle(
        x + radius + 5.0,
        y + 5.0,
        width - 2.0 * radius - 10.0,
        height - 10.0,
        fill,
    );
    draw_circle(x + radius + 5.0, cy, radius - 5.0, fill);
    draw_circle(x + width - radius - 5.0, cy, radius - 5.0, fill);
}

fn tri(a: Vec2, b: Vec2, c: Vec2, color: Color) {
    draw_triangle(a, b, c, color);
}

fn sticker_triangle(a: Vec2, b: Vec2, c: Vec2, fill: Color) {
    let center = (a + b + c) / 3.0;
    let expand = |point: Vec2| center + (point - center) * 1.08;
    let (outer_a, outer_b, outer_c) = (expand(a), expand(b), expand(c));
    let offset = vec2(4.0, 6.0);
    tri(
        outer_a + offset,
        outer_b + offset,
        outer_c + offset,
        shadow(),
    );
    tri(outer_a, outer_b, outer_c, paper());
    tri(a, b, c, fill);
}

fn edge_dot(x: f32, y: f32, radius: f32, color: Color) {
    draw_circle(x, y, radius + 2.0, paper());
    draw_circle(x, y, radius, color);
}

fn edge_line(x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color) {
    draw_line(x1, y1, x2, y2, width + 3.0, paper());
    draw_line(x1, y1, x2, y2, width, color);
}

fn perimeter_rays(cx: f32, cy: f32, count: u8, width: f32, color: Color) {
    for index in 0..count {
        let angle = index as f32 * std::f32::consts::TAU / count as f32;
        let dx = angle.cos();
        let dy = angle.sin();
        let safe_edge = (SAFE_HALF_WIDTH / dx.abs()).min(SAFE_HALF_HEIGHT / dy.abs());
        let sticker_edge =
            1.0 / ((dx * dx / 112.0_f32.powi(2)) + (dy * dy / 56.0_f32.powi(2))).sqrt();
        let inner = vec2(cx + dx * (safe_edge + 5.0), cy + dy * (safe_edge + 5.0));
        let outer = vec2(cx + dx * sticker_edge, cy + dy * sticker_edge);
        edge_line(inner.x, inner.y, outer.x, outer.y, width, color);
    }
}

fn draw_appearance(appearance: Appearance, cx: f32, cy: f32) {
    use Appearance::*;
    let fill = match appearance {
        JackOLantern | PumpkinPie => Color::from_rgba(244, 151, 61, 255),
        Ghost => Color::from_rgba(240, 239, 220, 255),
        BlackCat => Color::from_rgba(85, 82, 112, 255),
        Bat => Color::from_rgba(105, 116, 168, 255),
        CandyPail => Color::from_rgba(255, 174, 66, 255),
        Turkey => Color::from_rgba(190, 119, 73, 255),
        Cornucopia => Color::from_rgba(230, 187, 97, 255),
        MapleLeaf => Color::from_rgba(218, 111, 58, 255),
        ChristmasTree => Color::from_rgba(102, 160, 103, 255),
        WrappedGift => Color::from_rgba(225, 99, 105, 255),
        Snowman => Color::from_rgba(213, 232, 239, 255),
        Reindeer => Color::from_rgba(196, 143, 97, 255),
        GingerbreadPerson => Color::from_rgba(199, 139, 91, 255),
        CandyCane => Color::from_rgba(246, 226, 210, 255),
        Fireworks => Color::from_rgba(111, 130, 181, 255),
        LunarNewYearLantern => Color::from_rgba(226, 91, 77, 255),
        Diya => Color::from_rgba(237, 177, 84, 255),
        Heart => Color::from_rgba(235, 128, 148, 255),
        EasterBunny => Color::from_rgba(236, 196, 194, 255),
        DecoratedEgg => Color::from_rgba(163, 191, 224, 255),
        Tulip => Color::from_rgba(232, 144, 166, 255),
        Sun => Color::from_rgba(248, 211, 93, 255),
        IceCreamCone => Color::from_rgba(240, 190, 133, 255),
        CustomSvg => Color::from_rgba(164, 164, 164, 255),
    };

    match appearance {
        WrappedGift => rounded_body(cx, cy, 112.0, 52.0, 10.0, fill),
        CandyPail => rounded_body(cx, cy, 112.0, 52.0, 28.0, fill),
        LunarNewYearLantern => rounded_body(cx, cy, 92.0, 54.0, 44.0, fill),
        Heart => {
            oval(cx - 35.0, cy - 13.0, 58.0, 47.0, fill);
            oval(cx + 35.0, cy - 13.0, 58.0, 47.0, fill);
            tri(
                vec2(cx - 87.0, cy - 2.0),
                vec2(cx + 87.0, cy - 2.0),
                vec2(cx, cy + 68.0),
                fill,
            );
        }
        ChristmasTree => {
            oval(
                cx,
                cy + 2.0,
                112.0,
                58.0,
                Color::from_rgba(111, 165, 105, 255),
            );
            for (dx, dy) in [(-102.0, -8.0), (-105.0, 30.0), (105.0, 30.0), (102.0, -8.0)] {
                edge_dot(cx + dx, cy + dy, 5.0, Color::from_rgba(228, 113, 89, 255));
            }
            tri(
                vec2(cx, cy - 66.0),
                vec2(cx - 18.0, cy - 42.0),
                vec2(cx + 18.0, cy - 42.0),
                Color::from_rgba(255, 216, 107, 255),
            );
        }
        IceCreamCone => {
            oval(
                cx,
                cy - 10.0,
                104.0,
                54.0,
                Color::from_rgba(249, 211, 151, 255),
            );
            tri(
                vec2(cx - 58.0, cy + 44.0),
                vec2(cx + 58.0, cy + 44.0),
                vec2(cx, cy + 70.0),
                Color::from_rgba(209, 157, 105, 255),
            );
            edge_line(
                cx - 35.0,
                cy + 49.0,
                cx + 24.0,
                cy + 68.0,
                2.0,
                Color::from_rgba(156, 106, 73, 255),
            );
            edge_line(
                cx + 35.0,
                cy + 49.0,
                cx - 24.0,
                cy + 68.0,
                2.0,
                Color::from_rgba(156, 106, 73, 255),
            );
        }
        _ => oval(cx, cy, 112.0, 56.0, fill),
    }

    match appearance {
        JackOLantern => {
            for dx in [-106.0, -98.0, 98.0, 106.0] {
                edge_line(
                    cx + dx,
                    cy - 49.0,
                    cx + dx * 0.92,
                    cy + 48.0,
                    2.0,
                    Color::from_rgba(193, 91, 38, 255),
                );
            }
            edge_line(
                cx - 98.0,
                cy - 28.0,
                cx - 101.0,
                cy + 28.0,
                4.0,
                Color::from_rgba(193, 91, 38, 255),
            );
            edge_line(
                cx + 98.0,
                cy - 28.0,
                cx + 101.0,
                cy + 28.0,
                4.0,
                Color::from_rgba(193, 91, 38, 255),
            );
            edge_line(
                cx,
                cy - 52.0,
                cx + 4.0,
                cy - 66.0,
                5.0,
                Color::from_rgba(112, 89, 51, 255),
            );
            oval(cx + 17.0, cy - 61.0, 18.0, 7.0, green());
        }
        Ghost => {
            for dx in [-77.0, -26.0, 26.0, 77.0] {
                edge_dot(cx + dx, cy + 52.0, 7.0, fill);
            }
            edge_dot(
                cx - 108.0,
                cy - 4.0,
                7.0,
                Color::from_rgba(211, 222, 226, 255),
            );
            edge_dot(
                cx + 108.0,
                cy - 4.0,
                7.0,
                Color::from_rgba(211, 222, 226, 255),
            );
        }
        BlackCat => {
            sticker_triangle(
                vec2(cx - 91.0, cy - 42.0),
                vec2(cx - 66.0, cy - 78.0),
                vec2(cx - 40.0, cy - 42.0),
                Color::from_rgba(67, 64, 91, 255),
            );
            sticker_triangle(
                vec2(cx + 40.0, cy - 42.0),
                vec2(cx + 66.0, cy - 78.0),
                vec2(cx + 91.0, cy - 42.0),
                Color::from_rgba(67, 64, 91, 255),
            );
            edge_line(
                cx - 96.0,
                cy + 24.0,
                cx - 116.0,
                cy + 9.0,
                5.0,
                Color::from_rgba(67, 64, 91, 255),
            );
            edge_line(
                cx - 116.0,
                cy + 9.0,
                cx - 112.0,
                cy - 10.0,
                5.0,
                Color::from_rgba(67, 64, 91, 255),
            );
            edge_dot(
                cx - 106.0,
                cy - 1.0,
                3.0,
                Color::from_rgba(255, 222, 112, 255),
            );
            edge_dot(
                cx + 106.0,
                cy - 1.0,
                3.0,
                Color::from_rgba(255, 222, 112, 255),
            );
        }
        Bat => {
            sticker_triangle(
                vec2(cx - 103.0, cy),
                vec2(cx - 128.0, cy - 46.0),
                vec2(cx - 108.0, cy + 42.0),
                Color::from_rgba(72, 81, 126, 255),
            );
            sticker_triangle(
                vec2(cx + 103.0, cy),
                vec2(cx + 128.0, cy - 46.0),
                vec2(cx + 108.0, cy + 42.0),
                Color::from_rgba(72, 81, 126, 255),
            );
            edge_dot(
                cx - 96.0,
                cy + 8.0,
                4.0,
                Color::from_rgba(255, 229, 138, 255),
            );
            edge_dot(
                cx + 96.0,
                cy + 8.0,
                4.0,
                Color::from_rgba(255, 229, 138, 255),
            );
        }
        CandyPail => {
            draw_arc(cx, cy - 48.0, 24, 43.0, 180.0, 8.0, 180.0, paper());
            draw_arc(
                cx,
                cy - 48.0,
                24,
                43.0,
                180.0,
                4.0,
                180.0,
                Color::from_rgba(255, 238, 166, 255),
            );
            edge_line(
                cx - 88.0,
                cy - 45.0,
                cx + 88.0,
                cy - 45.0,
                4.0,
                Color::from_rgba(255, 238, 166, 255),
            );
            edge_dot(
                cx - 104.0,
                cy + 44.0,
                6.0,
                Color::from_rgba(255, 238, 166, 255),
            );
            edge_dot(
                cx + 104.0,
                cy + 44.0,
                6.0,
                Color::from_rgba(189, 112, 205, 255),
            );
        }
        Turkey => {
            for (i, color) in [
                Color::from_rgba(217, 108, 72, 255),
                Color::from_rgba(241, 174, 78, 255),
                Color::from_rgba(227, 197, 109, 255),
                Color::from_rgba(168, 188, 105, 255),
                Color::from_rgba(202, 127, 88, 255),
            ]
            .into_iter()
            .enumerate()
            {
                edge_dot(cx - 88.0 + i as f32 * 44.0, cy - 57.0, 12.0, color);
            }
            edge_dot(
                cx - 106.0,
                cy + 6.0,
                7.0,
                Color::from_rgba(218, 85, 72, 255),
            );
            edge_dot(cx + 105.0, cy - 17.0, 11.0, fill);
            sticker_triangle(
                vec2(cx + 111.0, cy - 24.0),
                vec2(cx + 128.0, cy - 18.0),
                vec2(cx + 111.0, cy - 12.0),
                Color::from_rgba(239, 175, 78, 255),
            );
            for dx in [-35.0, 35.0] {
                edge_line(
                    cx + dx,
                    cy + 53.0,
                    cx + dx,
                    cy + 67.0,
                    3.0,
                    Color::from_rgba(149, 91, 60, 255),
                );
            }
        }
        Cornucopia => {
            tri(
                vec2(cx - 108.0, cy - 40.0),
                vec2(cx - 128.0, cy),
                vec2(cx - 108.0, cy + 40.0),
                Color::from_rgba(195, 140, 66, 255),
            );
            for (dx, color) in [
                (-105.0, Color::from_rgba(227, 111, 72, 255)),
                (-78.0, Color::from_rgba(137, 179, 96, 255)),
                (105.0, Color::from_rgba(225, 116, 77, 255)),
            ] {
                edge_dot(cx + dx, cy - 55.0, 8.0, color);
            }
        }
        MapleLeaf => {
            let leaf = Color::from_rgba(207, 91, 50, 255);
            sticker_triangle(
                vec2(cx - 47.0, cy - 47.0),
                vec2(cx, cy - 79.0),
                vec2(cx + 47.0, cy - 47.0),
                leaf,
            );
            sticker_triangle(
                vec2(cx - 99.0, cy - 32.0),
                vec2(cx - 127.0, cy - 13.0),
                vec2(cx - 99.0, cy + 6.0),
                leaf,
            );
            sticker_triangle(
                vec2(cx + 99.0, cy - 32.0),
                vec2(cx + 127.0, cy - 13.0),
                vec2(cx + 99.0, cy + 6.0),
                leaf,
            );
            sticker_triangle(
                vec2(cx - 92.0, cy + 10.0),
                vec2(cx - 111.0, cy + 43.0),
                vec2(cx - 54.0, cy + 43.0),
                leaf,
            );
            sticker_triangle(
                vec2(cx + 92.0, cy + 10.0),
                vec2(cx + 111.0, cy + 43.0),
                vec2(cx + 54.0, cy + 43.0),
                leaf,
            );
            sticker_triangle(
                vec2(cx - 49.0, cy + 44.0),
                vec2(cx - 16.0, cy + 70.0),
                vec2(cx + 2.0, cy + 44.0),
                leaf,
            );
            sticker_triangle(
                vec2(cx + 49.0, cy + 44.0),
                vec2(cx + 16.0, cy + 70.0),
                vec2(cx - 2.0, cy + 44.0),
                leaf,
            );
            edge_line(
                cx - 101.0,
                cy + 46.0,
                cx - 116.0,
                cy + 63.0,
                4.0,
                Color::from_rgba(147, 88, 51, 255),
            );
        }
        PumpkinPie => {
            edge_line(
                cx - 72.0,
                cy - 48.0,
                cx + 72.0,
                cy - 48.0,
                3.0,
                Color::from_rgba(255, 227, 171, 255),
            );
            for dx in [-100.0, -70.0, 70.0, 100.0] {
                edge_dot(
                    cx + dx,
                    cy + 48.0,
                    4.0,
                    Color::from_rgba(250, 222, 169, 255),
                );
            }
        }
        ChristmasTree => {
            sticker_triangle(
                vec2(cx, cy - 74.0),
                vec2(cx - 24.0, cy - 47.0),
                vec2(cx + 24.0, cy - 47.0),
                Color::from_rgba(75, 143, 91, 255),
            );
            sticker_triangle(
                vec2(cx - 101.0, cy - 32.0),
                vec2(cx - 128.0, cy - 5.0),
                vec2(cx - 101.0, cy + 22.0),
                Color::from_rgba(75, 143, 91, 255),
            );
            sticker_triangle(
                vec2(cx + 101.0, cy - 32.0),
                vec2(cx + 128.0, cy - 5.0),
                vec2(cx + 101.0, cy + 22.0),
                Color::from_rgba(75, 143, 91, 255),
            );
            for (dx, dy, color) in [
                (-105.0, -20.0, Color::from_rgba(238, 104, 94, 255)),
                (105.0, -20.0, Color::from_rgba(255, 211, 100, 255)),
                (-105.0, 39.0, Color::from_rgba(255, 211, 100, 255)),
                (105.0, 39.0, Color::from_rgba(238, 104, 94, 255)),
            ] {
                edge_dot(cx + dx, cy + dy, 5.0, color);
            }
            edge_dot(cx, cy - 66.0, 7.0, Color::from_rgba(255, 221, 113, 255));
        }
        WrappedGift => {
            draw_rectangle(cx - 111.0, cy - 46.0, 8.0, 92.0, paper());
            draw_rectangle(
                cx - 109.0,
                cy - 44.0,
                4.0,
                88.0,
                Color::from_rgba(255, 226, 142, 255),
            );
            draw_rectangle(cx + 103.0, cy - 46.0, 8.0, 92.0, paper());
            draw_rectangle(
                cx + 105.0,
                cy - 44.0,
                4.0,
                88.0,
                Color::from_rgba(255, 226, 142, 255),
            );
            let ribbon = Color::from_rgba(255, 226, 142, 255);
            for (dx, rotation) in [(-16.0, -18.0), (16.0, 18.0)] {
                draw_ellipse(
                    cx + dx + 4.0,
                    cy - 55.0 + 6.0,
                    18.0,
                    9.0,
                    rotation,
                    shadow(),
                );
                draw_ellipse(cx + dx, cy - 55.0, 18.0, 9.0, rotation, paper());
                draw_ellipse(cx + dx, cy - 55.0, 13.0, 5.0, rotation, ribbon);
            }
            edge_dot(cx, cy - 52.0, 6.0, Color::from_rgba(246, 191, 74, 255));
        }
        Snowman => {
            oval(
                cx,
                cy - 58.0,
                25.0,
                16.0,
                Color::from_rgba(228, 239, 241, 255),
            );
            rounded_body(
                cx,
                cy - 77.0,
                31.0,
                6.0,
                5.0,
                Color::from_rgba(65, 76, 91, 255),
            );
            edge_dot(cx - 9.0, cy - 60.0, 2.5, ink());
            edge_dot(cx + 9.0, cy - 60.0, 2.5, ink());
            edge_dot(
                cx - 102.0,
                cy + 49.0,
                6.0,
                Color::from_rgba(99, 149, 196, 255),
            );
            edge_dot(
                cx + 102.0,
                cy + 49.0,
                6.0,
                Color::from_rgba(99, 149, 196, 255),
            );
            edge_dot(
                cx - 105.0,
                cy + 10.0,
                4.0,
                Color::from_rgba(99, 149, 196, 255),
            );
            edge_dot(
                cx + 105.0,
                cy + 10.0,
                4.0,
                Color::from_rgba(99, 149, 196, 255),
            );
        }
        Reindeer => {
            edge_line(
                cx - 101.0,
                cy - 45.0,
                cx - 111.0,
                cy - 68.0,
                5.0,
                Color::from_rgba(117, 79, 51, 255),
            );
            edge_line(
                cx - 111.0,
                cy - 68.0,
                cx - 122.0,
                cy - 53.0,
                5.0,
                Color::from_rgba(117, 79, 51, 255),
            );
            edge_line(
                cx + 101.0,
                cy - 45.0,
                cx + 111.0,
                cy - 68.0,
                5.0,
                Color::from_rgba(117, 79, 51, 255),
            );
            edge_line(
                cx + 111.0,
                cy - 68.0,
                cx + 122.0,
                cy - 53.0,
                5.0,
                Color::from_rgba(117, 79, 51, 255),
            );
            edge_dot(
                cx - 105.0,
                cy + 17.0,
                6.0,
                Color::from_rgba(213, 76, 70, 255),
            );
        }
        GingerbreadPerson => {
            edge_line(
                cx - 100.0,
                cy - 4.0,
                cx - 127.0,
                cy - 17.0,
                14.0,
                Color::from_rgba(184, 119, 75, 255),
            );
            edge_line(
                cx + 100.0,
                cy - 4.0,
                cx + 127.0,
                cy - 17.0,
                14.0,
                Color::from_rgba(184, 119, 75, 255),
            );
            edge_line(
                cx - 48.0,
                cy + 45.0,
                cx - 66.0,
                cy + 66.0,
                14.0,
                Color::from_rgba(184, 119, 75, 255),
            );
            edge_line(
                cx + 48.0,
                cy + 45.0,
                cx + 66.0,
                cy + 66.0,
                14.0,
                Color::from_rgba(184, 119, 75, 255),
            );
            for dx in [-104.0, 104.0] {
                edge_dot(
                    cx + dx,
                    cy + 11.0,
                    9.0,
                    Color::from_rgba(232, 221, 200, 255),
                );
                edge_dot(
                    cx + dx * 0.95,
                    cy + 50.0,
                    7.0,
                    Color::from_rgba(232, 221, 200, 255),
                );
            }
            for dx in [-104.0, 104.0] {
                edge_dot(
                    cx + dx,
                    cy - 45.0,
                    5.0,
                    Color::from_rgba(232, 221, 200, 255),
                );
            }
        }
        CandyCane => {
            draw_arc(cx - 79.0, cy - 44.0, 24, 34.0, 180.0, 11.0, 180.0, paper());
            draw_arc(
                cx - 79.0,
                cy - 44.0,
                24,
                34.0,
                180.0,
                7.0,
                180.0,
                Color::from_rgba(212, 81, 78, 255),
            );
            edge_line(cx - 113.0, cy - 44.0, cx - 113.0, cy + 54.0, 11.0, paper());
            edge_line(
                cx - 113.0,
                cy - 44.0,
                cx - 113.0,
                cy + 54.0,
                7.0,
                Color::from_rgba(212, 81, 78, 255),
            );
            for (x1, y1, x2, y2) in [
                (cx - 101.0, cy - 45.0, cx - 112.0, cy - 56.0),
                (cx - 101.0, cy + 45.0, cx - 112.0, cy + 56.0),
                (cx + 101.0, cy - 45.0, cx + 112.0, cy - 56.0),
                (cx + 101.0, cy + 45.0, cx + 112.0, cy + 56.0),
            ] {
                edge_line(x1, y1, x2, y2, 6.0, Color::from_rgba(212, 81, 78, 255));
            }
            edge_dot(
                cx - 105.0,
                cy + 34.0,
                7.0,
                Color::from_rgba(213, 87, 79, 255),
            );
        }
        Fireworks => {
            perimeter_rays(cx, cy, 12, 2.0, Color::from_rgba(255, 204, 105, 255));
            for dx in [-73.0, 73.0] {
                edge_dot(
                    cx + dx,
                    cy + 48.0,
                    4.0,
                    Color::from_rgba(255, 238, 169, 255),
                );
            }
        }
        LunarNewYearLantern => {
            edge_line(
                cx - 71.0,
                cy - 48.0,
                cx + 71.0,
                cy - 48.0,
                3.0,
                Color::from_rgba(255, 211, 119, 255),
            );
            edge_line(
                cx - 71.0,
                cy + 48.0,
                cx + 71.0,
                cy + 48.0,
                3.0,
                Color::from_rgba(255, 211, 119, 255),
            );
            edge_line(
                cx,
                cy + 55.0,
                cx,
                cy + 69.0,
                4.0,
                Color::from_rgba(255, 211, 119, 255),
            );
        }
        Diya => {
            tri(
                vec2(cx, cy - 67.0),
                vec2(cx - 18.0, cy - 22.0),
                vec2(cx + 18.0, cy - 22.0),
                Color::from_rgba(255, 205, 95, 255),
            );
            edge_line(
                cx - 84.0,
                cy + 48.0,
                cx + 84.0,
                cy + 48.0,
                5.0,
                Color::from_rgba(222, 144, 67, 255),
            );
            edge_line(
                cx - 84.0,
                cy + 48.0,
                cx - 98.0,
                cy + 36.0,
                4.0,
                Color::from_rgba(222, 144, 67, 255),
            );
            edge_line(
                cx + 84.0,
                cy + 48.0,
                cx + 98.0,
                cy + 36.0,
                4.0,
                Color::from_rgba(222, 144, 67, 255),
            );
            edge_dot(
                cx - 104.0,
                cy + 31.0,
                4.0,
                Color::from_rgba(255, 232, 160, 255),
            );
            edge_dot(
                cx + 104.0,
                cy + 31.0,
                4.0,
                Color::from_rgba(255, 232, 160, 255),
            );
        }
        EasterBunny => {
            rounded_body(
                cx - 104.0,
                cy - 42.0,
                7.0,
                28.0,
                7.0,
                Color::from_rgba(240, 212, 211, 255),
            );
            rounded_body(
                cx + 104.0,
                cy - 42.0,
                7.0,
                28.0,
                7.0,
                Color::from_rgba(240, 212, 211, 255),
            );
            edge_dot(cx - 108.0, cy + 43.0, 7.0, paper());
        }
        DecoratedEgg => {
            edge_line(
                cx - 91.0,
                cy - 48.0,
                cx + 91.0,
                cy - 48.0,
                3.0,
                Color::from_rgba(245, 221, 156, 255),
            );
            edge_line(
                cx - 91.0,
                cy + 48.0,
                cx + 91.0,
                cy + 48.0,
                3.0,
                Color::from_rgba(245, 221, 156, 255),
            );
            for dx in [-75.0, -40.0, 40.0, 75.0] {
                edge_dot(
                    cx + dx,
                    cy - 48.0,
                    4.0,
                    Color::from_rgba(230, 128, 149, 255),
                );
                edge_dot(
                    cx + dx,
                    cy + 48.0,
                    4.0,
                    Color::from_rgba(230, 128, 149, 255),
                );
            }
        }
        Tulip => {
            edge_dot(
                cx - 45.0,
                cy - 59.0,
                14.0,
                Color::from_rgba(237, 121, 154, 255),
            );
            edge_dot(cx, cy - 61.0, 14.0, Color::from_rgba(237, 121, 154, 255));
            edge_dot(
                cx + 45.0,
                cy - 59.0,
                14.0,
                Color::from_rgba(237, 121, 154, 255),
            );
            edge_line(cx, cy + 47.0, cx, cy + 67.0, 5.0, green());
            oval(cx - 24.0, cy + 54.0, 20.0, 6.0, green());
            oval(cx + 24.0, cy + 54.0, 20.0, 6.0, green());
        }
        Sun => {
            perimeter_rays(cx, cy, 12, 4.0, Color::from_rgba(237, 185, 75, 255));
        }
        _ => {}
    }

    // Contrast details stay around the border so the shared text-safe center is clear.
    if matches!(appearance, BlackCat | Bat | Fireworks | ChristmasTree) {
        edge_dot(
            cx - 103.0,
            cy - 5.0,
            4.0,
            Color::from_rgba(255, 232, 159, 255),
        );
        edge_dot(
            cx + 103.0,
            cy - 5.0,
            4.0,
            Color::from_rgba(255, 232, 159, 255),
        );
    }
}

fn text_color(appearance: Appearance) -> Color {
    match appearance {
        Appearance::BlackCat
        | Appearance::Bat
        | Appearance::Fireworks
        | Appearance::ChristmasTree => pale_ink(),
        _ => ink(),
    }
}

fn draw_behavior(view: &StickerView, cx: f32, cy: f32, x: f32, y: f32) {
    let color = text_color(view.appearance);
    match view.kind {
        Kind::Timer => {
            let state = match view.state {
                "running" => "FOCUS",
                "paused" => "PAUSED",
                "done" => "DONE",
                _ => "READY",
            };
            let remaining = view.remaining_seconds.unwrap_or(0);
            draw_centered(state, cx, cy - 6.0, 10.0, color);
            draw_centered(
                &format!("{:02}:{:02}", remaining / 60, remaining % 60),
                cx,
                cy + 21.0,
                25.0,
                color,
            );
        }
        Kind::Countdown => {
            let days = view.days.unwrap_or(0);
            let label = if days > 0 {
                format!("{days} {}", if days == 1 { "DAY" } else { "DAYS" })
            } else if days == 0 {
                "TODAY".to_owned()
            } else {
                format!(
                    "{} {} PAST",
                    days.unsigned_abs(),
                    if days == -1 { "DAY" } else { "DAYS" }
                )
            };
            draw_centered(&label, cx, cy - 4.0, 19.0, color);
            let text = view.text.as_deref().unwrap_or("").to_uppercase();
            draw_lines_centered(
                &wrap(&text, SAFE_HALF_WIDTH * 2.0, 11, 1),
                cx,
                cy + 22.0,
                11.0,
                13.0,
                color,
            );
        }
        Kind::Todo => {
            let check_x = x + 75.0;
            let check_y = y + 110.0;
            draw_circle(check_x, check_y, 10.0, paper());
            draw_circle_lines(check_x, check_y, 8.0, 1.5, ink());
            if view.done.unwrap_or(false) {
                draw_line(
                    check_x - 4.0,
                    check_y,
                    check_x - 1.0,
                    check_y + 3.0,
                    2.0,
                    green(),
                );
                draw_line(
                    check_x - 1.0,
                    check_y + 3.0,
                    check_x + 5.0,
                    check_y - 5.0,
                    2.0,
                    green(),
                );
            }
            let lines = wrap(
                view.text.as_deref().unwrap_or(""),
                SAFE_HALF_WIDTH * 2.0,
                16,
                3,
            );
            let first = centered_baseline(cy, lines.len(), 18.0, 6.0);
            draw_lines_centered(&lines, cx, first, 16.0, 18.0, color);
        }
        Kind::Memo => {
            let title = view.title.as_deref().unwrap_or("");
            let body = view.text.as_deref().unwrap_or("");
            if !title.is_empty() && !body.is_empty() {
                draw_centered(title, cx, cy - 5.0, 16.0, color);
                let lines = wrap(body, SAFE_HALF_WIDTH * 2.0, 13, 2);
                draw_lines_centered(
                    &lines,
                    cx,
                    centered_baseline(cy, lines.len(), 15.0, 19.0),
                    13.0,
                    15.0,
                    color,
                );
            } else {
                let text = if title.is_empty() { body } else { title };
                let lines = wrap(text, SAFE_HALF_WIDTH * 2.0, 17, 3);
                let first = centered_baseline(cy, lines.len(), 19.0, 6.0);
                draw_lines_centered(&lines, cx, first, 17.0, 19.0, color);
            }
        }
        Kind::Notification => {
            let title = view
                .title
                .as_deref()
                .unwrap_or("Notification")
                .to_uppercase();
            let title_lines = wrap(&title, SAFE_HALF_WIDTH * 2.0, 13, 1);
            draw_lines_centered(&title_lines, cx, cy - 13.0, 13.0, 15.0, green());
            let message = view.text.as_deref().unwrap_or("").to_uppercase();
            let lines = wrap(&message, SAFE_HALF_WIDTH * 2.0, 12, 3);
            draw_lines_centered(
                &lines,
                cx,
                centered_baseline(cy, lines.len(), 14.0, 20.0),
                12.0,
                14.0,
                color,
            );
        }
        Kind::Clock => {
            let now = Local::now();
            let time = now.format("%-I:%M").to_string();
            let suffix = now.format("%p").to_string();
            let date = now.format("%a, %b %-d").to_string().to_uppercase();
            let time_width = measure_text(&time, None, 27, 1.0).width;
            let suffix_width = measure_text(&suffix, None, 11, 1.0).width;
            let group_width = time_width + 5.0 + suffix_width;
            let start_x = cx - group_width / 2.0;
            draw_text(&time, start_x, cy + 4.0, 27.0, color);
            draw_text(&suffix, start_x + time_width + 5.0, cy + 3.0, 11.0, green());
            draw_centered(&date, cx, cy + 26.0, 11.0, color);
        }
        Kind::Art => {}
    }
}

fn prepare_font_atlas(views: &[StickerView]) {
    let mut glyphs: Vec<char> = (32u8..=126).map(char::from).collect();
    for view in views {
        glyphs.extend(view.title.as_deref().unwrap_or_default().chars());
        glyphs.extend(view.text.as_deref().unwrap_or_default().chars());
    }
    glyphs.sort_unstable();
    glyphs.dedup();

    let font = get_default_font();
    let dpi = screen_dpi_scale();
    for logical_size in [10, 11, 12, 13, 14, 15, 16, 17, 19, 25, 26, 27] {
        let raster_size = (logical_size as f32 * dpi).ceil() as u16;
        font.populate_font_cache(&glyphs, raster_size);
    }
}

pub struct StickerLayer {
    target: RenderTarget,
    svg_textures: HashMap<u64, CachedSvg>,
}

struct CachedSvg {
    source: String,
    texture: Option<Texture2D>,
}

impl StickerLayer {
    pub fn new() -> Self {
        let target = render_target(LAYER_WIDTH, LAYER_HEIGHT);
        target.texture.set_filter(FilterMode::Nearest);
        Self {
            target,
            svg_textures: HashMap::new(),
        }
    }

    fn svg_texture(&mut self, id: u64, source: &str) -> Option<Texture2D> {
        if self
            .svg_textures
            .get(&id)
            .is_some_and(|cached| cached.source == source)
        {
            return self
                .svg_textures
                .get(&id)
                .and_then(|cached| cached.texture.clone());
        }

        let texture = match crate::svg_art::rasterize(source) {
            Ok(rgba) => {
                let texture = Texture2D::from_rgba8(
                    crate::svg_art::WIDTH as u16,
                    crate::svg_art::HEIGHT as u16,
                    &rgba,
                );
                texture.set_filter(FilterMode::Nearest);
                Some(texture)
            }
            Err(error) => {
                eprintln!("could not render SVG for sticker {id}: {error}");
                None
            }
        };
        let result = texture.clone();
        self.svg_textures.insert(
            id,
            CachedSvg {
                source: source.to_owned(),
                texture,
            },
        );
        result
    }

    pub fn draw(&mut self, views: &[StickerView]) {
        prepare_font_atlas(views);
        let active_svg_ids: HashSet<_> = views
            .iter()
            .filter(|view| view.appearance == Appearance::CustomSvg)
            .map(|view| view.id)
            .collect();
        self.svg_textures
            .retain(|id, _| active_svg_ids.contains(id));
        for view in views {
            let (width, height) = size(view.appearance);
            let custom_texture = if view.appearance == Appearance::CustomSvg {
                view.svg
                    .as_deref()
                    .and_then(|source| self.svg_texture(view.id, source))
            } else {
                None
            };
            push_camera_state();
            let camera = Camera2D {
                render_target: Some(self.target.clone()),
                ..Camera2D::from_display_rect(Rect::new(
                    0.0,
                    0.0,
                    LAYER_WIDTH as f32,
                    LAYER_HEIGHT as f32,
                ))
            };
            set_camera(&camera);
            clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
            let x = PAD;
            let y = PAD;
            let cx = x + width / 2.0;
            let cy = y + height / 2.0;
            if let Some(texture) = custom_texture {
                draw_texture_ex(
                    &texture,
                    x,
                    y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(width, height)),
                        ..Default::default()
                    },
                );
            } else {
                // Custom SVG failures fall back to a neutral sticker silhouette;
                // preset appearances use their normal motif renderer.
                draw_appearance(view.appearance, cx, cy);
            }
            draw_behavior(view, cx, cy, x, y);
            pop_camera_state();

            draw_texture_ex(
                &self.target.texture,
                view.x - PAD,
                view.y - PAD,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(LAYER_WIDTH as f32, LAYER_HEIGHT as f32)),
                    flip_y: true,
                    rotation: view.angle.to_radians(),
                    pivot: Some(vec2(view.x + width / 2.0, view.y + height / 2.0)),
                    ..Default::default()
                },
            );
        }
    }
}

fn size(_appearance: Appearance) -> (f32, f32) {
    STICKER_SIZE
}

fn draw_centered(text: &str, cx: f32, baseline: f32, size: f32, color: Color) {
    let width = measure_text(text, None, size as u16, 1.0).width;
    draw_text(text, cx - width / 2.0, baseline, size, color);
}

fn draw_lines_centered(
    lines: &[String],
    cx: f32,
    baseline: f32,
    size: f32,
    line_height: f32,
    color: Color,
) {
    for (index, line) in lines.iter().enumerate() {
        draw_centered(line, cx, baseline + index as f32 * line_height, size, color);
    }
}

fn centered_baseline(cy: f32, line_count: usize, line_height: f32, baseline_offset: f32) -> f32 {
    let block_height =
        (line_count.saturating_sub(1) as f32 * line_height).min(SAFE_HALF_HEIGHT * 2.0);
    cy - block_height / 2.0 + baseline_offset
}

fn wrap(text: &str, width: f32, size: u16, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_owned()
            } else {
                format!("{current} {word}")
            };
            if measure_text(&candidate, None, size, 1.0).width <= width {
                current = candidate;
            } else {
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
                current = word.to_owned();
            }
        }
        if !current.is_empty() || paragraph.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.last_mut().expect("at least one line");
        while !last.is_empty() && measure_text(&format!("{last}..."), None, size, 1.0).width > width
        {
            last.pop();
        }
        last.push_str("...");
    }
    lines
}
