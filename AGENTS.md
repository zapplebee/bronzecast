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
  thread. Models share lighting, animation, shadows, and the full-window scene.
- Mog/character and its spotlight render across the whole 960×540 frame. Stickers
  persist under `BRONZECAST_STATE_DIR` (default `state/stickers.json`). Fixed
  clock/date UI and the separate notification feed are absent.
- Preserve user-supplied source assets in `scratch/` and the original asset folders.
- Light falloff uses horizontal-only coverage dithering, with different treatment
  for floor/model. Avoid fake dead pixels, vertical grids, or row displacement.
- Restart loses the in-memory character selection. Stickers persist; notification
  stickers remain until expiry. Character switches do not require restarting.

## Stickers

There is one persisted sticker domain (`BRONZECAST_STATE_DIR`, default
`state/stickers.json`) and one generic REST/MCP CRUD surface. Its seven kinds are
`timer`, `countdown`, `todo`, `memo`, `notification`, `clock`, and `art`.

- Six text-bearing kinds can use any of the 24 named preset appearances or
  `custom_svg`. Art requires `custom_svg` and has no text overlay. SVG is inline,
  at most 32 KiB, vector-only, and rendered locally; no external assets or embedded
  images. BRONZECAST renders normal behavior text/controls over custom SVG artwork.
- Every create supplies `kind`, `appearance`, `x`, and `y`; `angle` is optional.
  There are no seeded or default stickers, and the renderer does not choose a
  position. Preset motifs reserve a central 184×76 text-safe area.
- Notifications require agent-supplied `expires_in_seconds`; API views return
  `expires_at`, but the expiry is never drawn. Expired notifications are removed.
  Clocks render host-local time/date. Neither has a separate feed or fixed UI.
- Stickers persist across renderer restarts; notification stickers remain until
  expiry. Keep storage at version 3 and avoid runtime migrations/legacy aliases.

Do not reintroduce the fixed open item, separate timer/notification APIs or MCP
tools, `reminder`/`note` kinds, aliases, or runtime migrations. Keep the board empty
until agents create stickers. Update renderer, tests, `README.md`, `docs/MCP.md`,
`docs/PLAYOUT.md`, and API descriptions together for future sticker changes.
