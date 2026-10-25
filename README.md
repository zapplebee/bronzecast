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

## Notifications and clock stickers

```sh
curl -X POST http://localhost:8080/api/stickers \
  -H 'Content-Type: application/json' \
  -d '{"kind":"notification","appearance":"wrapped_gift","x":650,"y":50,"title":"Build complete","text":"All checks passed. Ready to deploy.","expires_in_seconds":3600}'

curl -X POST http://localhost:8080/api/stickers \
  -H 'Content-Type: application/json' \
  -d '{"kind":"clock","appearance":"sun","x":100,"y":30}'

curl -X POST http://localhost:8080/api/stickers \
  -H 'Content-Type: application/json' \
  -d '{"kind":"art","appearance":"custom_svg","x":40,"y":40,"svg":"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 260 140\"><path d=\"M130 12 245 128H15z\" fill=\"#e55\"/></svg>"}'
```

All notifications and clocks are ordinary agent-created stickers. Every create
requires `kind`, `appearance`, and agent-supplied `x`/`y`;
there is no seeded clock or notification. Notification title defaults to
`Notification`, text is required, and `expires_in_seconds` is required (1–604800).
The API returns an `expires_at` timestamp; no expiry countdown is drawn. Expired
notification stickers are removed automatically and survive renderer restarts
only until their expiry. Clock stickers display the current host-local time/date.
Both are rendered into the encoded video and appear in `GET /api/stickers`.

The public API is proxied to the renderer on port 8081. When the renderer is
offline, character/sticker APIs return 503 while the test-card video keeps
playing. `GET /api/stream` remains available and reports live/fallback input status.

## Agent-controlled stickers

Stickers can be placed anywhere on the 960×540 video board. The seven kinds are
`timer`, `countdown`, `todo`, `memo`, `notification`, `clock`, and `art`. They are
stored in `state/stickers.json` and survive renderer restarts. A new or empty store
starts with **zero stickers**; there is no fixed memo, clock, or notification panel
and no separate timer or notification API.

Every create selects an `appearance`, supplies `x`/`y`, and may supply `angle`
(clockwise degrees around the sticker center; default `0`). Preset appearances are
`jack_o_lantern`, `ghost`, `black_cat`, `bat`, `candy_pail`, `turkey`, `cornucopia`,
`maple_leaf`, `pumpkin_pie`, `christmas_tree`, `wrapped_gift`, `snowman`, `reindeer`,
`gingerbread_person`, `candy_cane`, `fireworks`, `lunar_new_year_lantern`, `diya`,
`heart`, `easter_bunny`, `decorated_egg`, `tulip`, `sun`, and `ice_cream_cone`.
The six text-bearing kinds work with all 24 presets or `custom_svg`; `art` requires
`custom_svg` and has no text overlay. For text-bearing kinds, keep the center clear
for BRONZECAST's behavior text/controls. There is no separate countdown banner.
Rotated visual bounds clamp onto the board. No collision avoidance is performed.

```sh
curl http://localhost:8080/api/stickers

curl -X POST http://localhost:8080/api/stickers \
  -H 'Content-Type: application/json' \
  -d '{"kind":"countdown","appearance":"jack_o_lantern","x":700,"y":330,"angle":-8,"text":"til Halloween","target":"2026-10-31"}'

curl -X PATCH http://localhost:8080/api/stickers/2 \
  -H 'Content-Type: application/json' \
  -d '{"x":500,"y":400}'

curl -X DELETE http://localhost:8080/api/stickers/2
```

`POST /api/stickers` requires `kind`, `appearance`, `x`, and `y`; kinds are
`countdown`, `timer`, `todo`, `memo`, `notification`, `clock`, and `art`. The 24
preset appearances work with all six text-bearing kinds; `custom_svg` works with
those behaviors and is required for `art`.
Countdowns require `text` and a `YYYY-MM-DD` `target`; timers require
`duration_seconds` (1–3600); todos require `text` and accept `done`; memos require
at least one of `title` (up to 48 characters) or `text` (up to 160); notifications
require `text` and `expires_in_seconds` (1–604800). Clocks display host-local time
and date. `art` stickers require `appearance: "custom_svg"` and inline `svg` with
vector shapes only (max 32 KiB); they have no text overlay. For other kinds, custom
SVG is the artwork and BRONZECAST renders the kind's normal content over it. SVG
text, embedded raster images, and external assets are not supported. `PATCH`
accepts common `appearance`, `x`, `y`, and `angle` plus kind-specific fields.
Timer actions (`start`, `pause`, `reset`, or `set`) and `duration_seconds` are sent through
`PATCH /api/stickers/{id}`; `set` requires a duration. Multiple timers are
supported and identified by sticker ID.

`GET /api/snapshot` returns the latest complete renderer frame as `image/png`.
MCP agents can call `bronzecast_get_snapshot` to receive the same board as base64
MCP image content, inspect it, then adjust sticker coordinates or angle.

## Mog and sticker scene

The Mog/character scene and moving spotlight fill the complete 960×540 frame.
There is no fixed clock/date or notification panel. Agents can add clock and
notification stickers, place them anywhere on the canvas, and inspect the
rendered result with `GET /api/snapshot`.

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
Switches preserve stickers and use the same continuous stream, so no server
restart or new Cast session is required. Allow the usual HLS video delay.

**To add another model, follow [docs/ADDING_CHARACTERS.md](docs/ADDING_CHARACTERS.md).**
The supported path is one asset folder plus one manifest entry—no renderer code
or hardcoded UI changes. The registry controls orientation, dimensions, display
name, exclusions, and whether to bounce. Assets are preloaded at startup on the
rendering thread; adding/editing entries requires a restart. All source assets
are preserved locally in ignored source folders; only the runtime asset sets
under `characters/` are included in Git.

Characters render in the full-window scene; agent-created stickers are overlaid.
The loader preserves assembled geometry, normals, UVs, textures, and material
colors, fits the entire model within common bounds, and anchors it to the floor.
The shared animation loops through a 2.8-second bounce with an 8.4-second lateral
drift, gentle banking, and landing squash/stretch. The silhouette, lighting, and
projected shadow share the same transform; movement stays within the full frame.
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
a contact sheet, 2× full-scene enlargements, and 2× floor crops under
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
read status, switch characters, add/update/remove stickers, and inspect the
rendered board with `bronzecast_get_snapshot`;
mutations require the `bronzecast.write` scope. The snapshot is read-only and
requires `bronzecast.read`. Set
`BRONZECAST_MCP_ACCESS_KEYS` on the MCP service and `BRONZECAST_WRITE_KEY` on
playout to protect public writes. See [`docs/MCP.md`](docs/MCP.md).
