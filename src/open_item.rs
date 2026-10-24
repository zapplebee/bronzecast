use axum::{Json, extract::State, http::StatusCode};
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub type SharedOpenItem = Arc<Store>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenItem {
    pub title: String,
    pub note: String,
    pub opened_at: u64,
}

pub struct Store {
    path: PathBuf,
    item: Mutex<Option<OpenItem>>,
}

impl Store {
    pub fn load(dir: impl AsRef<Path>) -> SharedOpenItem {
        let path = dir.as_ref().join("open-item.json");
        let item = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(item) => Some(item),
                Err(error) => {
                    eprintln!(
                        "Ignoring unreadable open item at {}: {error}",
                        path.display()
                    );
                    None
                }
            },
            Err(_) => None,
        };
        Arc::new(Self {
            path,
            item: Mutex::new(item),
        })
    }

    pub fn snapshot(&self) -> Option<OpenItem> {
        self.item.lock().unwrap().clone()
    }

    fn write(&self, item: Option<&OpenItem>) -> std::io::Result<()> {
        match item {
            Some(item) => {
                if let Some(parent) = self.path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let temporary = self.path.with_extension("json.tmp");
                let bytes = serde_json::to_vec_pretty(item).map_err(std::io::Error::other)?;
                fs::write(&temporary, bytes)?;
                fs::rename(&temporary, &self.path)
            }
            None => match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            },
        }
    }

    pub fn replace(&self, item: OpenItem) -> std::io::Result<()> {
        let mut current = self.item.lock().unwrap();
        self.write(Some(&item))?;
        *current = Some(item);
        Ok(())
    }

    pub fn clear(&self) -> std::io::Result<()> {
        let mut current = self.item.lock().unwrap();
        self.write(None)?;
        *current = None;
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewOpenItem {
    title: String,
    #[serde(default)]
    note: String,
}

fn validate(input: NewOpenItem) -> Result<(String, String), String> {
    let title = input.title.trim();
    let note = input.note.trim();
    if title.is_empty()
        || title.chars().count() > 48
        || note.chars().count() > 160
        || title.chars().any(char::is_control)
        || note.chars().any(|c| c.is_control() && c != '\n')
    {
        return Err("title must be 1–48 characters; note must be 160 characters or fewer; only note newlines are allowed as control characters".into());
    }
    Ok((title.into(), note.into()))
}

fn merge(existing: Option<&OpenItem>, title: String, note: String, now: u64) -> OpenItem {
    let opened_at = existing
        .filter(|item| item.title == title)
        .map(|item| item.opened_at)
        .unwrap_or(now);
    OpenItem {
        title,
        note,
        opened_at,
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub async fn get(State(store): State<SharedOpenItem>) -> Json<Option<OpenItem>> {
    Json(store.snapshot())
}

pub async fn put(
    State(store): State<SharedOpenItem>,
    Json(input): Json<NewOpenItem>,
) -> Result<Json<OpenItem>, (StatusCode, String)> {
    let (title, note) = validate(input).map_err(|error| (StatusCode::BAD_REQUEST, error))?;
    let item = merge(store.snapshot().as_ref(), title, note, now_seconds());
    store.replace(item.clone()).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not persist open item: {error}"),
        )
    })?;
    Ok(Json(item))
}

pub async fn delete(
    State(store): State<SharedOpenItem>,
) -> Result<StatusCode, (StatusCode, String)> {
    store.clear().map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not clear open item: {error}"),
        )
    })?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn routes(store: SharedOpenItem) -> axum::Router {
    axum::Router::new()
        .route(
            "/api/open-item",
            axum::routing::get(get).put(put).delete(delete),
        )
        .with_state(store)
}

const ITEM_X: f32 = 700.0;
const ITEM_Y: f32 = 24.0;
const ITEM_WIDTH: f32 = 230.0;
const ITEM_HEIGHT: f32 = 94.0;

fn age_label(age: u64) -> String {
    if age < 60 {
        "NOW".to_owned()
    } else if age < 3600 {
        format!("{}M", age / 60)
    } else if age < 86400 {
        format!("{}H", age / 3600)
    } else {
        format!("{}D", age / 86400)
    }
}

pub fn draw(item: Option<&OpenItem>, now: u64) {
    let Some(item) = item else {
        return;
    };
    let ink = Color::from_rgba(4, 12, 7, 245);
    let green = Color::from_rgba(157, 208, 104, 255);
    let muted = Color::from_rgba(81, 113, 66, 255);
    let pale = Color::from_rgba(183, 198, 165, 255);
    let x = ITEM_X;
    let y = ITEM_Y;
    let w = ITEM_WIDTH;
    let h = ITEM_HEIGHT;

    draw_rectangle(x, y, w, h, ink);
    draw_rectangle_lines(x, y, w, h, 1.0, green);
    draw_text("OPEN ITEM", x + 12.0, y + 17.0, 11.0, muted);
    let age = age_label(now.saturating_sub(item.opened_at));
    let age_width = measure_text(&age, None, 11, 1.0).width;
    draw_text(&age, x + w - 12.0 - age_width, y + 17.0, 11.0, muted);
    for (index, line) in wrap(&item.note, w - 24.0, 15, 2).iter().enumerate() {
        draw_text(line, x + 12.0, y + 40.0 + index as f32 * 18.0, 15.0, pale);
    }
    draw_line(
        x + 12.0,
        y + h - 28.0,
        x + w - 12.0,
        y + h - 28.0,
        1.0,
        muted,
    );
    let title = wrap(&item.title, w - 24.0, 17, 1);
    draw_text(&title[0], x + 12.0, y + h - 9.0, 17.0, green);
}

fn wrap(text: &str, width: f32, size: u16, max_lines: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![String::new()];
    for c in text.chars() {
        if c == '\n' {
            lines.push(String::new());
            continue;
        }
        let current = lines.last_mut().unwrap();
        let candidate = format!("{current}{c}");
        if measure_text(&candidate, None, size, 1.0).width > width && !current.is_empty() {
            lines.push(c.to_string());
        } else {
            current.push(c);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bronzecast-open-item-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn rejects_bad_titles_and_notes() {
        let cases = [
            (String::new(), String::new()),
            ("   ".into(), String::new()),
            ("x".repeat(49), String::new()),
            ("bad\u{7}".into(), String::new()),
            ("ok".into(), "x".repeat(161)),
            ("ok".into(), "bad\u{7}".into()),
        ];
        for (title, note) in cases {
            assert!(
                validate(NewOpenItem { title, note }).is_err(),
                "expected rejection"
            );
        }
        assert_eq!(
            validate(NewOpenItem {
                title: "  Call the dentist  ".into(),
                note: "  Bring insurance\n".into(),
            })
            .unwrap(),
            ("Call the dentist".into(), "Bring insurance".into())
        );
    }

    #[test]
    fn same_title_keeps_the_open_age() {
        let existing = OpenItem {
            title: "Ship it".into(),
            note: String::new(),
            opened_at: 10,
        };
        assert_eq!(
            merge(Some(&existing), "Ship it".into(), "note".into(), 99).opened_at,
            10
        );
        assert_eq!(
            merge(Some(&existing), "Ship it now".into(), String::new(), 99).opened_at,
            99
        );
    }

    #[test]
    fn store_persists_and_clears() {
        let dir = temp_dir();
        let store = Store::load(&dir);
        assert_eq!(store.snapshot(), None);
        let item = OpenItem {
            title: "Water the plants".into(),
            note: "Kitchen too".into(),
            opened_at: 42,
        };
        store.replace(item.clone()).unwrap();
        assert_eq!(Store::load(&dir).snapshot(), Some(item));
        store.clear().unwrap();
        assert_eq!(Store::load(&dir).snapshot(), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
