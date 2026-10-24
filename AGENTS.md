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
- HTTP handlers only request an active ID. GPU work belongs to the rendering
  thread. Models share lighting, animation, shadows, and the notification layout.
- The character stays left; clock/dashboard/notifications stay right. Notifications
  are baked into the video, not a browser-only overlay.
- Preserve user-supplied source assets in `scratch/` and the original asset folders.
- Light falloff uses horizontal-only coverage dithering, with different treatment
  for floor/model. Avoid fake dead pixels, vertical grids, or row displacement.
- Restart loses in-memory notifications/selection. Preserve wanted message contents
  before a restart and restore them; switches should not require restarting.
