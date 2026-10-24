# Plan: One persistent sticker domain

Status: Implemented and verified with an empty store and live encoded frame
Scope: Rust renderer and state, HTTP API, MCP, playout proxy, tests, documentation
Related: [MCP](MCP.md), [Playout](PLAYOUT.md), [README](../README.md),
[AGENTS](../AGENTS.md), [Green notifications](PLAN_GREEN_NOTIFICATIONS.md)

## 1. Intent and problem

The board is a **user-controlled sticker surface**, not a dashboard with fixed
slots. The renderer starts with **zero stickers**. Every sticker, its position,
and its timer settings are created or changed by agents through the sticker API.
The renderer only draws the supplied sticker state and derives each type's live
display values (for example, timer remaining time or countdown days). Notifications
are transient and visually separate from stickers.

| Before | After |
| --- | --- |
| Fixed OPEN ITEM panel in the upper right | Removed; space is empty until an agent creates a memo there |
| Separate open-item store/API/MCP tools | One sticker store/API |
| Separate `/api/timer` and timer MCP tools | Timers are ordinary stickers |
| `reminder` and open item both hold freeform persistent text | One `memo` kind |
| Notifications styled as fruit pills | Notifications return to green boxes; see the separate plan |

The semantic rule is intentionally simple:

- If it can be completed, it is a **todo**.
- If it is information to keep visible, it is a **memo**.
- If it counts down to a calendar date, it is a **countdown**.
- If it counts down from a duration, it is a **timer**.

BRONZECAST currently has two persistent domains that overlap:

- The **open item** is one fixed green card in the upper-right of the dashboard. It
  has its own store (`src/open_item.rs`, `state/open-item.json`), HTTP API
  (`/api/open-item`), three MCP tools, and a fixed renderer position.
- **Stickers** are movable fruit cards with their own store
  (`src/stickers.rs`, `state/stickers.json`), HTTP API (`/api/stickers`,
  `/api/timer`), MCP tools, and a `reminder` kind that is also persistent freeform
  text.

The open item and the sticker `reminder` are both freeform persistent text; the
todo is a task with completion state. The open item and reminder collapse into
`memo`; todo remains a distinct kind. The current split duplicates validation,
persistence, MCP surface, docs, and visual language for persistent content.

The target is one persistent sticker domain with four kinds: **timer**,
**countdown**, **todo**, and **memo**. The open item is removed, along with every
compatibility path.

## 2. Decision

There is one way to place durable information on the rendered board: **stickers**.
Delete the fixed **OPEN ITEM** panel in the upper right and its independent
storage, API, and MCP tools. Delete the separate timer API/tools too. Four sticker
kinds remain:

| Kind | Meaning | Visual treatment | Key behavior |
| --- | --- | --- | --- |
| `timer` | Relative-duration focus timer | Tomato clock | Start, pause, reset, set duration; elapsed time survives restarts |
| `countdown` | Calendar date milestone | Lemon and date banner | Derived days until target date |
| `todo` | A completable task | Green fruit + checkbox | Toggle complete/incomplete |
| `memo` | Information to leave visible | Peach fruit + text | Title and/or body; no completion state |

Notifications remain a separate, ephemeral feed and are never persisted as
stickers.

The board shown in the reference image is the target composition _minus the fixed
upper-right OPEN ITEM card_. A memo is an ordinary agent-created movable sticker,
not a fixed dashboard panel. The space vacated by OPEN ITEM remains open; never
seed or silently recreate a sticker there.

### Goals

- Keep one persistent store and one CRUD API for all persistent board content.
- Define and document the schema for `timer`, `countdown`, `todo`, and `memo`.
- Remove the open item store, routes, MCP tools, renderer panel, and docs.
- Remove the separate timer API/tools; timers are ordinary stickers.
- Keep notifications ephemeral and separate; notifications are not stickers.
- Keep the fruit sticker visual language for stickers.

### Compatibility policy

This project is greenfield. Do not add compatibility layers, aliases, or legacy
readers:

- No `reminder` kind or alias. The kind is `memo`.
- No `/api/timer` endpoint. Use `/api/stickers` and
  `PATCH /api/stickers/{id}` with a timer `action`.
- No `bronzecast_get_timer` or `bronzecast_set_timer`. Use
  `bronzecast_list_stickers` and `bronzecast_update_sticker`.
- No migration from `open-item.json`, `timer.json`, or old sticker records.
- No `legacy_key` field or migration marker.
- No compatibility alias for `/api/open-item`.

## 3. Scope and explicit exclusions

**In scope**

- One versioned `state/stickers.json` store and one REST CRUD resource,
  `/api/stickers`.
- One family of four generic MCP sticker tools.
- Canonical discriminated schema, validation, computed views, agent-specified
  placement, timer
  state transitions, tests, and docs.
- Removal of `src/open_item.rs`, `state/open-item.json`, `/api/open-item`, and all
  open-item MCP methods and rendering.
- Removal of `/api/timer`, timer-specific MCP methods, and the obsolete
  `reminder` kind.

**Out of scope**

- Notification persistence or notification behavior/API changes. Notification
  visual identity and card styling are covered separately in
  [PLAN_GREEN_NOTIFICATIONS.md](PLAN_GREEN_NOTIFICATIONS.md).
- Changes to character, camera, lighting, animation, or video delivery.
- Browser overlays, a database, server-driven collision detection, or sticker
  drag UI changes beyond supporting existing coordinates.
- Automatic conversion of an old open item into a `memo` or `todo`.
- Compatibility aliases, legacy state readers, or migrations; this is a
  greenfield breaking cleanup.

## 4. Canonical data model

Use **tagged, per-kind records**, not one record containing every possible field.
A timer must not serialize `done` or `target`; a todo must not serialize timer
properties. The common envelope is shared across types:

| Common field | Type | Contract |
| --- | --- | --- |
| `id` | positive `u64` | Server-assigned, immutable, unique; never reused |
| `kind` | string enum | Exactly `timer`, `countdown`, `todo`, `memo`; immutable |
| `x`, `y` | finite number | Agent-supplied unrotated top-left in the renderer's **960 × 540 logical canvas**; normalized/clamped before persisting |
| `angle` | finite number | Clockwise degrees around the sticker center; defaults to `0`, normalized before persisting |
| `created_at` | Unix seconds (`u64`) | Server-assigned once; never changes |
| `updated_at` | Unix seconds (`u64`) | Last successful mutation; not updated per rendered frame |

All examples below show **stored records**. Fields such as `state`, `days`, and
`remaining_seconds` are derived in the API/view layer unless explicitly listed as
persisted. Unknown properties are rejected in API writes; internal storage
deserialization must not silently accept invalid per-kind shapes.

### Timer

```json
{
  "id": 1,
  "kind": "timer",
  "x": 20,
  "y": 16,
  "angle": 0,
  "duration_seconds": 1500,
  "remaining_at_anchor_seconds": 1500,
  "running_since": null,
  "has_started": false,
  "created_at": 1792843200,
  "updated_at": 1792843200
}
```

- `duration_seconds`: agent-supplied integer **1–3600**; required when creating a timer.
- `angle`: shared field; see the placement contract in §5.
- `remaining_at_anchor_seconds`: integer **0–duration_seconds**. Persisted amount
  remaining when last started/paused/edited; never continually rewritten by the
  renderer.
- `running_since`: Unix seconds or `null`. When non-null, timer runs; when null,
  it is stopped/paused/ready/finished.
- Effective remaining at current time `now` is
  `max(0, remaining_at_anchor_seconds - max(0, now - running_since))` if running,
  otherwise `remaining_at_anchor_seconds`.
- Read view adds `remaining_seconds` (effective value), `finished`
  (`remaining_seconds == 0`), and `state`: `running`, `paused`, `ready`, or
  `done`. A running timer that has elapsed reads as `done` even before the next
  write.
- Distinguish `ready` from `paused` deterministically: add a stored
  `has_started` boolean, initially `false`, set `true` on `start` and `pause`,
  reset to `false` on `reset`/`set`. The persisted timer schema must include
  `"has_started": false` on a new timer. Derived state precedence is `done` >
  `running` > (`paused` if `has_started`, else `ready`).
- Clock rollback is clamped to zero elapsed. The renderer uses host wall time for
  persistence and may use a monotonic clock for smooth frames within one process;
  views and persistence must agree within one second.
- At expiration, the API must report `done`, `finished: true`, and
  `remaining_seconds: 0` without relying on a background ticker or a disk write.
  Do not send repeated completion notifications from the rendering loop.

**Timer actions** via `PATCH /api/stickers/{id}`:

| Action | Effect | Invalid/edge cases |
| --- | --- | --- |
| `start` | Resume paused/ready at the current effective remaining; a finished timer restarts from full duration. Set `running_since = now`, `has_started = true` | Starting an already running timer is idempotent. Starting a finished timer is the normal "go again" action, not an error. |
| `pause` | Freeze effective remaining, set `running_since = null`, `has_started = true` | Pausing ready/done is a no-op, not a restart. |
| `reset` | Restore full duration, stop timer, clear `has_started` | Allowed from any state. |
| `set` | Require `duration_seconds`; replace duration and remaining with that duration, stop timer, clear `has_started` | Cannot combine with other actions. |

Only timer records accept `action`. An update may contain `x` and/or `y` plus a
timer action, but `duration_seconds` is writable **only with** `action: "set"`.
Reject client writes to anchors, `has_started`, `remaining_seconds`, `state`, or
`finished`. Applying `start`/`pause` uses a single captured `now` under the store
lock, then commits atomically.

### Countdown

```json
{
  "id": 2,
  "kind": "countdown",
  "x": 480,
  "y": 347,
  "angle": 0,
  "text": "til Halloween",
  "target": "2026-10-31",
  "created_at": 1792843200,
  "updated_at": 1792843200
}
```

- `text`: required, trimmed, 1–48 characters; no line breaks or control
  characters.
- `angle`: shared field; see the placement contract in §5.
- `target`: required, real calendar date in `YYYY-MM-DD`; reject impossible dates
  (for example February 30).
- Read view adds signed `days = target_date − board_local_today` and `state`:
  `upcoming` (> 0), `today` (= 0), `past` (< 0).
- **Calendar timezone:** use the configured board/host local timezone consistently
  for renderer and API. Do not use UTC date boundaries for one and local date
  boundaries for the other. Document the chosen timezone in runtime configuration
  and tests.
- Recalculate at render/view time; no midnight persistence rewrite is required.
  The visual can decide whether to show `0 DAYS`, `TODAY`, or an overdue label,
  but it must not misrepresent negative days as time remaining.

### Todo

```json
{
  "id": 3,
  "kind": "todo",
  "x": 697,
  "y": 380,
  "angle": 0,
  "text": "Pack for the weekend",
  "done": false,
  "created_at": 1792843200,
  "updated_at": 1792843200
}
```

- `text`: required, trimmed, 1–160 characters. Permit line breaks in data, with a
  bounded renderer wrap/truncation policy.
- `done`: boolean, default `false`; readable and directly patchable to either
  value. `state` in the read view is `open` or `done`.
- `angle`: shared field; see the placement contract in §5.
- Completing a todo does **not** delete it, remove it from the canvas, or convert
  it to a notification. Reopening sets `done: false`.

### Memo

```json
{
  "id": 4,
  "kind": "memo",
  "x": 697,
  "y": 12,
  "angle": 0,
  "title": "Open item is working",
  "text": "Pinned from OpenCode over MCP",
  "created_at": 1792843200,
  "updated_at": 1792843200
}
```

- `title`: optional, default `""`, trimmed, maximum 48 characters; no line
  breaks/control characters.
- `text`: optional, default `""`, trimmed, maximum 160 characters; line breaks
  allowed but reject other control characters.
- At least one of `title` or `text` must be non-empty after trim, **including after
  a PATCH**.
- `state: "memo"` in read view. Optional display age comes from `created_at`; age
  never changes the stored record. A memo is **not** a task: it has no `done` or
  checkbox.
- A memo is never auto-created. The open-item void stays empty unless an agent
  creates and positions a memo there.
- `angle`: shared field; see the placement contract in §5.

### Storage container

```json
{
  "version": 1,
  "next_id": 5,
  "stickers": [
    {
      "id": 1,
      "kind": "timer",
      "x": 20,
      "y": 16,
      "angle": 0,
      "duration_seconds": 1500,
      "remaining_at_anchor_seconds": 1500,
      "running_since": null,
      "has_started": false,
      "created_at": 1792843200,
      "updated_at": 1792843200
    }
  ]
}
```

- The examples describe agent-created records; their timestamps are illustrative.
- `version` is exactly `1`; reject unsupported versions explicitly rather than
  quietly resetting user state.
- `next_id` must be strictly greater than every stored ID and remain monotonic
  after deletions.
- A missing file loads as an **empty sticker collection** with `next_id: 1`.
  Create no timer, todo, countdown, or memo until an agent requests one. A read
  before the first mutation returns `[]`; persist the empty versioned container
  with the first successful create/update/delete.
- A malformed existing file must produce a visible load error; **do not overwrite
  it with an empty store**.
- Commit by writing to a temporary file in the same directory, flushing/syncing as
  appropriate, and atomically renaming. Mutations are serialized under a shared
  store lock so concurrent MCP/API clients cannot lose updates.
- Do not load `state/open-item.json` or `state/timer.json`; remove them from
  tracked/deployed state during rollout. A `reminder` record is invalid rather
  than auto-converted.

## 5. Validation and coordinate contract

- Reject writes with unknown properties, wrong kind-specific properties,
  nonfinite coordinates, invalid integers, empty required text, control
  characters, or impossible dates with **HTTP 400** and a field-specific
  explanation.
- `kind`, `id`, `created_at`, `updated_at`, and every derived property are
  read-only after create; reject attempted mutation, even if the value would not
  change.
- `POST` requires `kind`, `x`, and `y`; there are no position defaults. The agent
  chooses each sticker's board position. Timer creation also requires an explicit
  `duration_seconds`. `PATCH` changes only supplied writable fields; never erase
  omitted fields.
- `angle` is optional on create/update, finite, in degrees clockwise, and
  normalized to `[-180, 180)`. Missing create angle defaults to `0`; updates
  preserve it unless supplied.
- `x`/`y` identify the agent-chosen unrotated sticker bounding box's top-left. Normalize
  accepted coordinates to keep the **rotated sticker including its shadow** on
  the 960×540 logical canvas. Compute the rotated axis-aligned extents around the
  sticker center, then clamp the center to those extents; validate finiteness
  before normalization.
- Text must be bounded both in the stored model and at draw time; long content
  must wrap and then truncate/ellipsize within the fruit safe area, never spill
  outside the die-cut shape.
- A PATCH that makes no effective change may return the view without changing
  `updated_at`. Timer no-op actions follow this rule.

### Rendered snapshot

- Add read-only `GET /api/snapshot` on the renderer and allow it through playout.
- Return the most recently rendered **960×540 full board frame** as `image/png`,
  including character, dashboard, notifications, and stickers. Cache the same
  composed pixels used for the outgoing video; the HTTP handler must not perform
  GPU work.
- Add read-only MCP tool `bronzecast_get_snapshot`. It fetches that endpoint and
  returns a standard MCP image content item (`type: image`, `mimeType: image/png`,
  `data`: base64 PNG). The tool requires `bronzecast.read`, not write scope.
- This snapshot is the renderer's latest frame, not the playout fallback card;
  return a clear unavailable error before the first frame is cached.
- Agents should use `bronzecast_get_snapshot` with `bronzecast_list_stickers`,
  then adjust `x`, `y`, or `angle` with `bronzecast_update_sticker` and inspect
  another snapshot. Do not add browser or MCP-side image overlays.

## 6. REST and MCP contract

### HTTP

| Method | Endpoint | Success | Contract |
| --- | --- | --- | --- |
| `GET` | `/api/stickers` | 200 | Array of computed sticker views ordered by `id` |
| `POST` | `/api/stickers` | 201 | Create one kind-valid sticker; return computed view |
| `PATCH` | `/api/stickers/{id}` | 200 | Partial update or timer action; return computed view |
| `DELETE` | `/api/stickers/{id}` | 204 | Remove existing sticker |

There is no single-sticker `GET` in this plan. Unknown IDs on PATCH/DELETE
return **404**; invalid fields or actions return **400**. Every mutating route
continues to enforce the existing write-key policy.

Remove `/api/timer` and `/api/open-item` entirely. They should return `404`.

**Examples:**

```http
POST /api/stickers
Content-Type: application/json

{"kind":"memo","x":690,"y":24,"title":"Open item is working","text":"Pinned from OpenCode over MCP"}
```

```http
PATCH /api/stickers/1
Content-Type: application/json

{"action":"start"}
```

```http
PATCH /api/stickers/3
Content-Type: application/json

{"done":true}
```

### MCP

Keep only these sticker tools:

- `bronzecast_list_stickers`
- `bronzecast_add_sticker`
- `bronzecast_update_sticker`
- `bronzecast_remove_sticker`

Also expose read-only `bronzecast_get_snapshot` so agents can inspect the actual
rendered board and iteratively place/rotate stickers. It returns the PNG as MCP
image content, not only a text URL or a base64 string embedded in prose.

The add/update argument schemas must match REST rules, including `title` for
memos, `done` for todos, and `action`/`duration_seconds` for timers. MCP tools
must return actionable validation and not-found errors, not silent success.
`bronzecast_get_status` includes the sticker collection with computed views, plus
character, notifications, and stream health; **remove top-level `open_item` and
`timer`**.

Remove `bronzecast_get_open_item`, `bronzecast_set_open_item`,
`bronzecast_clear_open_item`, `bronzecast_get_timer`, and
`bronzecast_set_timer` completely. Never expose an alias or `reminder` kind.
Update tool descriptions so agents understand when to create a memo versus a
todo, and that timers are stickers identified by ID.

## 7. Canvas and renderer contract

| Kind | Position | Rendered bounding box | Visual |
| --- | --- | --- | --- |
| `timer` | Agent-specified | `164×164` | Existing tomato timer |
| `countdown` | Agent-specified | `200×190` | Existing yellow lemon and cream banner |
| `todo` | Agent-specified | `260×140` | Existing green fruit and checkbox |
| `memo` | Agent-specified | `260×140` | Peach fruit with title/body text |

There are no creation-position defaults or reserved slots. More than one sticker
of any kind is supported; never treat the timer as a singleton. Preserve
agent-supplied coordinates and angle. ID ascending determines draw order, so newer
IDs appear above older stickers where they overlap. No collision avoidance is
added.

- Apply each sticker's rotation to its complete composed art (fruit, outline,
  shadow, text, and marks), around the visual center. Positive degrees rotate
  clockwise. Default angle is zero.
- Use the rotated visual bounds for edge clamping. `x`/`y` remain the top-left of
  the unrotated footprint so a list/update round trip does not make placement
  coordinates ambiguous.

- Remove the fixed upper-right green OPEN ITEM panel, its border, header, content,
  and age marker; do not replace it with an empty placeholder.
- Keep the status metrics, notification section, and notification ordering;
  notification card styling is covered by
  [PLAN_GREEN_NOTIFICATIONS.md](PLAN_GREEN_NOTIFICATIONS.md).
- Render stickers **after** notifications, so sticker art is frontmost as in the
  reference screenshot.
- Keep existing tomato, lemon, and todo styles. Add a memo using the **same
  die-cut white outline, shadow, and pixel-font styling** as other fruit
  stickers. Preserve the playful sprite identity rather than reproducing the
  deleted rectangular dashboard card.
- Memos with both fields: title emphasized toward top, body wrapped below.
  Title-only or body-only: center content. A small muted age indicator may use
  `created_at`, if it fits safely; no age indicator may reduce text legibility.
- Preserve timer `MM:SS` readout based on effective remaining, not stale
  serialized seconds. Countdown visual derives local calendar days every
  frame/view.
- Check overlapping stickers, right/bottom boundaries, notification occlusion, and
  all four kinds on the **encoded output frame**, not just a raw compositing
  preview.
- Do not change notification rendering here; see
  [PLAN_GREEN_NOTIFICATIONS.md](PLAN_GREEN_NOTIFICATIONS.md).

## 8. Implementation sequence

1. **Model and store:** Replace union-shaped sticker data with a tagged kind model;
   add validation, serialization, derived views, `next_id`, atomic writes, and
   lock-based mutation. Load an empty store without seeding a timer. Add persisted
   common `angle`.
2. **Timer semantics:** Centralize effective-remaining computation and implement
   start/pause/reset/set as store mutations using one `now` per operation.
3. **REST:** Implement generic CRUD/action handling and errors. Remove `/api/timer`
   and `/api/open-item` registrations and modules.
4. **Renderer:** Remove `open_item` dependencies and fixed panel drawing. Render
   `memo` with other stickers; rotate each complete sticker as a unit; cache the
   latest composed output pixels for snapshot requests. Keep existing fruit
   visuals. Notification visual identity is handled by the separate plan.
5. **MCP:** Remove open-item/timer methods, update generic sticker tool schemas and
   `bronzecast_get_status`, and add `bronzecast_get_snapshot` with image content.
6. **Playout:** Allow `/api/stickers`, `/api/stickers/{id}`, and `/api/snapshot` with existing
   method/write-key gating; remove obsolete paths from `playout/server.py`
   `API_PATHS`.
7. **Docs:** Update `docs/MCP.md`, `docs/PLAYOUT.md`, `README.md`, and `AGENTS.md`;
   remove references to legacy open-item/reminder/timer surfaces.
8. **Clean deployment:** The old `stickers.json`, `open-item.json`, and `timer.json`
   state was explicitly nuked. Do not migrate or recreate any of its contents.
   Deploy renderer, MCP, and playout together; reconnect MCP clients to refresh
   tools. The new board starts empty.
9. **Verification:** Run tests and capture a live encoded frame showing all four
   kinds; commit only after verifying output.

**Operational note:** This is a deliberate greenfield reset, not a runtime
migration. The old sticker/timer/open-item files were removed after explicit user
approval. Future schema changes still do not add runtime legacy readers.

## 9. Test matrix

**Rust model/store**

- Create, update, list, and remove each kind; serialization round-trip;
  deterministic ID ordering and monotonic `next_id`.
- Verify a missing store produces an empty sticker list and allocates no timer ID
  until an agent creates a sticker; reject creates without x/y or timer duration.
- Reject missing/unknown kinds, `reminder`/`note`, cross-kind fields, unknown
  attributes, invalid/multiline title, empty memo, bad dates, impossible date, malformed
  existing file, and unsupported version.
- Date tests across local midnight, leap day, today, and past targets; timer tests
  across pause/restart/expiration/process restart/clock rollback.
- Start twice (idempotent), pause ready, pause running, set duration, reset
  finished, start finished restarts, reject illegal timer-field writes.
- Entire-sticker clamping at all edges for each footprint; reject NaN/infinite
  coordinates and angle; angle wrapping and rotated bounds.
- Verify stored `updated_at` changes on mutation only, not each render; concurrent
  updates do not overwrite unrelated records.

**HTTP/MCP**

- Generic route behaviors (201/200/204/400/404), read-only fields, error
  messages, and write-key enforcement.
- Snapshot returns the latest board PNG through renderer and playout; MCP returns
  valid base64 `image/png` content under read scope, with unavailable-before-first
  frame handled as an error.
- Removed `/api/open-item` and `/api/timer` are not registered (404 at renderer;
  blocked by playout).
- Only four generic sticker MCP tools appear; status has no top-level `open_item`
  or `timer`; kind-specific arguments are enforced.
- Start/pause/reset/set through `bronzecast_update_sticker`; all tool actions use
  `/api/stickers`.

**Playout and rendered visual**

- Allow only generic sticker routes, with `PATCH` and `DELETE` still gated by
  existing credentials.
- Capture an empty initial board plus frames with timer running and expired,
  countdown today and past, checked and unchecked todo, and memo title/body variants.
- Fixed OPEN ITEM panel absent; fruit stickers readable and unclipped;
  notifications intact and visually independent; stickers drawn on top as
  expected.

## 10. Acceptance criteria

- [x] One persistent durable board-content domain: `state/stickers.json`.
- [x] A new/missing store is empty; no timer or other sticker is seeded.
- [x] Exactly four supported sticker kinds, with kind-specific storage and
      validated read views.
- [x] No fixed OPEN ITEM panel, open-item code/store/API/tool, `/api/timer`, timer
      MCP tools, or `reminder` alias.
- [x] Timers remain accurate across pauses, restarts, and process downtime without
      continuous persistence writes.
- [x] Countdown uses one documented local date boundary; todos toggle; memos
      contain title and/or body.
- [x] Notifications remain ephemeral; character and video pipeline unchanged.
- [x] Retained v1 sticker records keep explicit coordinates; fruit visual style
      preserved; angle persists; rotated stickers clamp onto the canvas.
- [x] Read-only MCP snapshot returns the latest complete rendered board as PNG
      image content for placement iteration.
- [x] Rust, MCP, and playout tests pass; the live encoded frame visually matches
      the intended unified sticker approach.
- [x] Documentation and agent-facing tool descriptions reflect the new contract,
      with no old-domain references.

## 11. Resolved design decisions

- **Open item and `reminder` collapse into `memo`.** They are freeform persistent
  text; `todo` stays a distinct kind because it has completion state.
- **The initial board is empty.** No timer, memo, or other sticker is seeded.
- **Every creation is agent-directed.** `x` and `y` are required; the renderer
  does not choose a position. Timer creation also requires explicit duration.
- **Timer duration is agent-supplied.** No 25-minute or other duration is
  implicitly assigned to a timer sticker.
- **Do not auto-create a memo** to fill the upper-right void. It is agent content,
  not system chrome.
- **Do not pretend a memo is a todo:** memo has no completion state; todo has a
  checkbox.
- **A finished timer can be started again.** `start` after `done` restarts the
  full duration; it is not a 409 error.
- **Coordinates are required on create** and are always agent-supplied; accepted
  coordinates are preserved except where rotated bounds require clamping.
- **Timers are not singletons.** Every timer has a sticker ID and is operated
  through generic sticker CRUD.
- **Persist a running clock anchor, not second-by-second tick writes.** This makes
  timer recovery deterministic.
- **No runtime compatibility.** Remove the obsolete systems rather than hiding
  them behind adapters.
- **Derived view values never become writable storage fields.** Renderer, REST,
  and MCP share one calculation and validation contract.
- **No migration.** The only allowed state transition is a separately approved
  one-time offline conversion; the runtime never reads legacy files.
