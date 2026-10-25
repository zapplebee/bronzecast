# BRONZECAST MCP

The `mcp` service exposes the dashboard to MCP clients (Claude, ChatGPT
connectors, custom agents) over Streamable HTTP. It is also the OAuth
authorization server for that endpoint, so clients can register themselves
through DCR or CIMD without any manual client provisioning.

The service never renders. It proxies reads and writes to the generator's
control API and reads the playout encoder health, exactly like the web page.

The tool list below uses generic sticker operations and includes a rendered-board
snapshot. Notifications and clocks are agent-created sticker behaviors with the
same appearance choices, placement, and CRUD operations. Current sticker kinds,
appearances, and rendering rules are summarized in the project README.

```
MCP client ──HTTPS──> Traefik ──> mcp:8090 ──HTTP──> generator:8081
                          │                    └──> playout:8080/health
                          └── OAuth (same origin as /mcp)
```

## Endpoints

| Path | Purpose |
| --- | --- |
| `POST /mcp` | Streamable HTTP MCP endpoint (initialize, tools, resources). |
| `GET /.well-known/oauth-authorization-server` | RFC 8414 authorization-server metadata. |
| `GET /.well-known/oauth-protected-resource` | RFC 9728 resource metadata (also at `/mcp`). |
| `POST /register` | Dynamic Client Registration (RFC 7591). |
| `GET/POST /authorize` | Static-key approval page. |
| `POST /token` | Code and refresh-token exchange. |
| `POST /revoke` | Token revocation (RFC 7009). |
| `GET /health` | Container health and configuration summary. |

Deployed host: `https://home-mcp.dev.prettybird.zapplebee.online`.

## OAuth

MCP clients are treated as **public clients**: `token_endpoint_auth_method`
must be `none`, and PKCE with `S256` is mandatory. There are no client secrets.

- **DCR** — `POST /register` accepts standard client metadata and returns a
  generated `client_id`. `redirect_uris` must be HTTPS, or loopback HTTP
  (`127.0.0.1`, `::1`, `localhost`) for native clients. Registration is capped
  (1000 clients) and rate-limited.
- **CIMD** — a `client_id` that is an HTTPS URL is treated as a Client ID
  Metadata Document. On `/authorize` the service fetches it (no redirects,
  5 s timeout, 64 KiB cap, JSON content type) and requires the document's
  `client_id` to equal the URL exactly. Hostnames must resolve only to public
  addresses, which blocks SSRF into the LAN or loopback. The metadata advertises
  `client_id_metadata_document_supported: true`, so capable clients use CIMD and
  older ones fall back to DCR.
- **Static key** — the authorization page is a single password field. The value
  is compared against `BRONZECAST_MCP_ACCESS_KEYS` (comma-separated to allow
  rotation) with a constant-time compare. Five failures per IP per five minutes
  produce a 429. The page is CSRF-protected with a double-submit cookie and never
  exposes the key to the client.
- **Tokens** — opaque random values; only SHA-256 hashes are stored. Access
  tokens live one hour, refresh tokens 30 days with rotation, and reuse of a
  rotated token revokes the whole token family. Tokens are bound to the MCP
  resource URL (RFC 8707), so a token issued for another audience is rejected.
- **Scopes** — `bronzecast.read`, `bronzecast.write`, `offline_access`.
  Read tools need `bronzecast.read`; mutating tools need `bronzecast.write`.
  Missing scope returns `403` with `insufficient_scope` and the required scope in
  `WWW-Authenticate`.
- **Storage** — SQLite (`MCP_DB`, default `/data/mcp.sqlite3`) on the `mcp-data`
  volume. Clients, codes, and tokens survive restarts; browser sessions do not.

## Tools

| Tool | Scope | Effect |
| --- | --- | --- |
| `bronzecast_get_status` | read | Character, stickers, and stream health in one call. |
| `bronzecast_get_snapshot` | read | Latest complete 960×540 renderer frame as MCP `image/png` content. |
| `bronzecast_list_characters` | read | Available characters plus active/requested ids. |
| `bronzecast_switch_character` | write | `{id}`; applied on the next rendered frame. |
| `bronzecast_list_stickers` | read | Every sticker and its computed state, including notification `expires_at`. |
| `bronzecast_add_sticker` | write | `{kind, appearance, x, y, angle?, title?, text?, target?, duration_seconds?, done?, expires_in_seconds?, svg?}`. Timers require duration; notifications require TTL; custom SVG requires `svg`. |
| `bronzecast_update_sticker` | write | `{id, appearance?, x?, y?, angle?, title?, text?, target?, done?, action?, duration_seconds?, svg?}`. |
| `bronzecast_remove_sticker` | write | `{id}`; deletes the sticker. |
| `bronzecast_stream_status` | read | Playout source, frame age, encoder state. |

Stickers are movable and rotatable artwork rendered into the full-window video
scene. They live in versioned `stickers.json` under `BRONZECAST_STATE_DIR` and
survive renderer restarts. A new store starts empty; the renderer never seeds any
sticker. `kind` is the behavior (`timer`, `countdown`, `todo`, `memo`,
`notification`, `clock`, or `art`) and `appearance` selects one of 24 presets or
`custom_svg`. Presets and custom SVG work with all six text-bearing kinds; `art`
requires `custom_svg` and has no behavior overlay.
The SVG is inline, capped at 32 KiB, and vector-only; text and embedded images are
not supported. Custom SVG artwork for text-bearing kinds is displayed beneath the
normal behavior content. Each create requires agent-supplied `x` and `y`, the unrotated
top-left in 960×540 board coordinates. `angle` is optional clockwise degrees
around the sticker center. The renderer clamps rotated visual bounds to the
canvas; there are no position or appearance defaults. Every preset has a clear
center for its behavior's text/controls, with no separate countdown banner. The
behavior overlay is rendered directly on any selected appearance.

Countdowns require a `YYYY-MM-DD` target and update automatically; todos have a
completion flag; memos have a title and/or body with no completion state. Timers
require an agent-supplied duration from 1–3600 seconds, are identified by sticker
ID, and accept `action` (`start`, `pause`, `reset`, or `set`) through
`bronzecast_update_sticker`. Notifications require `expires_in_seconds` (1–604800)
and return computed absolute `expires_at`; expiry is not rendered, and expired
records are removed. Clocks show host-local time and date and have no content fields
or expiry. The renderer does not add either kind automatically. No open-item/timer
aliases or runtime state migrations are present.

`bronzecast_get_snapshot` returns the latest complete 960×540 renderer frame as
a standard MCP image content item containing base64 PNG data. It includes the
full-window character/spotlight scene and stickers—not the playout fallback card.
Agents can pair it with `bronzecast_list_stickers`, adjust `x`, `y`, or `angle`,
and inspect a fresh snapshot. Before the renderer publishes its first frame, the
tool reports that the snapshot is unavailable.

Resource: `bronzecast://dashboard/state` returns the same snapshot as
`bronzecast_get_status`.

## Environment

| Variable | Default | Meaning |
| --- | --- | --- |
| `PUBLIC_BASE_URL` | `http://localhost:8090` | Public origin; also the token audience base. |
| `BRONZECAST_MCP_ACCESS_KEYS` | unset | Comma-separated static keys accepted on the login page. Required in production. |
| `GENERATOR_URL` | `http://127.0.0.1:8081` | Renderer control API. |
| `PLAYOUT_URL` | `http://127.0.0.1:8080` | Playout health endpoint. |
| `MCP_DB` | `/data/mcp.sqlite3` | SQLite path. |
| `MCP_ACCESS_TTL` | `3600` | Access-token lifetime in seconds. |
| `MCP_REFRESH_TTL` | `2592000` | Refresh-token lifetime in seconds. |
| `MCP_CODE_TTL` | `60` | Authorization-code lifetime in seconds. |
| `PORT` | `8090` | Listen port. |

The playout side has a matching `BRONZECAST_WRITE_KEY`. When set, writes
(`POST`, `PUT`, `PATCH`, `DELETE`) through the public playout require
`Authorization: Bearer <key>` or `X-Bronzecast-Key: <key>`; `GET`s stay public
for the player. The web UI prompts
for the key on the first rejected character switch and keeps it in
`sessionStorage`.

## Local development

```sh
uv run --with starlette==0.41.3 --with uvicorn==0.32.1 --with httpx==0.27.2 \
  env PUBLIC_BASE_URL=http://localhost:8090 \
      BRONZECAST_MCP_ACCESS_KEYS=dev-key \
      GENERATOR_URL=http://127.0.0.1:8081 \
      PLAYOUT_URL=http://127.0.0.1:8080 \
      MCP_DB=/tmp/bronzecast-mcp.sqlite3 \
  python -m mcp.server
```

Or `docker compose up -d --build`, which publishes `:8090` and joins the playout
network.

## Deployment

`.env` beside `compose.traefik.yaml` on the server:

```sh
BRONZECAST_MCP_ACCESS_KEYS=replace-with-a-long-random-key
BRONZECAST_WRITE_KEY=replace-with-a-long-random-key
GENERATOR_URL=http://192.168.1.223:8081
```

```sh
docker compose -f compose.traefik.yaml up -d --build
```

Traefik needs the `nyx_svc_dev` network and the `le` certificate resolver, as
with the playout host. The MCP host gets its own certificate because OAuth
well-known endpoints live at the origin root.

## Verifying

```sh
BASE=https://home-mcp.dev.prettybird.zapplebee.online
curl -fsS "$BASE/.well-known/oauth-authorization-server" | jq .
curl -fsS "$BASE/.well-known/oauth-protected-resource" | jq .
curl -fsS "$BASE/health" | jq .
```

Then add `$BASE/mcp` as a remote MCP server in a client, approve with the static
key, and call `bronzecast_get_status`. For a scripted check, register a client,
open the returned `/authorize` URL in a browser, and exchange the code with PKCE;
the test suite covers that flow end to end.

## Tests

```sh
uv run --with starlette==0.41.3 --with uvicorn==0.32.1 --with httpx==0.27.2 \
  python -m unittest mcp.test_server
```

Covered: DCR validation, full DCR authorization + tool calls, CIMD fetch and
identity checks, private-address rejection, PKCE failure, code single use,
refresh rotation and reuse revocation, scope enforcement, token revocation and
audience binding, and tool-error mapping.

## Security notes

- The static key is the only human credential. Rotate it by listing old and new
  keys in `BRONZECAST_MCP_ACCESS_KEYS` and restarting the service; remove the old
  one after clients re-authorize.
- Tokens are audience-bound and short-lived; the SQLite volume contains hashes
  only. Deleting the volume invalidates every client and token.
- CIMD fetching refuses redirects, non-JSON, oversized documents, and hosts that
  resolve to non-public addresses. It resolves the host immediately before the
  request, which narrows but does not eliminate DNS-rebinding risk.
- The MCP endpoint does not send CORS headers, so browsers cannot call it
  directly with a stolen token.
- Reads of the public dashboard remain unauthenticated by design; only mutations
  are gated.
