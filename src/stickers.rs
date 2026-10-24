use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path as FsPath, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub type SharedStickers = Arc<Store>;

const VERSION: u8 = 1;
const DEFAULT_DURATION: u64 = 25 * 60;
const MAX_DURATION: u64 = 60 * 60;
const CANVAS_WIDTH: f32 = 960.0;
const CANVAS_HEIGHT: f32 = 540.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Timer,
    Countdown,
    Todo,
    Note,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Sticker {
    Timer {
        id: u64,
        x: f32,
        y: f32,
        angle: f32,
        duration_seconds: u64,
        remaining_at_anchor_seconds: u64,
        running_since: Option<u64>,
        has_started: bool,
        created_at: u64,
        updated_at: u64,
    },
    Countdown {
        id: u64,
        x: f32,
        y: f32,
        angle: f32,
        text: String,
        target: String,
        created_at: u64,
        updated_at: u64,
    },
    Todo {
        id: u64,
        x: f32,
        y: f32,
        angle: f32,
        text: String,
        done: bool,
        created_at: u64,
        updated_at: u64,
    },
    Note {
        id: u64,
        x: f32,
        y: f32,
        angle: f32,
        title: String,
        text: String,
        created_at: u64,
        updated_at: u64,
    },
}

impl Sticker {
    fn id(&self) -> u64 {
        match self {
            Self::Timer { id, .. }
            | Self::Countdown { id, .. }
            | Self::Todo { id, .. }
            | Self::Note { id, .. } => *id,
        }
    }

    fn kind(&self) -> Kind {
        match self {
            Self::Timer { .. } => Kind::Timer,
            Self::Countdown { .. } => Kind::Countdown,
            Self::Todo { .. } => Kind::Todo,
            Self::Note { .. } => Kind::Note,
        }
    }

    fn position(&self) -> (f32, f32, f32) {
        match self {
            Self::Timer { x, y, angle, .. }
            | Self::Countdown { x, y, angle, .. }
            | Self::Todo { x, y, angle, .. }
            | Self::Note { x, y, angle, .. } => (*x, *y, *angle),
        }
    }

    fn timestamps(&self) -> (u64, u64) {
        match self {
            Self::Timer {
                created_at,
                updated_at,
                ..
            }
            | Self::Countdown {
                created_at,
                updated_at,
                ..
            }
            | Self::Todo {
                created_at,
                updated_at,
                ..
            }
            | Self::Note {
                created_at,
                updated_at,
                ..
            } => (*created_at, *updated_at),
        }
    }

    fn set_position(&mut self, x: f32, y: f32, angle: f32) {
        match self {
            Self::Timer {
                x: sx,
                y: sy,
                angle: sa,
                ..
            }
            | Self::Countdown {
                x: sx,
                y: sy,
                angle: sa,
                ..
            }
            | Self::Todo {
                x: sx,
                y: sy,
                angle: sa,
                ..
            }
            | Self::Note {
                x: sx,
                y: sy,
                angle: sa,
                ..
            } => {
                *sx = x;
                *sy = y;
                *sa = angle;
            }
        }
    }

    fn set_updated_at(&mut self, value: u64) {
        match self {
            Self::Timer { updated_at, .. }
            | Self::Countdown { updated_at, .. }
            | Self::Todo { updated_at, .. }
            | Self::Note { updated_at, .. } => *updated_at = value,
        }
    }

    fn valid(&self) -> bool {
        let (x, y, angle) = self.position();
        self.id() > 0
            && x.is_finite()
            && y.is_finite()
            && angle.is_finite()
            && (-180.0..180.0).contains(&angle)
            && match self {
                Self::Timer {
                    duration_seconds,
                    remaining_at_anchor_seconds,
                    running_since,
                    has_started,
                    ..
                } => {
                    (1..=MAX_DURATION).contains(duration_seconds)
                        && remaining_at_anchor_seconds <= duration_seconds
                        && running_since.is_none_or(|stamp| stamp > 0)
                        && (running_since.is_none() || *has_started)
                        && (*has_started || remaining_at_anchor_seconds == duration_seconds)
                        && validate_stored_position(Kind::Timer, x, y, angle)
                }
                Self::Countdown { text, target, .. } => {
                    valid_text(text, 48, false)
                        && parse_target(target).is_some()
                        && validate_stored_position(Kind::Countdown, x, y, angle)
                }
                Self::Todo { text, .. } => {
                    valid_text(text, 160, true) && validate_stored_position(Kind::Todo, x, y, angle)
                }
                Self::Note { title, text, .. } => {
                    valid_optional_text(title, 48, false)
                        && valid_optional_text(text, 160, true)
                        && (!title.trim().is_empty() || !text.trim().is_empty())
                        && validate_stored_position(Kind::Note, x, y, angle)
                }
            }
    }

    fn view(&self, now: u64, today: NaiveDate) -> StickerView {
        let (x, y, angle) = self.position();
        let (created_at, updated_at) = self.timestamps();
        let mut view =
            StickerView::base(self.id(), self.kind(), x, y, angle, created_at, updated_at);
        match self {
            Self::Timer {
                duration_seconds,
                remaining_at_anchor_seconds,
                running_since,
                has_started,
                ..
            } => {
                let remaining =
                    effective_remaining(*remaining_at_anchor_seconds, *running_since, now);
                let running = running_since.is_some() && remaining > 0;
                view.duration_seconds = Some(*duration_seconds);
                view.remaining_seconds = Some(remaining);
                view.running = Some(running);
                view.finished = Some(remaining == 0);
                view.state = if remaining == 0 {
                    "done"
                } else if running {
                    "running"
                } else if *has_started {
                    "paused"
                } else {
                    "ready"
                };
            }
            Self::Countdown { text, target, .. } => {
                let days = (parse_target(target).unwrap_or(today) - today).num_days();
                view.text = Some(text.clone());
                view.target = Some(target.clone());
                view.days = Some(days);
                view.state = if days < 0 {
                    "past"
                } else if days == 0 {
                    "today"
                } else {
                    "upcoming"
                };
            }
            Self::Todo { text, done, .. } => {
                view.text = Some(text.clone());
                view.done = Some(*done);
                view.state = if *done { "done" } else { "open" };
            }
            Self::Note { title, text, .. } => {
                if !title.is_empty() {
                    view.title = Some(title.clone());
                }
                if !text.is_empty() {
                    view.text = Some(text.clone());
                }
                view.state = "note";
            }
        }
        view
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StickerFile {
    version: u8,
    next_id: u64,
    stickers: Vec<Sticker>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StickerView {
    pub id: u64,
    pub kind: Kind,
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished: Option<bool>,
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
}

impl StickerView {
    fn base(
        id: u64,
        kind: Kind,
        x: f32,
        y: f32,
        angle: f32,
        created_at: u64,
        updated_at: u64,
    ) -> Self {
        Self {
            id,
            kind,
            x,
            y,
            angle,
            created_at,
            updated_at,
            title: None,
            text: None,
            target: None,
            days: None,
            duration_seconds: None,
            remaining_seconds: None,
            running: None,
            finished: None,
            state: "",
            done: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Start,
    Pause,
    Reset,
    Set,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewSticker {
    kind: Kind,
    #[serde(default)]
    x: Option<f32>,
    #[serde(default)]
    y: Option<f32>,
    #[serde(default)]
    angle: Option<f32>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    duration_seconds: Option<u64>,
    #[serde(default)]
    done: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StickerUpdate {
    #[serde(default)]
    x: Option<f32>,
    #[serde(default)]
    y: Option<f32>,
    #[serde(default)]
    angle: Option<f32>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    done: Option<bool>,
    #[serde(default)]
    action: Option<Action>,
    #[serde(default)]
    duration_seconds: Option<u64>,
}

struct Inner {
    next_id: u64,
    stickers: Vec<Sticker>,
}

pub struct Store {
    path: PathBuf,
    inner: Mutex<Inner>,
}

impl Store {
    pub fn load(dir: impl AsRef<FsPath>) -> Result<SharedStickers, String> {
        let path = dir.as_ref().join("stickers.json");
        let (next_id, stickers) = match fs::read(&path) {
            Ok(bytes) => {
                let file: StickerFile = serde_json::from_slice(&bytes).map_err(|error| {
                    format!("invalid sticker state at {}: {error}", path.display())
                })?;
                if file.version != VERSION {
                    return Err(format!(
                        "unsupported sticker state version {} at {} (expected {VERSION})",
                        file.version,
                        path.display()
                    ));
                }
                if file.stickers.iter().any(|sticker| !sticker.valid()) {
                    return Err(format!("invalid sticker record at {}", path.display()));
                }
                let minimum_next = file
                    .stickers
                    .iter()
                    .map(|sticker| sticker.id().saturating_add(1))
                    .max()
                    .unwrap_or(1);
                if file.next_id < minimum_next || file.next_id == 0 {
                    return Err(format!("invalid next_id in {}", path.display()));
                }
                (file.next_id, file.stickers)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (2, vec![default_timer(1, now_seconds())])
            }
            Err(error) => return Err(format!("could not read {}: {error}", path.display())),
        };
        Ok(Arc::new(Self {
            path,
            inner: Mutex::new(Inner { next_id, stickers }),
        }))
    }

    pub fn snapshot(&self) -> Vec<StickerView> {
        self.snapshot_at(now_seconds(), today())
    }

    fn snapshot_at(&self, now: u64, today: NaiveDate) -> Vec<StickerView> {
        let inner = self.inner.lock().expect("sticker lock");
        let mut stickers = inner
            .stickers
            .iter()
            .map(|sticker| sticker.view(now, today))
            .collect::<Vec<_>>();
        stickers.sort_by_key(|sticker| sticker.id);
        stickers
    }

    pub fn add(&self, input: NewSticker) -> Result<StickerView, (StatusCode, String)> {
        self.add_at(input, now_seconds(), today())
    }

    fn add_at(
        &self,
        input: NewSticker,
        now: u64,
        today: NaiveDate,
    ) -> Result<StickerView, (StatusCode, String)> {
        validate_new(&input)?;
        let mut inner = self.inner.lock().expect("sticker lock");
        let id = inner.next_id;
        let (default_x, default_y) = default_position(input.kind);
        let angle = normalize_angle(input.angle.unwrap_or(0.0));
        let x = input.x.unwrap_or(default_x);
        let y = input.y.unwrap_or(default_y);
        let mut sticker = match input.kind {
            Kind::Timer => {
                let duration = input.duration_seconds.unwrap_or(DEFAULT_DURATION);
                Sticker::Timer {
                    id,
                    x,
                    y,
                    angle,
                    duration_seconds: duration,
                    remaining_at_anchor_seconds: duration,
                    running_since: None,
                    has_started: false,
                    created_at: now,
                    updated_at: now,
                }
            }
            Kind::Countdown => Sticker::Countdown {
                id,
                x,
                y,
                angle,
                text: input
                    .text
                    .expect("validated countdown text")
                    .trim()
                    .to_owned(),
                target: input.target.expect("validated countdown target"),
                created_at: now,
                updated_at: now,
            },
            Kind::Todo => Sticker::Todo {
                id,
                x,
                y,
                angle,
                text: input.text.expect("validated todo text").trim().to_owned(),
                done: input.done.unwrap_or(false),
                created_at: now,
                updated_at: now,
            },
            Kind::Note => Sticker::Note {
                id,
                x,
                y,
                angle,
                title: input.title.unwrap_or_default().trim().to_owned(),
                text: input.text.unwrap_or_default().trim().to_owned(),
                created_at: now,
                updated_at: now,
            },
        };
        clamp_sticker(&mut sticker);
        inner.next_id += 1;
        inner.stickers.push(sticker.clone());
        self.write(&inner)?;
        Ok(sticker.view(now, today))
    }

    pub fn update(
        &self,
        id: u64,
        input: StickerUpdate,
    ) -> Result<StickerView, (StatusCode, String)> {
        self.update_at(id, input, now_seconds(), today())
    }

    fn update_at(
        &self,
        id: u64,
        input: StickerUpdate,
        now: u64,
        today: NaiveDate,
    ) -> Result<StickerView, (StatusCode, String)> {
        let mut inner = self.inner.lock().expect("sticker lock");
        let index = inner
            .stickers
            .iter()
            .position(|sticker| sticker.id() == id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("no sticker {id}")))?;
        let mut next = inner.stickers[index].clone();
        validate_update(&next, &input)?;
        let (mut x, mut y, mut angle) = next.position();
        if let Some(value) = input.x {
            x = value;
        }
        if let Some(value) = input.y {
            y = value;
        }
        if let Some(value) = input.angle {
            angle = normalize_angle(value);
        }
        next.set_position(x, y, angle);
        match &mut next {
            Sticker::Timer {
                duration_seconds,
                remaining_at_anchor_seconds,
                running_since,
                has_started,
                ..
            } => {
                if let Some(action) = input.action {
                    apply_timer(
                        duration_seconds,
                        remaining_at_anchor_seconds,
                        running_since,
                        has_started,
                        action,
                        input.duration_seconds,
                        now,
                    )?;
                }
            }
            Sticker::Countdown { text, target, .. } => {
                if let Some(value) = input.text {
                    *text = value.trim().to_owned();
                }
                if let Some(value) = input.target {
                    *target = value;
                }
            }
            Sticker::Todo { text, done, .. } => {
                if let Some(value) = input.text {
                    *text = value.trim().to_owned();
                }
                if let Some(value) = input.done {
                    *done = value;
                }
            }
            Sticker::Note { title, text, .. } => {
                if let Some(value) = input.title {
                    *title = value.trim().to_owned();
                }
                if let Some(value) = input.text {
                    *text = value.trim().to_owned();
                }
            }
        }
        clamp_sticker(&mut next);
        if next != inner.stickers[index] {
            next.set_updated_at(now);
            inner.stickers[index] = next.clone();
            self.write(&inner)?;
        }
        Ok(next.view(now, today))
    }

    pub fn remove(&self, id: u64) -> Result<(), (StatusCode, String)> {
        let mut inner = self.inner.lock().expect("sticker lock");
        let index = inner
            .stickers
            .iter()
            .position(|sticker| sticker.id() == id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("no sticker {id}")))?;
        inner.stickers.remove(index);
        self.write(&inner)
    }

    fn write(&self, inner: &Inner) -> Result<(), (StatusCode, String)> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("could not create sticker directory: {error}"),
                )
            })?;
        }
        let temporary = self.path.with_extension("json.tmp");
        let file = StickerFile {
            version: VERSION,
            next_id: inner.next_id,
            stickers: inner.stickers.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&file).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("could not encode stickers: {error}"),
            )
        })?;
        fs::write(&temporary, bytes).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("could not write stickers: {error}"),
            )
        })?;
        fs::rename(&temporary, &self.path).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("could not replace stickers: {error}"),
            )
        })
    }
}

fn default_timer(id: u64, now: u64) -> Sticker {
    Sticker::Timer {
        id,
        x: 20.0,
        y: 16.0,
        angle: 0.0,
        duration_seconds: DEFAULT_DURATION,
        remaining_at_anchor_seconds: DEFAULT_DURATION,
        running_since: None,
        has_started: false,
        created_at: now,
        updated_at: now,
    }
}

fn default_position(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Timer => (20.0, 16.0),
        Kind::Countdown => (480.0, 347.0),
        Kind::Todo => (697.0, 380.0),
        Kind::Note => (697.0, 12.0),
    }
}

fn dimensions(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Timer => (164.0, 164.0),
        Kind::Countdown => (200.0, 190.0),
        Kind::Todo | Kind::Note => (260.0, 140.0),
    }
}

fn clamp_sticker(sticker: &mut Sticker) {
    let (x, y, angle) = sticker.position();
    let (x, y) = clamp_position(sticker.kind(), x, y, angle);
    sticker.set_position(x, y, angle);
}

fn clamp_position(kind: Kind, x: f32, y: f32, angle: f32) -> (f32, f32) {
    let (width, height) = dimensions(kind);
    let radians = angle.to_radians();
    let cos = radians.cos().abs();
    let sin = radians.sin().abs();
    let half_w = width / 2.0 + 10.0;
    let half_h = height / 2.0 + 10.0;
    let extent_x = cos * half_w + sin * half_h;
    let extent_y = sin * half_w + cos * half_h;
    let center_x = (x + width / 2.0).clamp(extent_x, CANVAS_WIDTH - extent_x);
    let center_y = (y + height / 2.0).clamp(extent_y, CANVAS_HEIGHT - extent_y);
    (center_x - width / 2.0, center_y - height / 2.0)
}

fn validate_stored_position(kind: Kind, x: f32, y: f32, angle: f32) -> bool {
    let (clamped_x, clamped_y) = clamp_position(kind, x, y, angle);
    (x - clamped_x).abs() < 0.01 && (y - clamped_y).abs() < 0.01
}

fn normalize_angle(angle: f32) -> f32 {
    (angle + 180.0).rem_euclid(360.0) - 180.0
}

fn validate_new(input: &NewSticker) -> Result<(), (StatusCode, String)> {
    for (name, value, max) in [("x", input.x, CANVAS_WIDTH), ("y", input.y, CANVAS_HEIGHT)] {
        if let Some(value) = value {
            validate_coordinate(value, name, max)?;
        }
    }
    if input.angle.is_some_and(|angle| !angle.is_finite()) {
        return Err((StatusCode::BAD_REQUEST, "angle must be finite".into()));
    }
    match input.kind {
        Kind::Timer => {
            reject_fields(
                input.title.is_some()
                    || input.text.is_some()
                    || input.target.is_some()
                    || input.done.is_some(),
                "timer accepts only duration_seconds in addition to position and angle",
            )?;
            if let Some(duration) = input.duration_seconds {
                validate_duration(duration)?;
            }
        }
        Kind::Countdown => {
            reject_fields(
                input.title.is_some() || input.duration_seconds.is_some() || input.done.is_some(),
                "countdown does not accept title, duration_seconds, or done",
            )?;
            validate_label(input.text.as_deref().unwrap_or(""))?;
            let target = input.target.as_deref().ok_or((
                StatusCode::BAD_REQUEST,
                "countdown requires target (YYYY-MM-DD)".into(),
            ))?;
            if parse_target(target).is_none() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "target must be a valid YYYY-MM-DD date".into(),
                ));
            }
        }
        Kind::Todo => {
            reject_fields(
                input.title.is_some() || input.target.is_some() || input.duration_seconds.is_some(),
                "todo does not accept title, target, or duration_seconds",
            )?;
            validate_body(input.text.as_deref().unwrap_or(""), "todo")?;
        }
        Kind::Note => {
            reject_fields(
                input.target.is_some() || input.duration_seconds.is_some() || input.done.is_some(),
                "note does not accept target, duration_seconds, or done",
            )?;
            validate_optional_note(
                input.title.as_deref().unwrap_or(""),
                input.text.as_deref().unwrap_or(""),
            )?;
        }
    }
    Ok(())
}

fn validate_update(sticker: &Sticker, input: &StickerUpdate) -> Result<(), (StatusCode, String)> {
    if let Some(x) = input.x {
        validate_coordinate(x, "x", CANVAS_WIDTH)?;
    }
    if let Some(y) = input.y {
        validate_coordinate(y, "y", CANVAS_HEIGHT)?;
    }
    if input.angle.is_some_and(|angle| !angle.is_finite()) {
        return Err((StatusCode::BAD_REQUEST, "angle must be finite".into()));
    }
    match sticker {
        Sticker::Timer { .. } => {
            reject_fields(
                input.title.is_some()
                    || input.text.is_some()
                    || input.target.is_some()
                    || input.done.is_some(),
                "timer accepts only position, angle, action, and duration_seconds",
            )?;
            if let Some(duration) = input.duration_seconds {
                validate_duration(duration)?;
            }
            if input.duration_seconds.is_some() && input.action != Some(Action::Set) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "duration_seconds is writable only with action 'set'".into(),
                ));
            }
            if input.action == Some(Action::Set) && input.duration_seconds.is_none() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "action 'set' requires duration_seconds".into(),
                ));
            }
        }
        Sticker::Countdown { .. } => {
            reject_fields(
                input.title.is_some()
                    || input.done.is_some()
                    || input.action.is_some()
                    || input.duration_seconds.is_some(),
                "countdown accepts only position, angle, text, and target",
            )?;
            if let Some(text) = &input.text {
                validate_label(text)?;
            }
            if let Some(target) = &input.target {
                if parse_target(target).is_none() {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "target must be a valid YYYY-MM-DD date".into(),
                    ));
                }
            }
        }
        Sticker::Todo { .. } => {
            reject_fields(
                input.title.is_some()
                    || input.target.is_some()
                    || input.action.is_some()
                    || input.duration_seconds.is_some(),
                "todo accepts only position, angle, text, and done",
            )?;
            if let Some(text) = &input.text {
                validate_body(text, "todo")?;
            }
        }
        Sticker::Note { title, text, .. } => {
            reject_fields(
                input.target.is_some()
                    || input.done.is_some()
                    || input.action.is_some()
                    || input.duration_seconds.is_some(),
                "note accepts only position, angle, title, and text",
            )?;
            let next_title = input.title.as_deref().unwrap_or(title);
            let next_text = input.text.as_deref().unwrap_or(text);
            validate_optional_note(next_title, next_text)?;
        }
    }
    if input.x.is_none()
        && input.y.is_none()
        && input.angle.is_none()
        && input.title.is_none()
        && input.text.is_none()
        && input.target.is_none()
        && input.done.is_none()
        && input.action.is_none()
        && input.duration_seconds.is_none()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "at least one sticker field must be provided".into(),
        ));
    }
    Ok(())
}

fn reject_fields(reject: bool, message: &str) -> Result<(), (StatusCode, String)> {
    if reject {
        Err((StatusCode::BAD_REQUEST, message.into()))
    } else {
        Ok(())
    }
}

fn validate_coordinate(value: f32, name: &str, max: f32) -> Result<(), (StatusCode, String)> {
    if value.is_finite() && (0.0..=max).contains(&value) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            format!("{name} must be finite and between 0 and {max}"),
        ))
    }
}

fn validate_duration(duration: u64) -> Result<(), (StatusCode, String)> {
    if (1..=MAX_DURATION).contains(&duration) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            format!("duration_seconds must be between 1 and {MAX_DURATION}"),
        ))
    }
}

fn validate_label(text: &str) -> Result<(), (StatusCode, String)> {
    if valid_text(text, 48, false) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "text must be 1–48 characters with no control characters".into(),
        ))
    }
}

fn validate_body(text: &str, name: &str) -> Result<(), (StatusCode, String)> {
    if valid_text(text, 160, true) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            format!(
                "{name} text must be 1–160 characters; only newlines are allowed as control characters"
            ),
        ))
    }
}

fn validate_optional_note(title: &str, text: &str) -> Result<(), (StatusCode, String)> {
    if !valid_optional_text(title, 48, false) {
        return Err((
            StatusCode::BAD_REQUEST,
            "note title must be at most 48 characters with no control characters".into(),
        ));
    }
    if !valid_optional_text(text, 160, true) {
        return Err((StatusCode::BAD_REQUEST, "note text must be at most 160 characters; only newlines are allowed as control characters".into()));
    }
    if title.trim().is_empty() && text.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "note requires a non-empty title or text".into(),
        ));
    }
    Ok(())
}

fn valid_optional_text(value: &str, max: usize, allow_newlines: bool) -> bool {
    value.chars().count() <= max
        && value
            .chars()
            .all(|char| !char.is_control() || (allow_newlines && char == '\n'))
}

fn valid_text(text: &str, max: usize, allow_newlines: bool) -> bool {
    !text.trim().is_empty() && valid_optional_text(text, max, allow_newlines)
}

fn parse_target(target: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(target, "%Y-%m-%d").ok()
}

fn effective_remaining(remaining: u64, running_since: Option<u64>, now: u64) -> u64 {
    running_since.map_or(remaining, |since| {
        remaining.saturating_sub(now.saturating_sub(since))
    })
}

fn apply_timer(
    duration_seconds: &mut u64,
    remaining: &mut u64,
    running_since: &mut Option<u64>,
    has_started: &mut bool,
    action: Action,
    new_duration: Option<u64>,
    now: u64,
) -> Result<(), (StatusCode, String)> {
    let current = effective_remaining(*remaining, *running_since, now);
    match action {
        Action::Start => {
            if running_since.is_none() || current == 0 {
                *remaining = if current == 0 {
                    *duration_seconds
                } else {
                    current
                };
                *running_since = Some(now);
                *has_started = true;
            }
        }
        Action::Pause => {
            if running_since.is_some() && current > 0 {
                *remaining = current;
                *running_since = None;
                *has_started = true;
            }
        }
        Action::Reset => {
            *remaining = *duration_seconds;
            *running_since = None;
            *has_started = false;
        }
        Action::Set => {
            let duration = new_duration.ok_or((
                StatusCode::BAD_REQUEST,
                "action 'set' requires duration_seconds".into(),
            ))?;
            validate_duration(duration)?;
            *duration_seconds = duration;
            *remaining = duration;
            *running_since = None;
            *has_started = false;
        }
    }
    Ok(())
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub async fn list(State(store): State<SharedStickers>) -> Json<Vec<StickerView>> {
    Json(store.snapshot())
}

pub async fn add(
    State(store): State<SharedStickers>,
    Json(input): Json<NewSticker>,
) -> Result<(StatusCode, Json<StickerView>), (StatusCode, String)> {
    Ok((StatusCode::CREATED, Json(store.add(input)?)))
}

pub async fn update(
    State(store): State<SharedStickers>,
    Path(id): Path<u64>,
    Json(input): Json<StickerUpdate>,
) -> Result<Json<StickerView>, (StatusCode, String)> {
    Ok(Json(store.update(id, input)?))
}

pub async fn remove(
    State(store): State<SharedStickers>,
    Path(id): Path<u64>,
) -> Result<StatusCode, (StatusCode, String)> {
    store.remove(id)?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn routes(store: SharedStickers) -> axum::Router {
    axum::Router::new()
        .route("/api/stickers", axum::routing::get(list).post(add))
        .route(
            "/api/stickers/{id}",
            axum::routing::patch(update).delete(remove),
        )
        .with_state(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "bronzecast-stickers-v1-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        path
    }

    fn input(kind: Kind, text: Option<&str>) -> NewSticker {
        NewSticker {
            kind,
            x: None,
            y: None,
            angle: None,
            title: None,
            text: text.map(str::to_owned),
            target: None,
            duration_seconds: None,
            done: None,
        }
    }

    fn timer_update(action: Action, duration_seconds: Option<u64>) -> StickerUpdate {
        StickerUpdate {
            x: None,
            y: None,
            angle: None,
            title: None,
            text: None,
            target: None,
            done: None,
            action: Some(action),
            duration_seconds,
        }
    }

    #[test]
    fn seeds_only_a_ready_timer_and_persists_tagged_records() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let views = store.snapshot_at(1_000, NaiveDate::from_ymd_opt(2026, 10, 24).unwrap());
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].kind, Kind::Timer);
        assert_eq!(views[0].state, "ready");
        assert_eq!(views[0].angle, 0.0);

        let mut note = input(Kind::Note, Some("Body"));
        note.title = Some("Title".into());
        let note = store
            .add_at(note, 1_000, NaiveDate::from_ymd_opt(2026, 10, 24).unwrap())
            .unwrap();
        assert_eq!(note.kind, Kind::Note);
        let raw = fs::read_to_string(dir.join("stickers.json")).unwrap();
        assert!(raw.contains("\"version\": 1"));
        assert!(raw.contains("\"kind\": \"note\""));
        assert!(!raw.contains("remaining_at_anchor_seconds\": 0"));
        let reloaded = Store::load(&dir).unwrap();
        assert_eq!(reloaded.snapshot().len(), 2);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn creates_all_kinds_and_rejects_empty_or_cross_kind_fields() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let mut countdown = input(Kind::Countdown, Some("Halloween"));
        countdown.target = Some("2026-10-31".into());
        assert_eq!(store.add_at(countdown, 1_000, today).unwrap().days, Some(7));
        assert_eq!(
            store
                .add_at(input(Kind::Todo, Some("Pack")), 1_000, today)
                .unwrap()
                .state,
            "open"
        );
        assert!(
            store
                .add_at(input(Kind::Note, Some("   ")), 1_000, today)
                .is_err()
        );
        let mut bad = input(Kind::Todo, Some("Task"));
        bad.target = Some("2026-10-31".into());
        assert!(store.add_at(bad, 1_000, today).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn timer_anchor_survives_restart_and_finished_start_restarts() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let timer_id = store.snapshot()[0].id;
        let started = store
            .update_at(
                timer_id,
                timer_update(Action::Set, Some(10)),
                1_000,
                today(),
            )
            .unwrap();
        assert_eq!(started.remaining_seconds, Some(10));
        store
            .update_at(timer_id, timer_update(Action::Start, None), 1_000, today())
            .unwrap();
        let reopened = Store::load(&dir).unwrap();
        assert_eq!(
            reopened.snapshot_at(1_006, today())[0].remaining_seconds,
            Some(4)
        );
        let paused = reopened
            .update_at(timer_id, timer_update(Action::Pause, None), 1_006, today())
            .unwrap();
        assert_eq!(paused.remaining_seconds, Some(4));
        assert_eq!(paused.state, "paused");
        reopened
            .update_at(timer_id, timer_update(Action::Start, None), 2_000, today())
            .unwrap();
        let reset = reopened
            .update_at(timer_id, timer_update(Action::Reset, None), 2_003, today())
            .unwrap();
        assert_eq!(reset.remaining_seconds, Some(10));
        assert_eq!(reset.state, "ready");
        reopened
            .update_at(timer_id, timer_update(Action::Start, None), 3_000, today())
            .unwrap();
        let done = reopened.snapshot_at(3_011, today());
        assert_eq!(done[0].state, "done");
        let again = reopened
            .update_at(timer_id, timer_update(Action::Start, None), 3_011, today())
            .unwrap();
        assert_eq!(again.remaining_seconds, Some(10));
        assert_eq!(again.state, "running");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn accepts_arbitrary_angles_and_clamps_rotated_bounds() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let mut todo = input(Kind::Todo, Some("Rotate me"));
        todo.x = Some(959.0);
        todo.y = Some(539.0);
        todo.angle = Some(450.0);
        let view = store.add_at(todo, 1_000, today()).unwrap();
        assert_eq!(view.angle, 90.0);
        assert!(view.x + 130.0 <= CANVAS_WIDTH);
        assert!(view.y + 70.0 <= CANVAS_HEIGHT);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_old_or_malformed_state_instead_of_migrating_or_overwriting() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stickers.json");
        fs::write(&path, br#"{"next_id":2,"stickers":[]}"#).unwrap();
        assert!(Store::load(&dir).is_err());
        assert_eq!(fs::read(&path).unwrap(), br#"{"next_id":2,"stickers":[]}"#);
        fs::write(
            &path,
            br#"{"version":1,"next_id":2,"stickers":[{"kind":"timer","id":1,"x":20,"y":16,"angle":0,"duration_seconds":1500,"remaining_at_anchor_seconds":1500,"running_since":null,"has_started":false,"created_at":1,"updated_at":1,"text":"legacy-shape"}]}"#,
        )
        .unwrap();
        assert!(Store::load(&dir).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
