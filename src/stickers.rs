use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{Local, NaiveDate};
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path as FsPath, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub type SharedStickers = Arc<Store>;

const DEFAULT_DURATION: u64 = 25 * 60;
const MAX_DURATION: u64 = 60 * 60;
const DEFAULT_TIMER_X: f32 = 20.0;
const DEFAULT_TIMER_Y: f32 = 16.0;
const TIMER_WIDTH: f32 = 164.0;
const TIMER_HEIGHT: f32 = 164.0;
const COUNTDOWN_WIDTH: f32 = 200.0;
const COUNTDOWN_HEIGHT: f32 = 190.0;
const REMINDER_WIDTH: f32 = 260.0;
const REMINDER_HEIGHT: f32 = 140.0;
const TODO_WIDTH: f32 = 260.0;
const TODO_HEIGHT: f32 = 140.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Countdown,
    Timer,
    Reminder,
    Todo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Sticker {
    id: u64,
    kind: Kind,
    x: f32,
    y: f32,
    #[serde(default)]
    text: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    duration_seconds: u64,
    #[serde(default)]
    remaining_seconds: u64,
    #[serde(default)]
    running: bool,
    #[serde(default)]
    updated_at: u64,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    created_at: u64,
}

#[derive(Serialize, Deserialize)]
struct StickerFile {
    next_id: u64,
    stickers: Vec<Sticker>,
}

#[derive(Deserialize)]
struct LegacyTimer {
    duration_seconds: u64,
    remaining_seconds: u64,
    running: bool,
    updated_at: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StickerView {
    pub id: u64,
    pub kind: Kind,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub target: Option<String>,
    pub days: Option<i64>,
    pub duration_seconds: u64,
    pub remaining_seconds: u64,
    pub running: bool,
    pub finished: bool,
    pub state: &'static str,
    pub done: bool,
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
    x: f32,
    y: f32,
    #[serde(default)]
    text: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    duration_seconds: Option<u64>,
    #[serde(default)]
    done: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StickerUpdate {
    #[serde(default)]
    x: Option<f32>,
    #[serde(default)]
    y: Option<f32>,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimerRequest {
    action: Action,
    #[serde(default)]
    duration_seconds: Option<u64>,
    #[serde(default)]
    x: Option<f32>,
    #[serde(default)]
    y: Option<f32>,
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
    pub fn load(dir: impl AsRef<FsPath>) -> SharedStickers {
        let path = dir.as_ref().join("stickers.json");
        let now = now_seconds();
        let (next_id, stickers) = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<StickerFile>(&bytes) {
                Ok(file) => {
                    let stickers: Vec<Sticker> =
                        file.stickers.into_iter().filter(Sticker::valid).collect();
                    let next_id = file.next_id.max(
                        stickers
                            .iter()
                            .map(|sticker| sticker.id + 1)
                            .max()
                            .unwrap_or(1),
                    );
                    (next_id, stickers)
                }
                Err(error) => {
                    eprintln!(
                        "Ignoring unreadable sticker state at {}: {error}",
                        path.display()
                    );
                    default_stickers(now, None)
                }
            },
            Err(_) => {
                let legacy = fs::read(dir.as_ref().join("timer.json"))
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<LegacyTimer>(&bytes).ok());
                default_stickers(now, legacy)
            }
        };
        Arc::new(Self {
            path,
            inner: Mutex::new(Inner { next_id, stickers }),
        })
    }

    pub fn snapshot(&self) -> Vec<StickerView> {
        self.snapshot_at(now_seconds(), today())
    }

    fn snapshot_at(&self, now: u64, today: NaiveDate) -> Vec<StickerView> {
        let inner = self.inner.lock().unwrap();
        let mut stickers: Vec<StickerView> = inner
            .stickers
            .iter()
            .map(|sticker| view_of(sticker, now, today))
            .collect();
        stickers.sort_by_key(|sticker| sticker.id);
        stickers
    }

    pub fn timer_view(&self) -> Option<StickerView> {
        self.timer_view_at(now_seconds(), today())
    }

    fn timer_view_at(&self, now: u64, today: NaiveDate) -> Option<StickerView> {
        let inner = self.inner.lock().unwrap();
        inner
            .stickers
            .iter()
            .find(|sticker| sticker.kind == Kind::Timer)
            .map(|sticker| view_of(sticker, now, today))
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
        let mut inner = self.inner.lock().unwrap();
        let id = inner.next_id;
        let (x, y) = clamp_position(input.kind, &input.text, input.x, input.y);
        let mut sticker = Sticker {
            id,
            kind: input.kind,
            x,
            y,
            text: input.text.trim().to_owned(),
            target: input.target,
            duration_seconds: 0,
            remaining_seconds: 0,
            running: false,
            updated_at: now,
            done: input.done,
            created_at: now,
        };
        if sticker.kind == Kind::Timer {
            let duration = input.duration_seconds.unwrap_or(DEFAULT_DURATION);
            sticker.duration_seconds = duration;
            sticker.remaining_seconds = duration;
            sticker.updated_at = now;
            sticker.done = false;
        }
        if sticker.kind != Kind::Countdown {
            sticker.target = None;
        }
        if sticker.kind != Kind::Todo {
            sticker.done = false;
        }
        inner.next_id += 1;
        inner.stickers.push(sticker.clone());
        self.write(&inner)?;
        Ok(view_of(&sticker, now, today))
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
        let mut inner = self.inner.lock().unwrap();
        let index = inner
            .stickers
            .iter()
            .position(|sticker| sticker.id == id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("no sticker {id}")))?;
        let mut next = inner.stickers[index].clone();
        validate_update(&next, &input)?;
        if let Some(x) = input.x {
            next.x = x;
        }
        if let Some(y) = input.y {
            next.y = y;
        }
        if let Some(text) = input.text {
            next.text = text.trim().to_owned();
        }
        match next.kind {
            Kind::Countdown => {
                if let Some(target) = input.target {
                    next.target = Some(target);
                }
            }
            Kind::Timer => {
                if let Some(action) = input.action {
                    apply_timer(&mut next, action, input.duration_seconds, now)?;
                } else if let Some(duration) = input.duration_seconds {
                    next.duration_seconds = duration;
                    next.remaining_seconds = duration;
                    next.running = false;
                    next.updated_at = now;
                }
            }
            Kind::Todo => {
                if let Some(done) = input.done {
                    next.done = done;
                }
            }
            Kind::Reminder => {}
        }
        let (x, y) = clamp_position(next.kind, &next.text, next.x, next.y);
        next.x = x;
        next.y = y;
        inner.stickers[index] = next.clone();
        self.write(&inner)?;
        Ok(view_of(&next, now, today))
    }

    pub fn remove(&self, id: u64) -> Result<(), (StatusCode, String)> {
        let mut inner = self.inner.lock().unwrap();
        let index = inner
            .stickers
            .iter()
            .position(|sticker| sticker.id == id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("no sticker {id}")))?;
        inner.stickers.remove(index);
        self.write(&inner)?;
        Ok(())
    }

    pub fn apply_timer_request(
        &self,
        input: TimerRequest,
    ) -> Result<StickerView, (StatusCode, String)> {
        self.apply_timer_request_at(input, now_seconds(), today())
    }

    fn apply_timer_request_at(
        &self,
        input: TimerRequest,
        now: u64,
        today: NaiveDate,
    ) -> Result<StickerView, (StatusCode, String)> {
        if input.action == Action::Set && input.duration_seconds.is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                "action 'set' requires duration_seconds".into(),
            ));
        }
        if let Some(duration) = input.duration_seconds {
            validate_duration(duration)?;
        }
        if let Some(x) = input.x {
            validate_x(x)?;
        }
        if let Some(y) = input.y {
            validate_y(y)?;
        }
        let mut inner = self.inner.lock().unwrap();
        let existing = inner
            .stickers
            .iter()
            .position(|sticker| sticker.kind == Kind::Timer);
        let mut sticker = existing
            .map(|index| inner.stickers[index].clone())
            .unwrap_or_else(|| default_timer(0, now));
        if let Some(x) = input.x {
            sticker.x = x;
        }
        if let Some(y) = input.y {
            sticker.y = y;
        }
        apply_timer(&mut sticker, input.action, input.duration_seconds, now)?;
        let (x, y) = clamp_position(sticker.kind, &sticker.text, sticker.x, sticker.y);
        sticker.x = x;
        sticker.y = y;
        match existing {
            Some(index) => inner.stickers[index] = sticker.clone(),
            None => {
                sticker.id = inner.next_id;
                inner.next_id += 1;
                inner.stickers.push(sticker.clone());
            }
        }
        self.write(&inner)?;
        Ok(view_of(&sticker, now, today))
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

fn default_stickers(now: u64, legacy: Option<LegacyTimer>) -> (u64, Vec<Sticker>) {
    let mut timer = default_timer(1, now);
    if let Some(legacy) = legacy {
        if (1..=MAX_DURATION).contains(&legacy.duration_seconds)
            && legacy.remaining_seconds <= legacy.duration_seconds
            && (!legacy.running || legacy.updated_at > 0)
        {
            timer.duration_seconds = legacy.duration_seconds;
            timer.remaining_seconds = legacy.remaining_seconds;
            timer.running = legacy.running;
            timer.updated_at = legacy.updated_at;
        }
    }
    (2, vec![timer])
}

fn default_timer(id: u64, now: u64) -> Sticker {
    Sticker {
        id,
        kind: Kind::Timer,
        x: DEFAULT_TIMER_X,
        y: DEFAULT_TIMER_Y,
        text: String::new(),
        target: None,
        duration_seconds: DEFAULT_DURATION,
        remaining_seconds: DEFAULT_DURATION,
        running: false,
        updated_at: now,
        done: false,
        created_at: now,
    }
}

impl Sticker {
    fn valid(&self) -> bool {
        self.id > 0
            && valid_position(self.x, self.y)
            && match self.kind {
                Kind::Countdown => {
                    valid_text(&self.text, 48, false)
                        && self.target.as_deref().and_then(parse_target).is_some()
                }
                Kind::Timer => {
                    (1..=MAX_DURATION).contains(&self.duration_seconds)
                        && self.remaining_seconds <= self.duration_seconds
                        && (!self.running || self.updated_at > 0)
                        && (self.text.is_empty() || valid_text(&self.text, 48, false))
                }
                Kind::Reminder => valid_text(&self.text, 160, true),
                Kind::Todo => valid_text(&self.text, 160, true),
            }
    }
}

fn validate_new(input: &NewSticker) -> Result<(), (StatusCode, String)> {
    if !valid_position(input.x, input.y) {
        return Err((
            StatusCode::BAD_REQUEST,
            "x must be 0–960 and y must be 0–540".into(),
        ));
    }
    match input.kind {
        Kind::Countdown => {
            validate_label(&input.text)?;
            let target = input.target.as_deref().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "countdown requires target (YYYY-MM-DD)".into(),
                )
            })?;
            if parse_target(target).is_none() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "target must be a valid YYYY-MM-DD date".into(),
                ));
            }
        }
        Kind::Timer => {
            if !input.text.is_empty() {
                validate_label(&input.text)?;
            }
            if let Some(duration) = input.duration_seconds {
                validate_duration(duration)?;
            }
        }
        Kind::Reminder => validate_body(&input.text, "reminder")?,
        Kind::Todo => validate_body(&input.text, "todo")?,
    }
    Ok(())
}

fn validate_update(sticker: &Sticker, input: &StickerUpdate) -> Result<(), (StatusCode, String)> {
    if let Some(x) = input.x {
        validate_x(x)?;
    }
    if let Some(y) = input.y {
        validate_y(y)?;
    }
    if let Some(text) = &input.text {
        match sticker.kind {
            Kind::Countdown | Kind::Timer => validate_label(text)?,
            Kind::Reminder | Kind::Todo => validate_body(text, "text")?,
        }
    }
    match sticker.kind {
        Kind::Countdown => {
            if input.target.is_some() {
                if let Some(target) = input.target.as_deref() {
                    if parse_target(target).is_none() {
                        return Err((
                            StatusCode::BAD_REQUEST,
                            "target must be a valid YYYY-MM-DD date".into(),
                        ));
                    }
                }
            }
            reject_timer_fields(input)?;
            if input.done.is_some() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "done only applies to todo stickers".into(),
                ));
            }
        }
        Kind::Timer => {
            if input.target.is_some() || input.done.is_some() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "target and done do not apply to timer stickers".into(),
                ));
            }
            if let Some(duration) = input.duration_seconds {
                validate_duration(duration)?;
            }
            if input.action == Some(Action::Set) && input.duration_seconds.is_none() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "action 'set' requires duration_seconds".into(),
                ));
            }
        }
        Kind::Reminder => {
            if input.target.is_some() || input.done.is_some() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "target and done do not apply to reminder stickers".into(),
                ));
            }
            reject_timer_fields(input)?;
        }
        Kind::Todo => {
            if input.target.is_some() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "target does not apply to todo stickers".into(),
                ));
            }
            reject_timer_fields(input)?;
        }
    }
    Ok(())
}

fn reject_timer_fields(input: &StickerUpdate) -> Result<(), (StatusCode, String)> {
    if input.action.is_some() || input.duration_seconds.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            "action and duration_seconds only apply to timer stickers".into(),
        ));
    }
    Ok(())
}

fn validate_x(x: f32) -> Result<(), (StatusCode, String)> {
    if x.is_finite() && (0.0..=960.0).contains(&x) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "x must be between 0 and 960".into(),
        ))
    }
}

fn validate_y(y: f32) -> Result<(), (StatusCode, String)> {
    if y.is_finite() && (0.0..=540.0).contains(&y) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "y must be between 0 and 540".into(),
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
            "label must be 1–48 characters with no control characters".into(),
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

fn apply_timer(
    sticker: &mut Sticker,
    action: Action,
    duration: Option<u64>,
    now: u64,
) -> Result<(), (StatusCode, String)> {
    if action == Action::Set && duration.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "action 'set' requires duration_seconds".into(),
        ));
    }
    if let Some(duration) = duration {
        validate_duration(duration)?;
        sticker.duration_seconds = duration;
        sticker.remaining_seconds = duration;
        sticker.running = false;
        sticker.updated_at = now;
    }
    let remaining = if sticker.running {
        sticker
            .remaining_seconds
            .saturating_sub(now.saturating_sub(sticker.updated_at))
    } else {
        sticker.remaining_seconds
    };
    match action {
        Action::Start => {
            sticker.remaining_seconds = if remaining == 0 {
                sticker.duration_seconds
            } else {
                remaining
            };
            sticker.running = true;
            sticker.updated_at = now;
        }
        Action::Pause => {
            sticker.remaining_seconds = remaining;
            sticker.running = false;
            sticker.updated_at = now;
        }
        Action::Reset | Action::Set => {
            sticker.remaining_seconds = sticker.duration_seconds;
            sticker.running = false;
            sticker.updated_at = now;
        }
    }
    Ok(())
}

fn view_of(sticker: &Sticker, now: u64, today: NaiveDate) -> StickerView {
    let days = sticker
        .target
        .as_deref()
        .and_then(|target| parse_target(target))
        .map(|target| (target - today).num_days());
    let (remaining_seconds, running, finished, state) = match sticker.kind {
        Kind::Timer => {
            let remaining = if sticker.running {
                sticker
                    .remaining_seconds
                    .saturating_sub(now.saturating_sub(sticker.updated_at))
            } else {
                sticker.remaining_seconds
            };
            let running = sticker.running && remaining > 0;
            let finished = remaining == 0;
            let state = if finished {
                "done"
            } else if running {
                "running"
            } else if remaining < sticker.duration_seconds {
                "paused"
            } else {
                "ready"
            };
            (remaining, running, finished, state)
        }
        Kind::Todo => (0, false, false, if sticker.done { "done" } else { "open" }),
        Kind::Countdown => {
            let state = match days {
                Some(days) if days < 0 => "past",
                Some(0) => "today",
                Some(_) => "upcoming",
                None => "invalid",
            };
            (0, false, false, state)
        }
        Kind::Reminder => (0, false, false, "reminder"),
    };
    StickerView {
        id: sticker.id,
        kind: sticker.kind,
        x: sticker.x,
        y: sticker.y,
        text: sticker.text.clone(),
        target: sticker.target.clone(),
        days,
        duration_seconds: sticker.duration_seconds,
        remaining_seconds,
        running,
        finished,
        state,
        done: sticker.done,
    }
}

fn valid_position(x: f32, y: f32) -> bool {
    x.is_finite() && y.is_finite() && (0.0..=960.0).contains(&x) && (0.0..=540.0).contains(&y)
}

fn valid_text(text: &str, max: usize, allow_newlines: bool) -> bool {
    !text.trim().is_empty()
        && text.chars().count() <= max
        && text
            .chars()
            .all(|char| !char.is_control() || (allow_newlines && char == '\n'))
}

fn parse_target(target: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(target, "%Y-%m-%d").ok()
}

fn sticker_size(kind: Kind, _text: &str) -> (f32, f32) {
    match kind {
        Kind::Timer => (TIMER_WIDTH, TIMER_HEIGHT),
        Kind::Countdown => (COUNTDOWN_WIDTH, COUNTDOWN_HEIGHT),
        Kind::Reminder => (REMINDER_WIDTH, REMINDER_HEIGHT),
        Kind::Todo => (TODO_WIDTH, TODO_HEIGHT),
    }
}

fn clamp_position(kind: Kind, text: &str, x: f32, y: f32) -> (f32, f32) {
    let (width, height) = sticker_size(kind, text);
    (
        x.clamp(3.0, 960.0 - width - 3.0),
        y.clamp(3.0, 540.0 - height - 3.0),
    )
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
    let view = store.add(input)?;
    Ok((StatusCode::CREATED, Json(view)))
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

pub async fn get_timer(
    State(store): State<SharedStickers>,
) -> Result<Json<StickerView>, (StatusCode, String)> {
    store
        .timer_view()
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "no timer sticker".into()))
}

pub async fn set_timer(
    State(store): State<SharedStickers>,
    Json(input): Json<TimerRequest>,
) -> Result<Json<StickerView>, (StatusCode, String)> {
    Ok(Json(store.apply_timer_request(input)?))
}

pub fn routes(store: SharedStickers) -> axum::Router {
    axum::Router::new()
        .route("/api/stickers", axum::routing::get(list).post(add))
        .route(
            "/api/stickers/{id}",
            axum::routing::patch(update).delete(remove),
        )
        .route("/api/timer", axum::routing::get(get_timer).post(set_timer))
        .with_state(store)
}

pub(crate) fn ink() -> Color {
    Color::from_rgba(42, 36, 28, 255)
}

pub(crate) fn muted_ink() -> Color {
    Color::from_rgba(112, 94, 74, 255)
}

pub(crate) fn deep_green() -> Color {
    Color::from_rgba(52, 138, 68, 255)
}

fn tomato() -> Color {
    Color::from_rgba(226, 76, 58, 255)
}

fn tomato_dark() -> Color {
    Color::from_rgba(188, 50, 42, 255)
}

fn lemon() -> Color {
    Color::from_rgba(255, 214, 72, 255)
}

fn peach() -> Color {
    Color::from_rgba(255, 176, 142, 255)
}

fn peach_dark() -> Color {
    Color::from_rgba(226, 135, 112, 255)
}

fn apple() -> Color {
    Color::from_rgba(145, 205, 106, 255)
}

fn leaf() -> Color {
    Color::from_rgba(86, 172, 82, 255)
}

fn cream() -> Color {
    Color::from_rgba(255, 248, 225, 255)
}

fn paper() -> Color {
    Color::from_rgba(255, 253, 246, 255)
}

fn blush() -> Color {
    Color::from_rgba(255, 132, 130, 150)
}

fn sticker_shadow() -> Color {
    Color::from_rgba(20, 18, 14, 70)
}

fn stem() -> Color {
    Color::from_rgba(92, 72, 48, 255)
}

#[derive(Clone, Copy)]
struct Oval {
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    rotation: f32,
}

fn draw_blob(ovals: &[Oval], fill: Color) {
    for oval in ovals {
        draw_ellipse(
            oval.cx + 6.0,
            oval.cy + 9.0,
            oval.rx + 8.0,
            oval.ry + 8.0,
            oval.rotation,
            sticker_shadow(),
        );
    }
    for oval in ovals {
        draw_ellipse(
            oval.cx,
            oval.cy,
            oval.rx + 8.0,
            oval.ry + 8.0,
            oval.rotation,
            paper(),
        );
    }
    for oval in ovals {
        draw_ellipse(oval.cx, oval.cy, oval.rx, oval.ry, oval.rotation, fill);
    }
}

fn draw_leaves(leaves: &[Oval], fill: Color) {
    for leaf in leaves {
        draw_ellipse(
            leaf.cx + 3.0,
            leaf.cy + 5.0,
            leaf.rx + 4.0,
            leaf.ry + 4.0,
            leaf.rotation,
            sticker_shadow(),
        );
    }
    for leaf in leaves {
        draw_ellipse(
            leaf.cx,
            leaf.cy,
            leaf.rx + 4.0,
            leaf.ry + 4.0,
            leaf.rotation,
            paper(),
        );
    }
    for leaf in leaves {
        draw_ellipse(leaf.cx, leaf.cy, leaf.rx, leaf.ry, leaf.rotation, fill);
    }
}

fn draw_stem(cx: f32, cy: f32, height: f32) {
    draw_line(cx, cy, cx, cy - height, 7.0, paper());
    draw_line(cx, cy, cx, cy - height, 3.0, stem());
}

fn draw_gloss(cx: f32, cy: f32, rx: f32, ry: f32, rotation: f32, alpha: f32) {
    draw_ellipse(cx, cy, rx, ry, rotation, Color::new(1.0, 1.0, 1.0, alpha));
}

fn draw_face(cx: f32, cy: f32, scale: f32) {
    let eye_dx = 17.0 * scale;
    draw_ellipse(cx - eye_dx, cy, 3.2 * scale, 5.0 * scale, 0.0, ink());
    draw_ellipse(cx + eye_dx, cy, 3.2 * scale, 5.0 * scale, 0.0, ink());
    draw_circle(cx - 30.0 * scale, cy + 9.0 * scale, 5.0 * scale, blush());
    draw_circle(cx + 30.0 * scale, cy + 9.0 * scale, 5.0 * scale, blush());
    draw_arc(
        cx,
        cy + 3.0 * scale,
        16,
        8.0 * scale,
        0.0,
        2.2 * scale,
        180.0,
        ink(),
    );
}

fn draw_centered(text: &str, cx: f32, baseline: f32, size: f32, color: Color) {
    let width = measure_text(text, None, size as u16, 1.0).width;
    draw_text(text, cx - width / 2.0, baseline, size, color);
}

fn draw_centered_lines(
    lines: &[String],
    cx: f32,
    first_baseline: f32,
    size: f32,
    line_height: f32,
    color: Color,
) {
    for (index, line) in lines.iter().enumerate() {
        draw_centered(
            line,
            cx,
            first_baseline + index as f32 * line_height,
            size,
            color,
        );
    }
}

pub(crate) fn sticker_card(x: f32, y: f32, width: f32, height: f32, fill: Color) {
    let radius = height / 2.0;
    let border = 6.0;
    let cy = y + radius;
    let sx = x + 5.0;
    let sy = y + 7.0;
    draw_rectangle(
        sx + radius,
        sy,
        width - 2.0 * radius,
        height,
        sticker_shadow(),
    );
    draw_circle(sx + radius, sy + radius, radius, sticker_shadow());
    draw_circle(sx + width - radius, sy + radius, radius, sticker_shadow());

    let outer = radius + border;
    draw_rectangle(
        x + radius,
        cy - outer,
        width - 2.0 * radius,
        outer * 2.0,
        paper(),
    );
    draw_circle(x + radius, cy, outer, paper());
    draw_circle(x + width - radius, cy, outer, paper());

    draw_rectangle(x + radius, y, width - 2.0 * radius, height, fill);
    draw_circle(x + radius, cy, radius, fill);
    draw_circle(x + width - radius, cy, radius, fill);
}

pub(crate) fn mini_fruit(cx: f32, cy: f32, radius: f32, fill: Color) {
    draw_circle(cx + 2.0, cy + 3.0, radius + 3.0, sticker_shadow());
    draw_circle(cx, cy, radius + 3.0, paper());
    draw_circle(cx, cy, radius, fill);
    draw_gloss(
        cx - radius * 0.35,
        cy - radius * 0.35,
        radius * 0.24,
        radius * 0.14,
        -20.0,
        0.35,
    );
    draw_stem(cx + radius * 0.25, cy - radius * 0.82, radius * 0.38);
    draw_leaves(
        &[Oval {
            cx: cx + radius * 0.55,
            cy: cy - radius * 0.92,
            rx: radius * 0.44,
            ry: radius * 0.18,
            rotation: -28.0,
        }],
        leaf(),
    );
}

pub fn draw(views: &[StickerView]) {
    for view in views {
        match view.kind {
            Kind::Timer => draw_timer(view),
            Kind::Countdown => draw_countdown(view),
            Kind::Reminder => draw_reminder(view),
            Kind::Todo => draw_todo(view),
        }
    }
}

fn draw_timer(view: &StickerView) {
    let width = TIMER_WIDTH;
    let height = TIMER_HEIGHT;
    let x = view.x.clamp(3.0, 960.0 - width - 3.0);
    let y = view.y.clamp(3.0, 540.0 - height - 3.0);
    let cx = x + width / 2.0;
    let cy = y + height / 2.0 + 4.0;

    draw_blob(
        &[Oval {
            cx,
            cy,
            rx: 66.0,
            ry: 60.0,
            rotation: -4.0,
        }],
        tomato(),
    );
    draw_gloss(cx - 22.0, cy - 26.0, 20.0, 10.0, -18.0, 0.25);
    draw_stem(cx, cy - 56.0, 15.0);
    draw_leaves(
        &[
            Oval {
                cx: cx - 20.0,
                cy: cy - 64.0,
                rx: 16.0,
                ry: 7.0,
                rotation: -28.0,
            },
            Oval {
                cx: cx + 20.0,
                cy: cy - 64.0,
                rx: 16.0,
                ry: 7.0,
                rotation: 28.0,
            },
            Oval {
                cx: cx - 7.0,
                cy: cy - 70.0,
                rx: 14.0,
                ry: 6.0,
                rotation: -55.0,
            },
            Oval {
                cx: cx + 7.0,
                cy: cy - 70.0,
                rx: 14.0,
                ry: 6.0,
                rotation: 55.0,
            },
            Oval {
                cx,
                cy: cy - 74.0,
                rx: 12.0,
                ry: 6.0,
                rotation: 0.0,
            },
        ],
        leaf(),
    );

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
    let fraction = if view.duration_seconds == 0 {
        0.0
    } else {
        view.remaining_seconds as f32 / view.duration_seconds as f32
    };
    if fraction > 0.0 {
        draw_arc(
            cx,
            cy + 6.0,
            40,
            45.0,
            -90.0,
            3.0,
            360.0 * fraction,
            tomato_dark(),
        );
    }

    let label = match view.state {
        "running" => "FOCUS",
        "paused" => "PAUSED",
        "done" => "DONE",
        _ => "READY",
    };
    let label_color = if view.state == "running" || view.state == "done" {
        deep_green()
    } else {
        muted_ink()
    };
    draw_centered(label, cx, cy - 8.0, 10.0, label_color);
    draw_centered(&clock(view.remaining_seconds), cx, cy + 22.0, 26.0, ink());
}

fn draw_countdown(view: &StickerView) {
    let width = COUNTDOWN_WIDTH;
    let height = COUNTDOWN_HEIGHT;
    let x = view.x.clamp(3.0, 960.0 - width - 3.0);
    let y = view.y.clamp(3.0, 540.0 - height - 3.0);
    let cx = x + width / 2.0;
    let cy = y + 72.0;

    draw_blob(
        &[
            Oval {
                cx,
                cy,
                rx: 80.0,
                ry: 54.0,
                rotation: -8.0,
            },
            Oval {
                cx: cx - 72.0,
                cy: cy + 12.0,
                rx: 20.0,
                ry: 18.0,
                rotation: -18.0,
            },
            Oval {
                cx: cx + 72.0,
                cy: cy - 12.0,
                rx: 20.0,
                ry: 18.0,
                rotation: 18.0,
            },
        ],
        lemon(),
    );
    draw_gloss(cx - 28.0, cy - 22.0, 24.0, 10.0, -16.0, 0.35);
    draw_stem(cx + 24.0, cy - 52.0, 10.0);
    draw_leaves(
        &[Oval {
            cx: cx + 34.0,
            cy: cy - 58.0,
            rx: 18.0,
            ry: 8.0,
            rotation: -28.0,
        }],
        leaf(),
    );
    draw_face(cx, cy - 10.0, 0.9);

    let banner_x = x + 10.0;
    let banner_y = y + 120.0;
    let banner_width = width - 20.0;
    let banner_height = 60.0;
    sticker_card(banner_x, banner_y, banner_width, banner_height, cream());

    let days = view.days.unwrap_or(0);
    let (big, unit) = if days > 0 {
        (days.to_string(), if days == 1 { "DAY" } else { "DAYS" })
    } else if days == 0 {
        ("TODAY".to_owned(), "")
    } else {
        (
            (-days).to_string(),
            if days == -1 { "DAY PAST" } else { "DAYS PAST" },
        )
    };
    let big_line = if unit.is_empty() {
        big
    } else {
        format!("{big} {unit}")
    };
    draw_centered(&big_line, cx, banner_y + 28.0, 20.0, ink());
    let label = wrap(&view.text.to_uppercase(), banner_width - 20.0, 10, 2);
    draw_centered_lines(&label, cx, banner_y + 43.0, 10.0, 12.0, ink());
}

fn draw_reminder(view: &StickerView) {
    let width = REMINDER_WIDTH;
    let height = REMINDER_HEIGHT;
    let x = view.x.clamp(3.0, 960.0 - width - 3.0);
    let y = view.y.clamp(3.0, 540.0 - height - 3.0);
    let cx = x + width / 2.0;
    let cy = y + height / 2.0;

    draw_blob(
        &[Oval {
            cx,
            cy,
            rx: 120.0,
            ry: 58.0,
            rotation: 2.0,
        }],
        peach(),
    );
    draw_line(cx, cy - 52.0, cx, cy - 30.0, 2.0, peach_dark());
    draw_gloss(cx - 48.0, cy - 30.0, 26.0, 10.0, -15.0, 0.35);
    draw_stem(cx + 30.0, cy - 52.0, 10.0);
    draw_leaves(
        &[Oval {
            cx: cx + 42.0,
            cy: cy - 54.0,
            rx: 20.0,
            ry: 8.0,
            rotation: -28.0,
        }],
        leaf(),
    );

    let lines = wrap(&view.text, 190.0, 17, 3);
    let first_baseline = cy - lines.len() as f32 * 20.0 / 2.0 + 17.0 * 0.8;
    draw_centered_lines(&lines, cx, first_baseline, 17.0, 20.0, ink());
}

fn draw_todo(view: &StickerView) {
    let width = TODO_WIDTH;
    let height = TODO_HEIGHT;
    let x = view.x.clamp(3.0, 960.0 - width - 3.0);
    let y = view.y.clamp(3.0, 540.0 - height - 3.0);
    let cx = x + width / 2.0;
    let cy = y + height / 2.0;

    draw_blob(
        &[
            Oval {
                cx: cx - 28.0,
                cy: cy + 2.0,
                rx: 74.0,
                ry: 58.0,
                rotation: -7.0,
            },
            Oval {
                cx: cx + 28.0,
                cy: cy + 2.0,
                rx: 74.0,
                ry: 58.0,
                rotation: 7.0,
            },
            Oval {
                cx,
                cy: cy + 12.0,
                rx: 82.0,
                ry: 50.0,
                rotation: 0.0,
            },
        ],
        apple(),
    );
    draw_stem(cx, cy - 52.0, 14.0);
    draw_leaves(
        &[Oval {
            cx: cx + 26.0,
            cy: cy - 56.0,
            rx: 20.0,
            ry: 7.0,
            rotation: -30.0,
        }],
        leaf(),
    );
    draw_gloss(cx - 42.0, cy - 28.0, 22.0, 9.0, -15.0, 0.3);

    let box_x = x + 34.0;
    draw_circle(box_x, cy, 15.0, paper());
    draw_circle_lines(box_x, cy, 15.0, 2.0, ink());
    if view.done {
        draw_line(box_x - 7.0, cy, box_x - 2.0, cy + 5.0, 3.0, deep_green());
        draw_line(
            box_x - 2.0,
            cy + 5.0,
            box_x + 7.0,
            cy - 6.0,
            3.0,
            deep_green(),
        );
    }

    let lines = wrap(&view.text, 170.0, 17, 3);
    let first_baseline = cy - lines.len() as f32 * 20.0 / 2.0 + 17.0 * 0.8;
    draw_centered_lines(&lines, x + 154.0, first_baseline, 17.0, 20.0, ink());
}

fn clock(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn wrap(text: &str, width: f32, size: u16, max_lines: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bronzecast-stickers-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn new_sticker(kind: Kind, x: f32, y: f32, text: &str) -> NewSticker {
        NewSticker {
            kind,
            x,
            y,
            text: text.into(),
            target: None,
            duration_seconds: None,
            done: false,
        }
    }

    #[test]
    fn seeds_a_ready_timer() {
        let dir = temp_dir();
        let store = Store::load(&dir);
        let views = store.snapshot_at(1_000, today());
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].kind, Kind::Timer);
        assert_eq!(views[0].state, "ready");
        assert_eq!(views[0].remaining_seconds, DEFAULT_DURATION);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn adds_all_sticker_kinds_and_reads_them() {
        let dir = temp_dir();
        let store = Store::load(&dir);
        let now = 1_000;
        let today = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();

        let mut countdown = new_sticker(Kind::Countdown, 476.0, 380.0, "til Halloween");
        countdown.target = Some("2026-10-31".into());
        let countdown = store.add_at(countdown, now, today).unwrap();
        assert_eq!(countdown.days, Some(7));
        assert_eq!(countdown.state, "upcoming");

        let reminder = store
            .add_at(
                new_sticker(Kind::Reminder, 700.0, 380.0, "Candy for Halloween"),
                now,
                today,
            )
            .unwrap();
        assert_eq!(reminder.text, "Candy for Halloween");

        let todo = store
            .add_at(
                new_sticker(Kind::Todo, 700.0, 460.0, "Ship sticker system"),
                now,
                today,
            )
            .unwrap();
        assert_eq!(todo.state, "open");

        let done = store
            .update_at(
                todo.id,
                StickerUpdate {
                    x: None,
                    y: None,
                    text: None,
                    target: None,
                    done: Some(true),
                    action: None,
                    duration_seconds: None,
                },
                now,
                today,
            )
            .unwrap();
        assert_eq!(done.state, "done");
        assert!(done.done);

        let views = store.snapshot_at(now, today);
        assert_eq!(views.len(), 4);
        assert!(views.iter().any(|view| view.kind == Kind::Countdown));
        assert!(views.iter().any(|view| view.kind == Kind::Reminder));
        assert!(views.iter().any(|view| view.kind == Kind::Todo));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn moves_stickers_and_clamps_them_on_the_board() {
        let dir = temp_dir();
        let store = Store::load(&dir);
        let now = 1_000;
        let today = today();
        let sticker = store
            .add_at(
                new_sticker(Kind::Reminder, 10.0, 10.0, "Move me"),
                now,
                today,
            )
            .unwrap();
        let moved = store
            .update_at(
                sticker.id,
                StickerUpdate {
                    x: Some(959.0),
                    y: Some(539.0),
                    text: None,
                    target: None,
                    done: None,
                    action: None,
                    duration_seconds: None,
                },
                now,
                today,
            )
            .unwrap();
        assert!(moved.x < 959.0);
        assert!(moved.y < 539.0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn timer_actions_and_persistence() {
        let dir = temp_dir();
        let today = today();
        {
            let store = Store::load(&dir);
            let timer = store
                .apply_timer_request_at(
                    TimerRequest {
                        action: Action::Start,
                        duration_seconds: Some(10),
                        x: Some(100.0),
                        y: Some(120.0),
                    },
                    1_000,
                    today,
                )
                .unwrap();
            assert_eq!(timer.remaining_seconds, 10);
            assert!(timer.running);
            assert_eq!(timer.x, 100.0);
        }
        let store = Store::load(&dir);
        let view = store.timer_view_at(1_004, today).unwrap();
        assert_eq!(view.remaining_seconds, 6);
        assert!(view.running);

        let paused = store
            .apply_timer_request_at(
                TimerRequest {
                    action: Action::Pause,
                    duration_seconds: None,
                    x: None,
                    y: None,
                },
                1_004,
                today,
            )
            .unwrap();
        assert_eq!(paused.remaining_seconds, 6);
        assert_eq!(paused.state, "paused");

        let finished = store
            .apply_timer_request_at(
                TimerRequest {
                    action: Action::Start,
                    duration_seconds: None,
                    x: None,
                    y: None,
                },
                2_000,
                today,
            )
            .unwrap();
        assert!(finished.running);
        let done = store.timer_view_at(2_006, today).unwrap();
        assert_eq!(done.state, "done");

        let reset = store
            .apply_timer_request_at(
                TimerRequest {
                    action: Action::Reset,
                    duration_seconds: None,
                    x: None,
                    y: None,
                },
                2_006,
                today,
            )
            .unwrap();
        assert_eq!(reset.state, "ready");
        assert_eq!(reset.remaining_seconds, 10);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn migrates_the_legacy_timer() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("timer.json"),
            br#"{"duration_seconds":30,"remaining_seconds":30,"running":true,"updated_at":1000}"#,
        )
        .unwrap();
        let store = Store::load(&dir);
        let view = store
            .timer_view_at(1_005, NaiveDate::from_ymd_opt(2026, 10, 24).unwrap())
            .unwrap();
        assert_eq!(view.remaining_seconds, 25);
        assert!(view.running);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_invalid_stickers() {
        let dir = temp_dir();
        let store = Store::load(&dir);
        let today = today();

        let mut countdown = new_sticker(Kind::Countdown, 100.0, 100.0, "til Halloween");
        countdown.target = None;
        assert!(store.add_at(countdown, 1_000, today).is_err());

        let mut bad_date = new_sticker(Kind::Countdown, 100.0, 100.0, "til Halloween");
        bad_date.target = Some("soon".into());
        assert!(store.add_at(bad_date, 1_000, today).is_err());

        assert!(
            store
                .add_at(new_sticker(Kind::Reminder, 100.0, 100.0, ""), 1_000, today)
                .is_err()
        );
        assert!(
            store
                .add_at(
                    new_sticker(Kind::Todo, 100.0, 100.0, "x".repeat(161).as_str()),
                    1_000,
                    today,
                )
                .is_err()
        );
        assert!(
            store
                .add_at(
                    new_sticker(Kind::Reminder, 1000.0, 100.0, "edge"),
                    1_000,
                    today
                )
                .is_err()
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn removes_stickers() {
        let dir = temp_dir();
        let store = Store::load(&dir);
        let today = today();
        let sticker = store
            .add_at(
                new_sticker(Kind::Reminder, 100.0, 100.0, "Remove me"),
                1_000,
                today,
            )
            .unwrap();
        store.remove(sticker.id).unwrap();
        assert!(
            store
                .snapshot_at(1_000, today)
                .iter()
                .all(|view| view.id != sticker.id)
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn countdown_tracks_days() {
        let target = today() + Duration::days(7);
        assert_eq!(
            (target - today()).num_days(),
            7,
            "chrono date arithmetic should be stable"
        );
    }
}
