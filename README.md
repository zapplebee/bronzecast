# BRONZECAST

BRONZECAST renders selectable characters with their supplied materials, stepped ivory lighting, subtly green contours, and crushed-black shading. The host Rust renderer sends H.264/AAC over UDP to a persistent Docker playout service. The browser and Cast device play that service's continuous HLS video/audio feed—even while the renderer restarts.

**[Add a character](docs/ADDING_CHARACTERS.md)** · **[Contribute](CONTRIBUTING.md)** · **[Asset provenance](characters/SOURCES.md)**

## Run

Install Docker Compose, Rust, and FFmpeg (`ffmpeg` must be on `PATH`), then run from the project directory:

```sh
docker compose up -d --build
cargo run --release --locked
```

Open `http://localhost:8080` or `http://<host-LAN-IP>:8080` from another device.
Docker serves the web app and `/stream/live.m3u8`; the host renderer opens a native
preview and sends media to UDP port 5000. Stop/rebuild/restart the renderer freely:
the shared stream switches to a stylized **1953 RTF test card with silence** until
live input returns. The web player starts muted; use **Unmute** for audio.

Use **Fullscreen**, double-click the video, or press **F** to fill the screen.
Press Escape or use the fullscreen exit button to return; the video keeps its aspect ratio.

See **[docs/PLAYOUT.md](docs/PLAYOUT.md)** for the process boundaries, configuration,
audio input, and FFmpeg UDP sender examples.

The current renderer needs a graphical desktop/OpenGL context. Run from the
repository root. Tested locally on an Apple Silicon Mac with FFmpeg; automated
Linux CI checks builds and asset/API tests, not visual output.

## Cast

Open the web app at its LAN URL in Chrome and use **Cast This Stream**. It sends the playout service's HLS media URL to Google's Default Media Receiver. The Hub must be able to reach the host at the same LAN URL. For HTTPS or network setups where the Hub cannot play local HTTP media, place the server behind a trusted HTTPS reverse proxy.

The `/stream` routes supply cross-origin headers for Google's receiver and disable
caching of live media responses. Encoding runs at 12 fps with two-second segments
to sustain real-time rendering on the host. Allow several seconds for initial buffering.

## Send a notification

```sh
curl -X POST http://localhost:8080/api/notifications \
  -H 'Content-Type: application/json' \
  -d '{"title":"Build complete","message":"All checks passed. Ready to deploy."}'
```

From another device, replace `localhost` with the server's LAN address. The
endpoint returns `201` and the notification's `id`, `title`, `message`, and Unix
`created_at` timestamp. `title` is optional (defaults to `Notification`, maximum
48 characters); `message` is required (1–320 characters, newlines supported).
Invalid text lengths return `400`; malformed JSON/schema errors return `400`/`422`.

`GET /api/notifications` returns the latest 32 notifications, newest first.
History is in memory and resets on restart. The video shows the latest three
cards on the right; long text is wrapped and visually truncated, while the API
retains the full message. The green box above the character shows the newest
title. Notifications are drawn **into the encoded video**, not just the web UI,
so the browser and Hub both receive them, with several seconds of HLS delay.
The current notification cards use fruit styling; the planned return to green
boxes is tracked separately in
[`docs/PLAN_GREEN_NOTIFICATIONS.md`](docs/PLAN_GREEN_NOTIFICATIONS.md).

The public API is proxied to the renderer on port 8081. When the renderer is
offline, character/notification APIs return 503 while the test-card video keeps
playing. `GET /api/stream` remains available and reports live/fallback input status.

## Stickers and tomato timer

Persistent stickers can be placed anywhere on the 960×540 video board. The
renderer supports auto-updating countdowns, tomato timers, todos, and notes.
They are stored as kind-specific records in versioned `state/stickers.json` and
survive renderer restarts; a missing store seeds one 25-minute tomato timer at the
top-left of the character frame. There is no fixed open-item panel or separate
timer API.

`x`/`y` are the unrotated top-left in board pixels. `angle` is optional clockwise
degrees around the sticker center (default `0`). The renderer clamps the rotated
sticker, including its shadow, onto the board. Coordinates are optional on create
and use kind-specific defaults. No collision avoidance is performed.
The canonical data/action contract for API and MCP agents is documented in
[`docs/PLAN_UNIFY_STICKERS.md`](docs/PLAN_UNIFY_STICKERS.md).

```sh
curl http://localhost:8080/api/stickers

curl -X POST http://localhost:8080/api/stickers \
  -H 'Content-Type: application/json' \
  -d '{"kind":"countdown","x":476,"y":380,"angle":-8,"text":"til Halloween","target":"2026-10-31"}'

curl -X PATCH http://localhost:8080/api/stickers/2 \
  -H 'Content-Type: application/json' \
  -d '{"x":500,"y":400}'

curl -X DELETE http://localhost:8080/api/stickers/2
```

`POST /api/stickers` requires `kind` (`countdown`, `timer`, `todo`, or `note`).
Countdowns require `text` and a `YYYY-MM-DD` `target`; timers accept optional
`duration_seconds` (1–3600); todos require `text` and accept `done`; notes require
at least one of `title` (up to 48 characters) or `text` (up to 160). `PATCH`
accepts common `x`, `y`, and `angle` plus kind-specific fields. Timer actions
(`start`, `pause`, `reset`, or `set`) and `duration_seconds` are sent through
`PATCH /api/stickers/{id}`; `set` requires a duration. Multiple timers are
supported and identified by sticker ID.

`GET /api/snapshot` returns the latest complete renderer frame as `image/png`.
MCP agents can call `bronzecast_get_snapshot` to receive the same board as base64
MCP image content, inspect it, then adjust sticker coordinates or angle.

## Home dashboard

The right-hand panel includes a large 12-hour clock with seconds, weekday/date,
host-local UTC offset, time-of-day greeting, and day-progress bar. Compact tiles
show renderer status, renderer uptime, and the number of retained notifications.
Cards below show relative message ages. The clock uses the host computer's local
time zone (including DST) and is encoded into the video, so its displayed time
trails real time by the HLS playback delay. No external home-service accounts are
needed for these readouts.

## Characters and live switching

**Mog, Chocobo, and Monty Mole** are registered in `characters/manifest.json`.
The web player's character dropdown populates from that registry. You can also
switch directly through the API:

```sh
curl http://localhost:8080/api/characters

curl -X POST http://localhost:8080/api/character \
  -H 'Content-Type: application/json' \
  -d '{"id":"chocobo"}'
```

Use `mog`, `chocobo`, or `monty-mole`. POST returns `202`; check
`GET /api/character` for `active_id` and `requested_id`. Unknown IDs return 404.
Switches preserve notifications and use the same continuous stream, so no server
restart or new Cast session is required. Allow the usual HLS video delay.

**To add another model, follow [docs/ADDING_CHARACTERS.md](docs/ADDING_CHARACTERS.md).**
The supported path is one asset folder plus one manifest entry—no renderer code
or hardcoded UI changes. The registry controls orientation, dimensions, display
name, exclusions, and whether to bounce. Assets are preloaded at startup on the
rendering thread; adding/editing entries requires a restart. All source assets
are preserved locally in ignored source folders; only the runtime asset sets
under `characters/` are included in Git.

Characters occupy the left viewport; the right side is reserved for the dashboard.
The loader preserves assembled geometry, normals, UVs, textures, and material
colors, fits the entire model within common bounds, and anchors it to the floor.
The shared animation loops through a 2.8-second bounce with an 8.4-second lateral drift, gentle
banking, and landing squash/stretch. The silhouette, lighting, and projected
shadow share the same transform; movement stays inside the left panel.
The camera stays fixed. A distant, low-angle light in front of the character sweeps
back and forth on a 32-second cycle, easing at each end like a pendulum.

## Rendering

`src/lighting.vert` and `src/lighting.frag` shade the assembled geometry using
the OBJ's supplied normals (rotated with the model, with a computed
fallback) and a distant swinging point light aimed through the torso. The beam
is evaluated in 3D against its aim plane, so head, torso, and feet enter the light
at different positions and the ground footprint falls behind the character. A bounded gray ground
receives a planar projection of the character's shadow (not full self-shadow mapping).
The spotlight's outer falloff uses stable horizontal-strip coverage, with
thresholds based solely on row (no repeating column pattern): surviving pixels retain their
lit color instead of blending toward a darker floor color.
The character uses a finer, model-height-based pattern and a tighter beam transition;
that coverage moves and tilts with the character rather than matching the
floor's screen-space strips. This is an art-directed surface distinction, not
a simulation of distance-dependent dithering.
An additional normal/silhouette pass (`src/normals.vert`, `src/normals.frag`) keeps
faint olive contours visible when the light moves away. The post-process in
`src/main.rs` preserves the shaded surfaces and adds geometry-derived contours.
Soft, evenly spaced four-pixel scanlines complement the
materials, quantized lighting, and shadows. The scanline modulation is applied
after contours so surface features cannot overwrite it. Random dead pixels,
missing dashes, horizontal row jitter, and RGB line offsets are omitted.

## Visual iteration

Run `python3 scripts/inspect-stream.py` while the server is running. It records
32 seconds of the actual HLS video, then creates eight full-resolution PNG frames,
a contact sheet, 2× character crops, and 3× floor crops under
`scratch/visual-review/<timestamp>/`. Enlargements use nearest-neighbor sampling
to expose pixel patterns rather than blur them away. Use the contact sheet to
check the whole light sweep and the original-size/cropped frames to check artifacts.
Captured frames include compression; they are not pristine GPU screenshots.
The fixed camera uses a slightly elevated three-quarter view. All effects are
encoded into the shared video feed.

## Control from MCP

`mcp/` serves the dashboard to MCP clients at `POST /mcp` with an OAuth
authorization server in front of it. Clients register through DCR or a Client ID
Metadata Document, then approve access on a page that takes the static key. Tools
read status, switch characters, add or clear notifications, list/add/update/
remove stickers, and inspect the rendered board with `bronzecast_get_snapshot`;
mutations require the `bronzecast.write` scope. The snapshot is read-only and
requires `bronzecast.read`. Set
`BRONZECAST_MCP_ACCESS_KEYS` on the MCP service and `BRONZECAST_WRITE_KEY` on
playout to protect public writes. See [`docs/MCP.md`](docs/MCP.md).
