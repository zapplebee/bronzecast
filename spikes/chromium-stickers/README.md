# Chromium sticker pipeline spike

Local-only prototype for the proposed TSX sticker pipeline. Bun owns a small REST
API and saved TSX/state. A persistent Chromium page renders the existing Mog OBJ
with Three.js. The camera, motion, spotlight coverage, projected shadow, normal
contours, and scanlines follow the current Rust renderer; React components render
above it. Playwright captures the 960×540 page at 12 FPS; FFmpeg encodes it as
H.264/AAC MPEG-TS with silent audio. By default it sends to UDP port 5001 for VLC;
`UDP_HOST` and `UDP_PORT` can direct it to a remote BRONZECAST playout input.

## Run

Requires Bun, FFmpeg, Google Chrome, and VLC on macOS.

```sh
cd spikes/chromium-stickers
bun install
bun run start
```

Open `udp://@127.0.0.1:5001` in VLC, or run:

```sh
open -a VLC 'udp://@127.0.0.1:5001'
```

The API is at `http://127.0.0.1:8091`.

To feed the Nyx playout instead, set the UDP destination when starting the spike:

```sh
UDP_HOST=192.168.1.86 UDP_PORT=5000 bun run start
```

Then open `https://mail.dev.prettybird.zapplebee.online/stream/live.m3u8` in VLC
to watch the playout's HLS output.

## Run the pipeline on Nyx

From the repository root on Nyx, start the existing playout/MCP stack together
with the Chromium renderer:

```sh
docker compose -f compose.traefik.yaml -f spikes/chromium-stickers/compose.nyx.yaml up -d --build
```

The renderer joins `nyx_svc_dev`, sends H.264/AAC to the playout container's UDP
input, and persists sticker state in a Docker volume. The existing OAuth-protected
MCP endpoint gains `bronzecast_tsx_*` tools; state writes remain behind
`bronzecast.write`.

Each component can declare the state fields agents may edit with a named
`editableProps` export. The app registers that schema and passes the current
backend-owned snapshot to the component as `state`:

```tsx
export const editableProps = {
  complete: { type: "boolean", default: false },
} as const;

export default function Todo({ state }: { state: { complete: boolean } }) {
  return <div>{state.complete ? "Done" : "Open"}</div>;
}
```

For time-driven visuals, components can import `useAnimationFrame` from
`@bronzecast/sticker-runtime`. It reads the page's shared animation clock, so a
timer can derive its display from backend-owned values such as `cycleStartedAt`
without the agent writing state every frame.

Create a sticker by sending its TSX source and placement, without embedding its
editable state in the component call:

```sh
bun -e 'const tsx = await Bun.file("examples/bathroom-todo.tsx").text(); const r = await fetch("http://127.0.0.1:8091/api/stickers", {method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({tsx,x:30,y:30})}); console.log(await r.text())'
```

Once Chromium loads the module it registers `editableProps`. Agents can inspect the
schema with `GET /api/stickers/:id/schema`, then write the complete shared state.

The API supports `GET /api/stickers`, `PATCH /api/stickers/:id` for source or
placement, `DELETE /api/stickers/:id`, `GET /api/stickers/:id/state`, and
`PUT /api/stickers/:id/state`. State `PUT` replaces the complete object and is
validated against the component's `editableProps` schema. For example, an agent
marks a todo done without changing its TSX:

```sh
curl -X PUT http://127.0.0.1:8091/api/stickers/5/state \
  -H 'Content-Type: application/json' \
  -d '{"complete":true}'
```

Each renderer polls the same backend snapshots, so a state write is shared by the
capture page and other clients. State and schema persist in the spike's ignored
`state/` directory. Changing TSX loads the new component; updating state does not
rewrite or remount it.

This spike accepts only self-contained TSX components with optional React imports.
It binds to loopback and is not intended as a public endpoint or a secure sandbox.
