# Contributing to BRONZECAST

BRONZECAST is a Rust renderer and local HLS server with a small web video player.
The browser and Cast display consume the same encoded video; they do not render
their own characters.

## Local setup

Install a current stable Rust toolchain and FFmpeg with H.264 encoding support.
The current renderer requires a graphical desktop/OpenGL context (it opens a
native preview window). Blender is only needed when converting new DAE assets.

```sh
git clone https://github.com/zapplebee/bronzecast.git
cd bronzecast
cargo run --release --locked
```

Open `http://localhost:8080`. Use the printed LAN address for other devices/Cast.
Run from the repository root so relative asset paths resolve.

## Layout

| Path | Purpose |
|---|---|
| `src/main.rs` | Render loop, shared animation, post-processing, FFmpeg/HLS, HTTP startup |
| `src/characters.rs` | Manifest validation, generic OBJ/material loader, character selection API |
| `src/notifications.rs` | Notification API and dashboard drawn into the video |
| `src/*.vert`, `src/*.frag` | Lighting and normal-pass shaders |
| `characters/` | Runtime assets, manifest, and provenance notes |
| `web/` | Video player, character selector, and Cast sender |
| `scripts/` | Asset conversion and encoded-frame inspection |
| `docs/` | Character-addition recipe and validation notes |
| `scratch/` | Entirely ignored local experiments, source downloads, and captures |

## Before submitting changes

```sh
cargo fmt --check
cargo test --locked
cargo build --release --locked
node --check web/player.js
```

For rendering changes, also run the app and inspect an actual encoded light sweep:

```sh
python3 scripts/inspect-stream.py
```

CI checks compilation, asset integrity, API selection logic, formatting, and script
syntax. It does not visually validate GPU rendering or exercise a physical Cast
device. State what you checked when proposing a rendering change.

## Adding a character

Follow [docs/ADDING_CHARACTERS.md](docs/ADDING_CHARACTERS.md). Prefer assets plus a
manifest entry over character-specific Rust or shader code. Keep all runtime asset
paths inside their character folder. Record attribution in
[characters/SOURCES.md](characters/SOURCES.md).

## Working with a running display

Model switches use `POST /api/character` without restarting the server. Before a
necessary restart, preserve desired notifications from `GET /api/notifications`,
then repost their title/message contents afterward. Selection and notifications
are currently in memory. Avoid committing captures, personal notification data,
credentials, or local session exports; `scratch/` is fully ignored.
