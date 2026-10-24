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
  and in-memory; the open item and stickers (including the tomato timer) are
  persistent state under `BRONZECAST_STATE_DIR` (default `state/`) and must
  survive renderer restarts. Stickers may be placed anywhere; the timer defaults
  to the top-left of the character frame, so keep the character header beside it.
- Preserve user-supplied source assets in `scratch/` and the original asset folders.
- Light falloff uses horizontal-only coverage dithering, with different treatment
  for floor/model. Avoid fake dead pixels, vertical grids, or row displacement.
- Restart loses in-memory notifications/selection. Preserve wanted message contents
  before a restart and restore them; switches should not require restarting.
