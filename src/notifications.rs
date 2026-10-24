use crate::stickers::StickerView;
use axum::{Json, extract::State, http::StatusCode};
use chrono::{Local, Timelike};
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

/// Draw directly into the encoder target: every media client sees this same feed.
pub fn draw(
    feed: &SharedFeed,
    stickers: &[StickerView],
    sticker_layer: &crate::sticker_art::StickerLayer,
    elapsed: f32,
    character_name: &str,
) {
    let (entries, count): (Vec<_>, _) = {
        let feed = feed.lock().unwrap();
        (
            feed.entries.iter().take(3).cloned().collect(),
            feed.entries.len(),
        )
    };
    let now = Local::now();
    let green = Color::from_rgba(157, 208, 104, 255);
    let muted = Color::from_rgba(81, 113, 66, 255);
    let ink = Color::from_rgba(4, 12, 7, 245);
    let header_x = 196.0;
    let header_width = 440.0 - header_x;
    draw_rectangle(header_x, 24.0, header_width, 94.0, ink);
    draw_rectangle_lines(header_x, 24.0, header_width, 94.0, 1.0, green);
    draw_text(
        &format!("{} / SIGNAL HOST", character_name.to_uppercase()),
        header_x + 14.0,
        45.0,
        15.0,
        green,
    );
    let latest = entries
        .first()
        .map(|n| n.title.as_str())
        .unwrap_or("Awaiting notifications");
    for (i, line) in wrap(latest, header_width - 28.0, 19, 2).iter().enumerate() {
        draw_text(line, header_x + 14.0, 74.0 + i as f32 * 22.0, 19.0, green);
    }
    draw_line(448.0, 24.0, 448.0, 512.0, 1.0, muted);
    let ivory = Color::from_rgba(224, 229, 207, 255);
    let greeting = match now.hour() {
        5..=11 => "GOOD MORNING",
        12..=16 => "GOOD AFTERNOON",
        17..=21 => "GOOD EVENING",
        _ => "NIGHT SHIFT",
    };
    draw_text(&format!("HOME / {greeting}"), 476.0, 40.0, 14.0, green);
    draw_text(&now.format("%-I:%M").to_string(), 472.0, 104.0, 62.0, ivory);
    // Place seconds and meridiem after the measured clock, including 10/11/12.
    let clock_width = measure_text(&now.format("%-I:%M").to_string(), None, 62, 1.0).width;
    let suffix_x = 482.0 + clock_width;
    draw_text(&now.format(":%S").to_string(), suffix_x, 79.0, 22.0, green);
    draw_text(
        &now.format("%p").to_string(),
        suffix_x + 1.0,
        103.0,
        16.0,
        muted,
    );
    draw_text(
        &now.format("%A, %B %-d").to_string(),
        476.0,
        134.0,
        21.0,
        ivory,
    );
    let timezone = now.format("%Z").to_string();
    let zone = format!("{} / HOST LOCAL", timezone);
    draw_text(&zone, 476.0, 154.0, 11.0, muted);
    let day_fraction = now.num_seconds_from_midnight() as f32 / 86400.0;
    draw_rectangle(476.0, 165.0, 450.0, 2.0, muted);
    draw_rectangle(476.0, 165.0, 450.0 * day_fraction, 2.0, green);

    let uptime = elapsed.max(0.0) as u64;
    let uptime_label = if uptime >= 86400 {
        format!("{}d {:02}h", uptime / 86400, uptime / 3600 % 24)
    } else {
        format!("{:02}h {:02}m", uptime / 3600, uptime / 60 % 60)
    };
    for (i, (label, value)) in [
        ("RENDERER", "LIVE".to_owned()),
        ("UPTIME", uptime_label),
        ("MESSAGES", format!("{count:02} / 32")),
    ]
    .iter()
    .enumerate()
    {
        let x = 470.0 + i as f32 * 156.0;
        draw_rectangle(x, 180.0, 148.0, 49.0, ink);
        draw_rectangle_lines(x, 180.0, 148.0, 49.0, 1.0, muted);
        draw_text(label, x + 12.0, 196.0, 11.0, muted);
        draw_text(value, x + 12.0, 218.0, 19.0, green);
    }
    draw_text("NOTIFICATIONS / NEWEST FIRST", 476.0, 253.0, 13.0, green);
    let cards = [
        (
            Color::from_rgba(255, 222, 190, 255),
            Color::from_rgba(255, 166, 72, 255),
        ),
        (
            Color::from_rgba(255, 238, 150, 255),
            Color::from_rgba(255, 214, 72, 255),
        ),
        (
            Color::from_rgba(210, 242, 200, 255),
            Color::from_rgba(145, 205, 106, 255),
        ),
    ];
    if entries.is_empty() {
        crate::sticker_art::sticker_card(470.0, 278.0, 460.0, 84.0, cards[0].0);
        crate::sticker_art::mini_fruit(506.0, 320.0, 24.0, cards[0].1);
        draw_text(
            "ALL QUIET",
            546.0,
            306.0,
            14.0,
            crate::sticker_art::muted_ink(),
        );
        draw_text(
            "Your host is on watch.",
            546.0,
            334.0,
            20.0,
            crate::sticker_art::ink(),
        );
        draw_text(
            "POST /api/notifications",
            546.0,
            352.0,
            13.0,
            crate::sticker_art::muted_ink(),
        );
    }
    for (i, entry) in entries.iter().enumerate() {
        let (fill, fruit) = cards[i % cards.len()];
        let y = 267.0 + i as f32 * 84.0;
        crate::sticker_art::sticker_card(470.0, y, 460.0, 76.0, fill);
        crate::sticker_art::mini_fruit(504.0, y + 38.0, 21.0, fruit);
        let title = wrap(&entry.title, 330.0, 17, 1);
        draw_text(&title[0], 540.0, y + 27.0, 17.0, crate::sticker_art::ink());
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
        draw_text(
            &age_label,
            878.0,
            y + 27.0,
            13.0,
            crate::sticker_art::muted_ink(),
        );
        for (j, line) in wrap(&entry.message, 330.0, 15, 2).iter().enumerate() {
            draw_text(
                line,
                540.0,
                y + 50.0 + j as f32 * 18.0,
                15.0,
                Color::from_rgba(92, 78, 62, 255),
            );
        }
    }
    draw_text("BRONZECAST / LOCAL SIGNAL", 28.0, 522.0, 12.0, muted);
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
