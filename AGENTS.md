# Working on BRONZECAST

## Adding or switching characters

Read `docs/ADDING_CHARACTERS.md` first. The supported path is **asset folder +
manifest entry**. `characters/manifest.json` registers Mog, Chocobo, and Monty Mole
as tested examples. Do not hardcode model IDs, labels, per-model loaders, camera
changes, or shader branches into Rust to add a standard character.

Run `cargo test --locked` and `cargo build --release --locked`. Verify a switch with
`POST /api/character` and inspect actual encoded frames using
`python3 scripts/inspect-stream.py`. A successful build is not visual validation.

## Architecture to preserve

- Rust renders one live video and sends H.264/AAC MPEG-TS over UDP :5000.
  Docker playout owns the continuous output encoder and web/HLS :8080, switching
  to a stylized RTF card + silence when input disappears. Renderer control is :8081.
  Do not couple playout lifetime to generator restarts or add a browser/WebGL renderer.
- The `mcp` service is a public Streamable HTTP MCP endpoint with its own OAuth
  server (DCR + CIMD, PKCE, static-key login). It only proxies the control API and
  playout health; it must never render or own the stream. Keep its mutations behind
  `bronzecast.write` and the playout's `BRONZECAST_WRITE_KEY` gate. See `docs/MCP.md`.
- HTTP handlers only request an active ID. GPU work belongs to the rendering
  thread. Models share lighting, animation, shadows, and the notification layout.
- The character stays left; clock/dashboard/notifications stay right. Notifications
  are baked into the video, not a browser-only overlay. Notifications are ephemeral
  and in-memory; all durable board content is one sticker domain under
  `BRONZECAST_STATE_DIR` (default `state/stickers.json`) and must survive renderer
  restarts. Stickers exist only when an agent creates them; every create supplies
  its own `x`/`y`, and stickers may be rotated with `angle`. The renderer must not
  seed or position a default sticker.
- Preserve user-supplied source assets in `scratch/` and the original asset folders.
- Light falloff uses horizontal-only coverage dithering, with different treatment
  for floor/model. Avoid fake dead pixels, vertical grids, or row displacement.
- Restart loses in-memory notifications/selection. Preserve wanted message contents
  before a restart and restore them; switches should not require restarting.

## Sticker and notification contract

[`docs/PLAN_UNIFY_STICKERS.md`](docs/PLAN_UNIFY_STICKERS.md) records the implemented
sticker contract: one persistent domain (`timer`, `countdown`, `todo`, `memo`),
generic CRUD/API/MCP tools, arbitrary coordinates and rotation, and a read-only
MCP snapshot image. Do not reintroduce the fixed open item, separate timer API,
`reminder` kind, aliases, or runtime migrations.

The upper-right area must remain empty unless an agent creates a memo there—do not
create one automatically. The board starts with zero stickers. Notifications
remain ephemeral and use the green-box rendering recorded in
[`docs/PLAN_GREEN_NOTIFICATIONS.md`](docs/PLAN_GREEN_NOTIFICATIONS.md). Keep that
visual identity separate from fruit stickers; update renderer, tests, README, and
API docs together for future notification changes.
