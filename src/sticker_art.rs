use macroquad::prelude::*;

const TIMER_SIZE: (f32, f32) = (164.0, 164.0);
const COUNTDOWN_SIZE: (f32, f32) = (200.0, 190.0);
const CARD_SIZE: (f32, f32) = (260.0, 140.0);
const LAYER_WIDTH: u32 = 280;
const LAYER_HEIGHT: u32 = 220;
const PAD: f32 = 10.0;

pub(crate) fn ink() -> Color {
    Color::from_rgba(42, 36, 28, 255)
}

pub(crate) fn muted_ink() -> Color {
    Color::from_rgba(112, 94, 74, 255)
}

fn deep_green() -> Color {
    Color::from_rgba(52, 138, 68, 255)
}

fn cream() -> Color {
    Color::from_rgba(255, 248, 225, 255)
}

fn paper() -> Color {
    Color::from_rgba(255, 253, 246, 255)
}

fn shadow() -> Color {
    Color::from_rgba(20, 18, 14, 70)
}

fn leaf_color() -> Color {
    Color::from_rgba(86, 172, 82, 255)
}

pub(crate) fn sticker_card(x: f32, y: f32, width: f32, height: f32, fill: Color) {
    let radius = height / 2.0;
    draw_rectangle(
        x + radius + 5.0,
        y + 7.0,
        width - 2.0 * radius,
        height,
        shadow(),
    );
    draw_circle(x + radius + 5.0, y + radius + 7.0, radius, shadow());
    draw_circle(x + width - radius + 5.0, y + radius + 7.0, radius, shadow());
    draw_rectangle(
        x + radius,
        y - 6.0,
        width - 2.0 * radius,
        height + 12.0,
        paper(),
    );
    draw_circle(x + radius, y + radius, radius + 6.0, paper());
    draw_circle(x + width - radius, y + radius, radius + 6.0, paper());
    draw_rectangle(x + radius, y, width - 2.0 * radius, height, fill);
    draw_circle(x + radius, y + radius, radius, fill);
    draw_circle(x + width - radius, y + radius, radius, fill);
}

pub(crate) fn mini_fruit(cx: f32, cy: f32, radius: f32, fill: Color) {
    draw_circle(cx + 2.0, cy + 3.0, radius + 3.0, shadow());
    draw_circle(cx, cy, radius + 3.0, paper());
    draw_circle(cx, cy, radius, fill);
    draw_ellipse(
        cx - radius * 0.35,
        cy - radius * 0.35,
        radius * 0.48,
        radius * 0.28,
        -20.0,
        Color::new(1.0, 1.0, 1.0, 0.35),
    );
    draw_line(
        cx + 1.0,
        cy - radius * 0.8,
        cx + 4.0,
        cy - radius * 1.35,
        3.0,
        paper(),
    );
    draw_line(
        cx + 1.0,
        cy - radius * 0.8,
        cx + 4.0,
        cy - radius * 1.35,
        1.5,
        Color::from_rgba(92, 72, 48, 255),
    );
    draw_ellipse(
        cx + radius * 0.58,
        cy - radius * 1.04,
        radius * 0.85,
        radius * 0.35,
        -25.0,
        paper(),
    );
    draw_ellipse(
        cx + radius * 0.58,
        cy - radius * 1.04,
        radius * 0.65,
        radius * 0.2,
        -25.0,
        leaf_color(),
    );
}

pub struct StickerLayer {
    target: RenderTarget,
}

impl StickerLayer {
    pub fn new() -> Self {
        let target = render_target(LAYER_WIDTH, LAYER_HEIGHT);
        target.texture.set_filter(FilterMode::Nearest);
        Self { target }
    }

    pub fn draw(&self, views: &[crate::stickers::StickerView]) {
        for view in views {
            let (width, height) = size(view.kind);
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
            draw_one(view, PAD, PAD);
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

fn size(kind: crate::stickers::Kind) -> (f32, f32) {
    match kind {
        crate::stickers::Kind::Timer => TIMER_SIZE,
        crate::stickers::Kind::Countdown => COUNTDOWN_SIZE,
        crate::stickers::Kind::Todo | crate::stickers::Kind::Note => CARD_SIZE,
    }
}

fn draw_one(view: &crate::stickers::StickerView, x: f32, y: f32) {
    match view.kind {
        crate::stickers::Kind::Timer => draw_timer(view, x, y),
        crate::stickers::Kind::Countdown => draw_countdown(view, x, y),
        crate::stickers::Kind::Todo => draw_todo(view, x, y),
        crate::stickers::Kind::Note => draw_note(view, x, y),
    }
}

fn fruit(cx: f32, cy: f32, rx: f32, ry: f32, fill: Color) {
    draw_ellipse(cx + 6.0, cy + 9.0, rx + 8.0, ry + 8.0, 0.0, shadow());
    draw_ellipse(cx, cy, rx + 8.0, ry + 8.0, 0.0, paper());
    draw_ellipse(cx, cy, rx, ry, 0.0, fill);
    draw_ellipse(
        cx - rx * 0.34,
        cy - ry * 0.48,
        rx * 0.34,
        ry * 0.16,
        -18.0,
        Color::new(1.0, 1.0, 1.0, 0.3),
    );
}

fn stem_leaf(cx: f32, cy: f32) {
    draw_line(cx, cy, cx + 2.0, cy - 15.0, 7.0, paper());
    draw_line(
        cx,
        cy,
        cx + 2.0,
        cy - 15.0,
        3.0,
        Color::from_rgba(92, 72, 48, 255),
    );
    draw_ellipse(cx + 18.0, cy - 16.0, 30.0, 12.0, -28.0, paper());
    draw_ellipse(cx + 18.0, cy - 16.0, 22.0, 7.0, -28.0, leaf_color());
}

fn face(cx: f32, cy: f32) {
    draw_ellipse(cx - 16.0, cy, 3.0, 5.0, 0.0, ink());
    draw_ellipse(cx + 16.0, cy, 3.0, 5.0, 0.0, ink());
    draw_circle(
        cx - 29.0,
        cy + 9.0,
        4.5,
        Color::from_rgba(255, 132, 130, 150),
    );
    draw_circle(
        cx + 29.0,
        cy + 9.0,
        4.5,
        Color::from_rgba(255, 132, 130, 150),
    );
    draw_arc(cx, cy + 2.0, 16, 8.0, 0.0, 2.0, 180.0, ink());
}

fn draw_timer(view: &crate::stickers::StickerView, x: f32, y: f32) {
    let cx = x + TIMER_SIZE.0 / 2.0;
    let cy = y + TIMER_SIZE.1 / 2.0 + 4.0;
    fruit(cx, cy, 66.0, 60.0, Color::from_rgba(226, 76, 58, 255));
    stem_leaf(cx, cy - 56.0);
    draw_circle(cx, cy + 6.0, 47.0, paper());
    draw_circle(cx, cy + 6.0, 43.0, cream());
    draw_arc(
        cx,
        cy + 6.0,
        40,
        45.0,
        -90.0,
        3.0,
        360.0,
        Color::from_rgba(232, 218, 198, 255),
    );
    let fraction = view.duration_seconds.unwrap_or(0).max(1) as f32;
    let fraction = view.remaining_seconds.unwrap_or(0) as f32 / fraction;
    if fraction > 0.0 {
        draw_arc(
            cx,
            cy + 6.0,
            40,
            45.0,
            -90.0,
            3.0,
            360.0 * fraction.clamp(0.0, 1.0),
            Color::from_rgba(188, 50, 42, 255),
        );
    }
    let label = match view.state {
        "running" => "FOCUS",
        "paused" => "PAUSED",
        "done" => "DONE",
        _ => "READY",
    };
    draw_centered(
        label,
        cx,
        cy - 8.0,
        10.0,
        if view.state == "running" || view.state == "done" {
            deep_green()
        } else {
            muted_ink()
        },
    );
    let remaining = view.remaining_seconds.unwrap_or(0);
    draw_centered(
        &format!("{:02}:{:02}", remaining / 60, remaining % 60),
        cx,
        cy + 22.0,
        26.0,
        ink(),
    );
}

fn draw_countdown(view: &crate::stickers::StickerView, x: f32, y: f32) {
    let cx = x + COUNTDOWN_SIZE.0 / 2.0;
    let cy = y + 72.0;
    fruit(cx, cy, 80.0, 54.0, Color::from_rgba(255, 214, 72, 255));
    draw_ellipse(cx - 72.0, cy + 12.0, 28.0, 24.0, -18.0, paper());
    draw_ellipse(
        cx - 72.0,
        cy + 12.0,
        20.0,
        18.0,
        -18.0,
        Color::from_rgba(255, 214, 72, 255),
    );
    draw_ellipse(cx + 72.0, cy - 12.0, 28.0, 24.0, 18.0, paper());
    draw_ellipse(
        cx + 72.0,
        cy - 12.0,
        20.0,
        18.0,
        18.0,
        Color::from_rgba(255, 214, 72, 255),
    );
    face(cx, cy - 10.0);
    let banner_y = y + 120.0;
    sticker_card(x + 10.0, banner_y, COUNTDOWN_SIZE.0 - 20.0, 60.0, cream());
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
    draw_centered(&label, cx, banner_y + 26.0, 18.0, ink());
    draw_centered(
        &view.text.clone().unwrap_or_default().to_uppercase(),
        cx,
        banner_y + 46.0,
        10.0,
        ink(),
    );
}

fn draw_todo(view: &crate::stickers::StickerView, x: f32, y: f32) {
    let cx = x + CARD_SIZE.0 / 2.0;
    let cy = y + CARD_SIZE.1 / 2.0;
    fruit(
        cx,
        cy + 8.0,
        116.0,
        58.0,
        Color::from_rgba(145, 205, 106, 255),
    );
    stem_leaf(cx, cy - 48.0);
    draw_circle(x + 34.0, cy, 17.0, paper());
    draw_circle_lines(x + 34.0, cy, 15.0, 2.0, ink());
    if view.done.unwrap_or(false) {
        draw_line(x + 27.0, cy, x + 32.0, cy + 5.0, 3.0, deep_green());
        draw_line(x + 32.0, cy + 5.0, x + 42.0, cy - 7.0, 3.0, deep_green());
    }
    let lines = wrap(&view.text.clone().unwrap_or_default(), 170.0, 17, 3);
    let baseline = cy - lines.len() as f32 * 20.0 / 2.0 + 14.0;
    draw_lines_centered(&lines, x + 154.0, baseline, 17.0, 20.0, ink());
}

fn draw_note(view: &crate::stickers::StickerView, x: f32, y: f32) {
    let cx = x + CARD_SIZE.0 / 2.0;
    let cy = y + CARD_SIZE.1 / 2.0;
    fruit(cx, cy, 120.0, 58.0, Color::from_rgba(255, 176, 142, 255));
    stem_leaf(cx + 30.0, cy - 48.0);
    let title = view.title.as_deref().unwrap_or("");
    let body = view.text.as_deref().unwrap_or("");
    if !title.is_empty() && !body.is_empty() {
        draw_centered(title, cx, cy - 3.0, 17.0, ink());
        let lines = wrap(body, 190.0, 14, 2);
        draw_lines_centered(&lines, cx, cy + 21.0, 14.0, 17.0, ink());
    } else {
        let text = if title.is_empty() { body } else { title };
        let lines = wrap(text, 190.0, 17, 3);
        let baseline = cy - lines.len() as f32 * 20.0 / 2.0 + 14.0;
        draw_lines_centered(&lines, cx, baseline, 17.0, 20.0, ink());
    }
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
