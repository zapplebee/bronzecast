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

const VERSION: u8 = 3;
const MAX_DURATION: u64 = 60 * 60;
const MAX_NOTIFICATION_TTL: u64 = 7 * 24 * 60 * 60;
const CANVAS_WIDTH: f32 = 960.0;
const CANVAS_HEIGHT: f32 = 540.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    JackOLantern,
    Ghost,
    BlackCat,
    Bat,
    CandyPail,
    Turkey,
    Cornucopia,
    MapleLeaf,
    PumpkinPie,
    ChristmasTree,
    WrappedGift,
    Snowman,
    Reindeer,
    GingerbreadPerson,
    CandyCane,
    Fireworks,
    LunarNewYearLantern,
    Diya,
    Heart,
    EasterBunny,
    DecoratedEgg,
    Tulip,
    Sun,
    IceCreamCone,
    CustomSvg,
}

impl Appearance {
    pub const ALL: [Self; 25] = [
        Self::JackOLantern,
        Self::Ghost,
        Self::BlackCat,
        Self::Bat,
        Self::CandyPail,
        Self::Turkey,
        Self::Cornucopia,
        Self::MapleLeaf,
        Self::PumpkinPie,
        Self::ChristmasTree,
        Self::WrappedGift,
        Self::Snowman,
        Self::Reindeer,
        Self::GingerbreadPerson,
        Self::CandyCane,
        Self::Fireworks,
        Self::LunarNewYearLantern,
        Self::Diya,
        Self::Heart,
        Self::EasterBunny,
        Self::DecoratedEgg,
        Self::Tulip,
        Self::Sun,
        Self::IceCreamCone,
        Self::CustomSvg,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Timer,
    Countdown,
    Todo,
    Memo,
    Notification,
    Clock,
    Art,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Sticker {
    Timer {
        id: u64,
        appearance: Appearance,
        x: f32,
        y: f32,
        angle: f32,
        duration_seconds: u64,
        remaining_at_anchor_seconds: u64,
        running_since: Option<u64>,
        has_started: bool,
        created_at: u64,
        updated_at: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        svg: Option<String>,
    },
    Countdown {
        id: u64,
        appearance: Appearance,
        x: f32,
        y: f32,
        angle: f32,
        text: String,
        target: String,
        created_at: u64,
        updated_at: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        svg: Option<String>,
    },
    Todo {
        id: u64,
        appearance: Appearance,
        x: f32,
        y: f32,
        angle: f32,
        text: String,
        done: bool,
        created_at: u64,
        updated_at: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        svg: Option<String>,
    },
    Memo {
        id: u64,
        appearance: Appearance,
        x: f32,
        y: f32,
        angle: f32,
        title: String,
        text: String,
        created_at: u64,
        updated_at: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        svg: Option<String>,
    },
    Notification {
        id: u64,
        appearance: Appearance,
        x: f32,
        y: f32,
        angle: f32,
        title: String,
        text: String,
        expires_at: u64,
        created_at: u64,
        updated_at: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        svg: Option<String>,
    },
    Clock {
        id: u64,
        appearance: Appearance,
        x: f32,
        y: f32,
        angle: f32,
        created_at: u64,
        updated_at: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        svg: Option<String>,
    },
    Art {
        id: u64,
        appearance: Appearance,
        x: f32,
        y: f32,
        angle: f32,
        svg: String,
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
            | Self::Memo { id, .. }
            | Self::Notification { id, .. }
            | Self::Clock { id, .. }
            | Self::Art { id, .. } => *id,
        }
    }

    fn kind(&self) -> Kind {
        match self {
            Self::Timer { .. } => Kind::Timer,
            Self::Countdown { .. } => Kind::Countdown,
            Self::Todo { .. } => Kind::Todo,
            Self::Memo { .. } => Kind::Memo,
            Self::Notification { .. } => Kind::Notification,
            Self::Clock { .. } => Kind::Clock,
            Self::Art { .. } => Kind::Art,
        }
    }

    fn appearance(&self) -> Appearance {
        match self {
            Self::Timer { appearance, .. }
            | Self::Countdown { appearance, .. }
            | Self::Todo { appearance, .. }
            | Self::Memo { appearance, .. }
            | Self::Notification { appearance, .. }
            | Self::Clock { appearance, .. }
            | Self::Art { appearance, .. } => *appearance,
        }
    }

    fn set_appearance(&mut self, value: Appearance) {
        match self {
            Self::Timer {
                appearance, svg, ..
            }
            | Self::Countdown {
                appearance, svg, ..
            }
            | Self::Todo {
                appearance, svg, ..
            }
            | Self::Memo {
                appearance, svg, ..
            }
            | Self::Notification {
                appearance, svg, ..
            }
            | Self::Clock {
                appearance, svg, ..
            } => {
                *appearance = value;
                if value != Appearance::CustomSvg {
                    *svg = None;
                }
            }
            Self::Art { appearance, .. } => *appearance = value,
        }
    }

    fn svg(&self) -> Option<&str> {
        match self {
            Self::Timer { svg, .. }
            | Self::Countdown { svg, .. }
            | Self::Todo { svg, .. }
            | Self::Memo { svg, .. }
            | Self::Notification { svg, .. }
            | Self::Clock { svg, .. } => svg.as_deref(),
            Self::Art { svg, .. } => Some(svg),
        }
    }

    fn set_svg(&mut self, value: String) {
        match self {
            Self::Timer { svg, .. }
            | Self::Countdown { svg, .. }
            | Self::Todo { svg, .. }
            | Self::Memo { svg, .. }
            | Self::Notification { svg, .. }
            | Self::Clock { svg, .. } => *svg = Some(value),
            Self::Art { svg, .. } => *svg = value,
        }
    }

    fn position(&self) -> (f32, f32, f32) {
        match self {
            Self::Timer { x, y, angle, .. }
            | Self::Countdown { x, y, angle, .. }
            | Self::Todo { x, y, angle, .. }
            | Self::Memo { x, y, angle, .. }
            | Self::Notification { x, y, angle, .. }
            | Self::Clock { x, y, angle, .. }
            | Self::Art { x, y, angle, .. } => (*x, *y, *angle),
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
            | Self::Memo {
                created_at,
                updated_at,
                ..
            }
            | Self::Notification {
                created_at,
                updated_at,
                ..
            }
            | Self::Clock {
                created_at,
                updated_at,
                ..
            }
            | Self::Art {
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
            | Self::Memo {
                x: sx,
                y: sy,
                angle: sa,
                ..
            }
            | Self::Notification {
                x: sx,
                y: sy,
                angle: sa,
                ..
            }
            | Self::Clock {
                x: sx,
                y: sy,
                angle: sa,
                ..
            }
            | Self::Art {
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
            | Self::Memo { updated_at, .. }
            | Self::Notification { updated_at, .. }
            | Self::Clock { updated_at, .. }
            | Self::Art { updated_at, .. } => *updated_at = value,
        }
    }

    fn valid(&self) -> bool {
        let (x, y, angle) = self.position();
        self.id() > 0
            && Appearance::ALL.contains(&self.appearance())
            && x.is_finite()
            && y.is_finite()
            && angle.is_finite()
            && (-180.0..180.0).contains(&angle)
            && self.valid_svg()
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
                        && validate_stored_position(self.appearance(), x, y, angle)
                }
                Self::Countdown { text, target, .. } => {
                    valid_text(text, 48, false)
                        && parse_target(target).is_some()
                        && validate_stored_position(self.appearance(), x, y, angle)
                }
                Self::Todo { text, .. } => {
                    valid_text(text, 160, true)
                        && validate_stored_position(self.appearance(), x, y, angle)
                }
                Self::Memo { title, text, .. } => {
                    valid_optional_text(title, 48, false)
                        && valid_optional_text(text, 160, true)
                        && (!title.trim().is_empty() || !text.trim().is_empty())
                        && validate_stored_position(self.appearance(), x, y, angle)
                }
                Self::Notification {
                    title,
                    text,
                    expires_at,
                    created_at,
                    ..
                } => {
                    valid_text(title, 48, false)
                        && valid_text(text, 320, true)
                        && *expires_at > *created_at
                        && *expires_at - *created_at <= MAX_NOTIFICATION_TTL
                        && validate_stored_position(self.appearance(), x, y, angle)
                }
                Self::Clock { .. } => validate_stored_position(self.appearance(), x, y, angle),
                Self::Art { .. } => {
                    self.appearance() == Appearance::CustomSvg
                        && validate_stored_position(self.appearance(), x, y, angle)
                }
            }
    }

    fn valid_svg(&self) -> bool {
        match (self.appearance(), self.svg()) {
            (Appearance::CustomSvg, Some(svg)) => crate::svg_art::validate(svg).is_ok(),
            (Appearance::CustomSvg, None) => false,
            (_, None) => true,
            (_, Some(_)) => false,
        }
    }

    fn view(&self, now: u64, today: NaiveDate) -> StickerView {
        let (x, y, angle) = self.position();
        let (created_at, updated_at) = self.timestamps();
        let mut view = StickerView::base(
            self.id(),
            self.kind(),
            self.appearance(),
            x,
            y,
            angle,
            created_at,
            updated_at,
        );
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
            Self::Memo { title, text, .. } => {
                if !title.is_empty() {
                    view.title = Some(title.clone());
                }
                if !text.is_empty() {
                    view.text = Some(text.clone());
                }
                view.state = "memo";
            }
            Self::Notification {
                title,
                text,
                expires_at,
                ..
            } => {
                view.title = Some(title.clone());
                view.text = Some(text.clone());
                view.expires_at = Some(*expires_at);
                view.state = "notification";
            }
            Self::Clock { .. } => view.state = "clock",
            Self::Art { svg, .. } => {
                view.svg = Some(svg.clone());
                view.state = "art";
            }
        }
        if self.appearance() == Appearance::CustomSvg && self.kind() != Kind::Art {
            view.svg = self.svg().map(str::to_owned);
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
    pub appearance: Appearance,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub svg: Option<String>,
}

impl StickerView {
    fn base(
        id: u64,
        kind: Kind,
        appearance: Appearance,
        x: f32,
        y: f32,
        angle: f32,
        created_at: u64,
        updated_at: u64,
    ) -> Self {
        Self {
            id,
            kind,
            appearance,
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
            expires_at: None,
            svg: None,
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
    appearance: Appearance,
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
    #[serde(default)]
    expires_in_seconds: Option<u64>,
    #[serde(default)]
    svg: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StickerUpdate {
    #[serde(default)]
    appearance: Option<Appearance>,
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
    #[serde(default)]
    svg: Option<String>,
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
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (1, Vec::new()),
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
        let mut inner = self.inner.lock().expect("sticker lock");
        let existing = inner.stickers.len();
        inner.stickers.retain(|sticker| {
            !matches!(sticker, Sticker::Notification { expires_at, .. } if *expires_at <= now)
        });
        if inner.stickers.len() != existing
            && let Err((_, error)) = self.write(&inner)
        {
            eprintln!("could not persist expired sticker cleanup: {error}");
        }
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
        let x = input.x.expect("validated sticker x");
        let y = input.y.expect("validated sticker y");
        let appearance = input.appearance;
        let angle = normalize_angle(input.angle.unwrap_or(0.0));
        let svg = if appearance == Appearance::CustomSvg {
            Some(input.svg.clone().expect("validated custom SVG"))
        } else {
            None
        };
        let mut sticker = match input.kind {
            Kind::Timer => {
                let duration = input.duration_seconds.expect("validated timer duration");
                Sticker::Timer {
                    id,
                    appearance,
                    x,
                    y,
                    angle,
                    duration_seconds: duration,
                    remaining_at_anchor_seconds: duration,
                    running_since: None,
                    has_started: false,
                    created_at: now,
                    updated_at: now,
                    svg: svg.clone(),
                }
            }
            Kind::Countdown => Sticker::Countdown {
                id,
                appearance,
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
                svg: svg.clone(),
            },
            Kind::Todo => Sticker::Todo {
                id,
                appearance,
                x,
                y,
                angle,
                text: input.text.expect("validated todo text").trim().to_owned(),
                done: input.done.unwrap_or(false),
                created_at: now,
                updated_at: now,
                svg: svg.clone(),
            },
            Kind::Memo => Sticker::Memo {
                id,
                appearance,
                x,
                y,
                angle,
                title: input.title.unwrap_or_default().trim().to_owned(),
                text: input.text.unwrap_or_default().trim().to_owned(),
                created_at: now,
                updated_at: now,
                svg: svg.clone(),
            },
            Kind::Notification => Sticker::Notification {
                id,
                appearance,
                x,
                y,
                angle,
                title: input
                    .title
                    .unwrap_or_else(|| "Notification".to_owned())
                    .trim()
                    .to_owned(),
                text: input
                    .text
                    .expect("validated notification text")
                    .trim()
                    .to_owned(),
                expires_at: now.saturating_add(
                    input
                        .expires_in_seconds
                        .expect("validated notification TTL"),
                ),
                created_at: now,
                updated_at: now,
                svg: svg.clone(),
            },
            Kind::Clock => Sticker::Clock {
                id,
                appearance,
                x,
                y,
                angle,
                created_at: now,
                updated_at: now,
                svg: svg.clone(),
            },
            Kind::Art => Sticker::Art {
                id,
                appearance,
                x,
                y,
                angle,
                svg: input.svg.expect("validated art SVG"),
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
        if let Some(appearance) = input.appearance {
            next.set_appearance(appearance);
        }
        if let Some(svg) = input.svg {
            next.set_svg(svg);
        }
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
            Sticker::Memo { title, text, .. } => {
                if let Some(value) = input.title {
                    *title = value.trim().to_owned();
                }
                if let Some(value) = input.text {
                    *text = value.trim().to_owned();
                }
            }
            Sticker::Notification { title, text, .. } => {
                if let Some(value) = input.title {
                    *title = value.trim().to_owned();
                }
                if let Some(value) = input.text {
                    *text = value.trim().to_owned();
                }
            }
            Sticker::Clock { .. } => {}
            Sticker::Art { .. } => {}
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

fn dimensions(_appearance: Appearance) -> (f32, f32) {
    (260.0, 140.0)
}

fn clamp_sticker(sticker: &mut Sticker) {
    let (x, y, angle) = sticker.position();
    let (x, y) = clamp_position(sticker.appearance(), x, y, angle);
    sticker.set_position(x, y, angle);
}

fn clamp_position(appearance: Appearance, x: f32, y: f32, angle: f32) -> (f32, f32) {
    let (width, height) = dimensions(appearance);
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

fn validate_stored_position(appearance: Appearance, x: f32, y: f32, angle: f32) -> bool {
    let (clamped_x, clamped_y) = clamp_position(appearance, x, y, angle);
    (x - clamped_x).abs() < 0.01 && (y - clamped_y).abs() < 0.01
}

fn normalize_angle(angle: f32) -> f32 {
    (angle + 180.0).rem_euclid(360.0) - 180.0
}

fn validate_new(input: &NewSticker) -> Result<(), (StatusCode, String)> {
    let x = input
        .x
        .ok_or((StatusCode::BAD_REQUEST, "x is required".into()))?;
    let y = input
        .y
        .ok_or((StatusCode::BAD_REQUEST, "y is required".into()))?;
    validate_coordinate(x, "x", CANVAS_WIDTH)?;
    validate_coordinate(y, "y", CANVAS_HEIGHT)?;
    if input.angle.is_some_and(|angle| !angle.is_finite()) {
        return Err((StatusCode::BAD_REQUEST, "angle must be finite".into()));
    }
    validate_svg_pair(input.appearance, input.svg.as_deref())?;
    match input.kind {
        Kind::Timer => {
            reject_fields(
                input.title.is_some()
                    || input.text.is_some()
                    || input.target.is_some()
                    || input.done.is_some()
                    || input.expires_in_seconds.is_some(),
                "timer accepts only duration_seconds in addition to position and angle",
            )?;
            let duration = input.duration_seconds.ok_or((
                StatusCode::BAD_REQUEST,
                "timer requires duration_seconds".into(),
            ))?;
            validate_duration(duration)?;
        }
        Kind::Countdown => {
            reject_fields(
                input.title.is_some()
                    || input.duration_seconds.is_some()
                    || input.done.is_some()
                    || input.expires_in_seconds.is_some(),
                "countdown does not accept title, duration_seconds, done, or expiry",
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
                input.title.is_some()
                    || input.target.is_some()
                    || input.duration_seconds.is_some()
                    || input.expires_in_seconds.is_some(),
                "todo does not accept title, target, duration_seconds, or expiry",
            )?;
            validate_body(input.text.as_deref().unwrap_or(""), "todo")?;
        }
        Kind::Memo => {
            reject_fields(
                input.target.is_some()
                    || input.duration_seconds.is_some()
                    || input.done.is_some()
                    || input.expires_in_seconds.is_some(),
                "memo does not accept target, duration_seconds, or done",
            )?;
            validate_memo(
                input.title.as_deref().unwrap_or(""),
                input.text.as_deref().unwrap_or(""),
            )?;
        }
        Kind::Notification => {
            reject_fields(
                input.target.is_some() || input.duration_seconds.is_some() || input.done.is_some(),
                "notification accepts only title, text, expiry, position, and angle",
            )?;
            if let Some(title) = input.title.as_deref() {
                validate_notification_title(title)?;
            }
            validate_notification_text(input.text.as_deref().unwrap_or(""))?;
            let ttl = input.expires_in_seconds.ok_or((
                StatusCode::BAD_REQUEST,
                "notification requires expires_in_seconds".into(),
            ))?;
            validate_notification_ttl(ttl)?;
        }
        Kind::Clock => {
            reject_fields(
                input.title.is_some()
                    || input.text.is_some()
                    || input.target.is_some()
                    || input.duration_seconds.is_some()
                    || input.done.is_some()
                    || input.expires_in_seconds.is_some(),
                "clock accepts only appearance, position, and angle",
            )?;
        }
        Kind::Art => {
            reject_fields(
                input.title.is_some()
                    || input.text.is_some()
                    || input.target.is_some()
                    || input.duration_seconds.is_some()
                    || input.done.is_some()
                    || input.expires_in_seconds.is_some(),
                "art accepts only custom SVG artwork, position, and angle",
            )?;
            if input.appearance != Appearance::CustomSvg {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "art requires appearance 'custom_svg'".into(),
                ));
            }
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
    let appearance = input.appearance.unwrap_or_else(|| sticker.appearance());
    if sticker.kind() == Kind::Art && appearance != Appearance::CustomSvg {
        return Err((
            StatusCode::BAD_REQUEST,
            "art stickers require appearance 'custom_svg'".into(),
        ));
    }
    let next_svg = if appearance == Appearance::CustomSvg {
        input.svg.as_deref().or_else(|| sticker.svg())
    } else {
        None
    };
    if input.svg.is_some() && appearance != Appearance::CustomSvg {
        return Err((
            StatusCode::BAD_REQUEST,
            "svg is accepted only with appearance 'custom_svg'".into(),
        ));
    }
    validate_svg_pair(appearance, next_svg)?;
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
        Sticker::Memo { title, text, .. } => {
            reject_fields(
                input.target.is_some()
                    || input.done.is_some()
                    || input.action.is_some()
                    || input.duration_seconds.is_some(),
                "memo accepts only position, angle, title, and text",
            )?;
            let next_title = input.title.as_deref().unwrap_or(title);
            let next_text = input.text.as_deref().unwrap_or(text);
            validate_memo(next_title, next_text)?;
        }
        Sticker::Notification { .. } => {
            reject_fields(
                input.target.is_some()
                    || input.done.is_some()
                    || input.action.is_some()
                    || input.duration_seconds.is_some(),
                "notification accepts only position, angle, title, and text",
            )?;
            if let Some(value) = &input.title {
                validate_notification_title(value)?;
            }
            if let Some(value) = &input.text {
                validate_notification_text(value)?;
            }
        }
        Sticker::Clock { .. } => {
            reject_fields(
                input.title.is_some()
                    || input.text.is_some()
                    || input.target.is_some()
                    || input.done.is_some()
                    || input.action.is_some()
                    || input.duration_seconds.is_some(),
                "clock accepts only position and angle",
            )?;
        }
        Sticker::Art { .. } => {
            reject_fields(
                input.title.is_some()
                    || input.text.is_some()
                    || input.target.is_some()
                    || input.done.is_some()
                    || input.action.is_some()
                    || input.duration_seconds.is_some(),
                "art accepts only SVG artwork, position, and angle",
            )?;
        }
    }
    if input.x.is_none()
        && input.y.is_none()
        && input.angle.is_none()
        && input.appearance.is_none()
        && input.title.is_none()
        && input.text.is_none()
        && input.target.is_none()
        && input.done.is_none()
        && input.action.is_none()
        && input.duration_seconds.is_none()
        && input.svg.is_none()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "at least one sticker field must be provided".into(),
        ));
    }
    Ok(())
}

fn validate_svg_pair(
    appearance: Appearance,
    svg: Option<&str>,
) -> Result<(), (StatusCode, String)> {
    match (appearance, svg) {
        (Appearance::CustomSvg, None) => Err((
            StatusCode::BAD_REQUEST,
            "appearance 'custom_svg' requires svg artwork".into(),
        )),
        (Appearance::CustomSvg, Some(source)) => {
            crate::svg_art::validate(source).map_err(|error| (StatusCode::BAD_REQUEST, error))
        }
        (_, Some(_)) => Err((
            StatusCode::BAD_REQUEST,
            "svg is accepted only with appearance 'custom_svg'".into(),
        )),
        (_, None) => Ok(()),
    }
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

fn validate_notification_ttl(ttl: u64) -> Result<(), (StatusCode, String)> {
    if (1..=MAX_NOTIFICATION_TTL).contains(&ttl) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            format!("expires_in_seconds must be between 1 and {MAX_NOTIFICATION_TTL}"),
        ))
    }
}

fn validate_notification_title(title: &str) -> Result<(), (StatusCode, String)> {
    if valid_text(title, 48, false) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "notification title must be 1–48 characters with no control characters".into(),
        ))
    }
}

fn validate_notification_text(text: &str) -> Result<(), (StatusCode, String)> {
    if valid_text(text, 320, true) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "notification text must be 1–320 characters; only newlines are allowed as control characters".into(),
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

fn validate_memo(title: &str, text: &str) -> Result<(), (StatusCode, String)> {
    if !valid_optional_text(title, 48, false) {
        return Err((
            StatusCode::BAD_REQUEST,
            "memo title must be at most 48 characters with no control characters".into(),
        ));
    }
    if !valid_optional_text(text, 160, true) {
        return Err((StatusCode::BAD_REQUEST, "memo text must be at most 160 characters; only newlines are allowed as control characters".into()));
    }
    if title.trim().is_empty() && text.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "memo requires a non-empty title or text".into(),
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
    const TEST_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 260 140"><path d="M10 10h240v120H10z" fill="#e55"/></svg>"##;

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
            appearance: Appearance::JackOLantern,
            x: Some(100.0),
            y: Some(100.0),
            angle: None,
            title: None,
            text: text.map(str::to_owned),
            target: None,
            duration_seconds: None,
            done: None,
            expires_in_seconds: None,
            svg: None,
        }
    }

    fn timer_update(action: Action, duration_seconds: Option<u64>) -> StickerUpdate {
        StickerUpdate {
            appearance: None,
            x: None,
            y: None,
            angle: None,
            title: None,
            text: None,
            target: None,
            done: None,
            action: Some(action),
            duration_seconds,
            svg: None,
        }
    }

    #[test]
    fn empty_store_stays_empty_and_persists_tagged_memos() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let views = store.snapshot_at(1_000, NaiveDate::from_ymd_opt(2026, 10, 24).unwrap());
        assert!(views.is_empty());
        assert!(!dir.join("stickers.json").exists());

        let mut memo = input(Kind::Memo, Some("Body"));
        memo.title = Some("Title".into());
        let memo = store
            .add_at(memo, 1_000, NaiveDate::from_ymd_opt(2026, 10, 24).unwrap())
            .unwrap();
        assert_eq!(memo.kind, Kind::Memo);
        assert_eq!(memo.appearance, Appearance::JackOLantern);
        let raw = fs::read_to_string(dir.join("stickers.json")).unwrap();
        assert!(raw.contains("\"version\": 3"));
        assert!(raw.contains("\"kind\": \"memo\""));
        assert!(raw.contains("\"appearance\": \"jack_o_lantern\""));
        assert!(!raw.contains("\"kind\": \"timer\""));
        assert_eq!(Appearance::ALL.len(), 25);
        let reloaded = Store::load(&dir).unwrap();
        assert_eq!(reloaded.snapshot().len(), 1);

        let changed = reloaded
            .update_at(
                memo.id,
                StickerUpdate {
                    appearance: Some(Appearance::Ghost),
                    x: None,
                    y: None,
                    angle: None,
                    title: None,
                    text: None,
                    target: None,
                    done: None,
                    action: None,
                    duration_seconds: None,
                    svg: None,
                },
                1_001,
                NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
            )
            .unwrap();
        assert_eq!(changed.appearance, Appearance::Ghost);
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
                .add_at(input(Kind::Memo, Some("   ")), 1_000, today)
                .is_err()
        );
        let mut bad = input(Kind::Todo, Some("Task"));
        bad.target = Some("2026-10-31".into());
        assert!(store.add_at(bad, 1_000, today).is_err());
        assert!(
            store
                .add_at(input(Kind::Notification, Some("Temporary")), 1_000, today)
                .is_err()
        );
        assert!(
            store
                .add_at(input(Kind::Clock, Some("Static text")), 1_000, today)
                .is_err()
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn every_preset_and_custom_svg_appearance_works_with_supported_kinds() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let kinds = [
            Kind::Timer,
            Kind::Countdown,
            Kind::Todo,
            Kind::Memo,
            Kind::Notification,
            Kind::Clock,
            Kind::Art,
        ];
        for appearance in Appearance::ALL {
            for kind in kinds {
                if kind == Kind::Art && appearance != Appearance::CustomSvg {
                    continue;
                }
                let text = if matches!(kind, Kind::Timer | Kind::Clock | Kind::Art) {
                    None
                } else {
                    Some("Center text")
                };
                let mut sticker = input(kind, text);
                sticker.appearance = appearance;
                if appearance == Appearance::CustomSvg {
                    sticker.svg = Some(TEST_SVG.into());
                }
                match kind {
                    Kind::Timer => sticker.duration_seconds = Some(600),
                    Kind::Countdown => sticker.target = Some("2026-10-31".into()),
                    Kind::Notification => sticker.expires_in_seconds = Some(3_600),
                    Kind::Todo | Kind::Memo | Kind::Clock | Kind::Art => {}
                }
                let view = store.add_at(sticker, 1_000, today).unwrap();
                assert_eq!(view.kind, kind);
                assert_eq!(view.appearance, appearance);
            }
        }
        assert_eq!(store.snapshot_at(1_000, today).len(), 24 * 6 + 7);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn custom_svg_is_persisted_updated_and_cleared_when_selecting_a_preset() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let today = today();

        let mut art = input(Kind::Art, None);
        art.appearance = Appearance::CustomSvg;
        art.svg = Some(TEST_SVG.into());
        let art = store.add_at(art, 1_000, today).unwrap();
        assert_eq!(art.svg.as_deref(), Some(TEST_SVG));
        assert_eq!(art.state, "art");

        let mut timer = input(Kind::Timer, None);
        timer.appearance = Appearance::CustomSvg;
        timer.svg = Some(TEST_SVG.into());
        timer.duration_seconds = Some(600);
        let timer = store.add_at(timer, 1_000, today).unwrap();
        let changed = store
            .update_at(
                timer.id,
                StickerUpdate {
                    appearance: Some(Appearance::Ghost),
                    x: None,
                    y: None,
                    angle: None,
                    title: None,
                    text: None,
                    target: None,
                    done: None,
                    action: None,
                    duration_seconds: None,
                    svg: None,
                },
                1_001,
                today,
            )
            .unwrap();
        assert!(changed.svg.is_none());
        let reloaded = Store::load(&dir).unwrap();
        assert_eq!(
            reloaded.snapshot_at(1_001, today)[0].svg.as_deref(),
            Some(TEST_SVG)
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn notification_ttl_is_returned_and_expired_notifications_are_removed() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        let mut notification = input(Kind::Notification, Some("A temporary message"));
        notification.title = Some("Hello".into());
        notification.expires_in_seconds = Some(30);
        let view = store.add_at(notification, 1_000, today).unwrap();
        assert_eq!(view.expires_at, Some(1_030));
        assert_eq!(view.state, "notification");
        assert_eq!(store.snapshot_at(1_029, today).len(), 1);
        assert_eq!(
            Store::load(&dir).unwrap().snapshot_at(1_029, today)[0].expires_at,
            Some(1_030)
        );
        assert!(store.snapshot_at(1_030, today).is_empty());
        assert!(
            Store::load(&dir)
                .unwrap()
                .snapshot_at(1_030, today)
                .is_empty()
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn clock_sticker_has_no_fixed_content_fields() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let view = store
            .add_at(input(Kind::Clock, None), 1_000, today())
            .unwrap();
        assert_eq!(view.kind, Kind::Clock);
        assert_eq!(view.state, "clock");
        assert!(view.title.is_none());
        assert!(view.text.is_none());
        assert!(view.expires_at.is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn timer_anchor_survives_restart_and_finished_start_restarts() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let mut timer = input(Kind::Timer, None);
        timer.duration_seconds = Some(10);
        let timer_id = store.add_at(timer, 1_000, today()).unwrap().id;
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
    fn creates_require_agent_supplied_coordinates_and_timer_duration() {
        let dir = temp_dir();
        let store = Store::load(&dir).unwrap();
        let today = today();
        let mut missing_coordinates = input(Kind::Todo, Some("Task"));
        missing_coordinates.x = None;
        assert!(store.add_at(missing_coordinates, 1_000, today).is_err());

        let missing_duration = input(Kind::Timer, None);
        assert!(store.add_at(missing_duration, 1_000, today).is_err());
        let mut timer_input = input(Kind::Timer, None);
        timer_input.duration_seconds = Some(600);
        let timer = store.add_at(timer_input, 1_000, today).unwrap();
        assert_eq!(timer.duration_seconds, Some(600));
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
            br#"{"version":3,"next_id":2,"stickers":[{"kind":"timer","id":1,"x":20,"y":16,"angle":0,"appearance":"jack_o_lantern","duration_seconds":1500,"remaining_at_anchor_seconds":1500,"running_since":null,"has_started":false,"created_at":1,"updated_at":1,"text":"legacy-shape"}]}"#,
        )
        .unwrap();
        assert!(Store::load(&dir).is_err());

        fs::write(
            &path,
            br#"{"version":3,"next_id":2,"stickers":[{"kind":"memo","id":1,"appearance":"ghost","x":20,"y":16,"angle":0,"title":"Existing","text":"Kept","created_at":1,"updated_at":1}]}"#,
        )
        .unwrap();
        let existing = Store::load(&dir).unwrap().snapshot_at(2, today());
        assert_eq!(existing.len(), 1);
        assert_eq!(existing[0].title.as_deref(), Some("Existing"));
        assert!(existing[0].svg.is_none());
        let _ = fs::remove_dir_all(dir);
    }
}
