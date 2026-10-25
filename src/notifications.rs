use crate::stickers::StickerView;
use axum::{Json, extract::State, http::StatusCode};
use chrono::Local;
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub type SharedFeed = Arc<Mutex<Feed>>;

#[derive(Default)]
pub struct Feed {
    next_id: u64,
    entries: VecDeque<Notification>,
}

#[derive(Clone, Serialize)]
pub struct Notification {
    id: u64,
    title: String,
    message: String,
    created_at: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewNotification {
    #[serde(default = "default_title")]
    title: String,
    message: String,
}

fn default_title() -> String {
    "Notification".to_owned()
}

pub async fn post(
    State(feed): State<SharedFeed>,
    Json(input): Json<NewNotification>,
) -> Result<(StatusCode, Json<Notification>), (StatusCode, String)> {
    let title = input.title.trim();
    let message = input.message.trim();
    if title.is_empty()
        || title.chars().count() > 48
        || message.is_empty()
        || message.chars().count() > 320
        || title.chars().any(char::is_control)
        || message.chars().any(|c| c.is_control() && c != '\n')
    {
        return Err((StatusCode::BAD_REQUEST,
            "title must be 1–48 characters; message must be 1–320 characters; only message newlines are allowed as control characters".into()));
    }
    let mut feed = feed.lock().unwrap();
    feed.next_id += 1;
    let notification = Notification {
        id: feed.next_id,
        title: title.into(),
        message: message.into(),
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    feed.entries.push_front(notification.clone());
    feed.entries.truncate(32);
    Ok((StatusCode::CREATED, Json(notification)))
}

pub async fn list(State(feed): State<SharedFeed>) -> Json<Vec<Notification>> {
    Json(feed.lock().unwrap().entries.iter().cloned().collect())
}

pub async fn clear(State(feed): State<SharedFeed>) -> StatusCode {
    feed.lock().unwrap().entries.clear();
    StatusCode::NO_CONTENT
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
                continue;
            }
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            if measure_text(word, None, size, 1.0).width <= width {
                current = word.to_owned();
            } else {
                for c in word.chars() {
                    let candidate = format!("{current}{c}");
                    if measure_text(&candidate, None, size, 1.0).width > width
                        && !current.is_empty()
                    {
                        lines.push(std::mem::take(&mut current));
                    }
                    current.push(c);
                }
            }
        }
        if !current.is_empty() {
            lines.push(current);
        } else if paragraph.is_empty() {
            lines.push(String::new());
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.last_mut().unwrap();
        while measure_text(&format!("{last}..."), None, size, 1.0).width > width {
            last.pop();
        }
        last.push_str("...");
    }
    lines
}

fn prepare_font_atlas(notifications: &[Notification], stickers: &[StickerView]) {
    let mut glyphs: Vec<char> = (32u8..=126).map(char::from).collect();
    for notification in notifications {
        glyphs.extend(notification.title.chars());
        glyphs.extend(notification.message.chars());
    }
    for sticker in stickers {
        glyphs.extend(sticker.title.as_deref().unwrap_or_default().chars());
        glyphs.extend(sticker.text.as_deref().unwrap_or_default().chars());
    }
    glyphs.sort_unstable();
    glyphs.dedup();

    let font = get_default_font();
    let dpi = screen_dpi_scale();
    for logical_size in [
        10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 26, 52, 62,
    ] {
        let raster_size = (logical_size as f32 * dpi).ceil() as u16;
        font.populate_font_cache(&glyphs, raster_size);
    }
}

/// Draw directly into the encoder target: every media client sees this same feed.
pub fn draw(
    feed: &SharedFeed,
    stickers: &[StickerView],
    sticker_layer: &crate::sticker_art::StickerLayer,
) {
    let entries: Vec<_> = {
        let feed = feed.lock().unwrap();
        feed.entries.iter().take(3).cloned().collect()
    };
    // Populate all glyphs/sizes before queuing any frame text. Macroquad may
    // resize its glyph atlas when new text appears, invalidating queued quads.
    prepare_font_atlas(&entries, stickers);
    let now = Local::now();
    let green = Color::from_rgba(157, 208, 104, 255);
    let muted = Color::from_rgba(81, 113, 66, 255);
    let ivory = Color::from_rgba(224, 229, 207, 255);
    let clock = now.format("%-I:%M").to_string();
    let seconds = now.format(":%S").to_string();
    let meridiem = now.format("%p").to_string();
    let clock_width = measure_text(&clock, None, 52, 1.0).width;
    let suffix_width = measure_text(&seconds, None, 17, 1.0)
        .width
        .max(measure_text(&meridiem, None, 13, 1.0).width);
    let clock_x = (440.0 - clock_width - 8.0 - suffix_width) / 2.0;
    let suffix_x = clock_x + clock_width + 8.0;
    draw_text(&clock, clock_x, 68.0, 52.0, ivory);
    draw_text(&seconds, suffix_x, 48.0, 17.0, green);
    draw_text(&meridiem, suffix_x, 67.0, 13.0, muted);
    let date = now.format("%A, %B %-d").to_string();
    let date_width = measure_text(&date, None, 18, 1.0).width;
    draw_text(&date, 220.0 - date_width / 2.0, 101.0, 18.0, ivory);

    for (i, entry) in entries.iter().enumerate() {
        let y = 24.0 + i as f32 * 82.0;
        let fill = Color::from_rgba(4, 12, 7, 245);
        let pale = Color::from_rgba(183, 198, 165, 255);
        draw_rectangle(470.0, y, 460.0, 75.0, fill);
        draw_rectangle_lines(470.0, y, 460.0, 75.0, 1.0, muted);
        draw_rectangle(470.0, y, 3.0, 75.0, green);
        let title = wrap(&entry.title, 340.0, 17, 1);
        draw_text(&title[0], 486.0, y + 22.0, 17.0, green);
        let age = (now.timestamp().max(0) as u64).saturating_sub(entry.created_at);
        let age_label = if age < 60 {
            "now".to_owned()
        } else if age < 3600 {
            format!("{}m", age / 60)
        } else if age < 86400 {
            format!("{}h", age / 3600)
        } else {
            format!("{}d", age / 86400)
        };
        draw_text(&age_label, 878.0, y + 22.0, 13.0, muted);
        for (j, line) in wrap(&entry.message, 428.0, 16, 2).iter().enumerate() {
            draw_text(line, 486.0, y + 43.0 + j as f32 * 18.0, 16.0, pale);
        }
    }
    sticker_layer.draw(stickers);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn clear_removes_every_entry() {
        let feed: SharedFeed = Arc::new(Mutex::new(Feed::default()));
        {
            let mut state = feed.lock().unwrap();
            state.next_id = 1;
            state.entries.push_back(Notification {
                id: 1,
                title: "Kupo!".into(),
                message: "You have mail.".into(),
                created_at: 0,
            });
        }
        assert_eq!(clear(State(feed.clone())).await, StatusCode::NO_CONTENT);
        assert!(feed.lock().unwrap().entries.is_empty());
        assert_eq!(feed.lock().unwrap().next_id, 1);
    }
}
