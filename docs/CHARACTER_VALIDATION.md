# Character-system validation

Validated on 2026-10-23 on the existing Mac/Rust/FFmpeg host.

## Automated

- `cargo test --locked`: four tests passed (bundled OBJ/MTL/textures, ID and
  dimension validation, safe asset paths, queued selection/renderer acknowledgement).
- `cargo build --release --locked`: passed.
- `node --check web/player.js`: passed.
- Unknown ID through HTTP returned 404; the active selection was unchanged.

## Encoded visual checks

Used `python3 scripts/inspect-stream.py` after `POST /api/character`, not just
GPU/window screenshots. Reviewed a complete light sweep for each model.

| Character | Checks |
|---|---|
| Mog | Faces camera, original facial/material colors preserved, old framing retained, stays below status box during bounce. |
| Chocobo | Beak, blue eyes, pale source feathers, feet/tail assembled; normalized to fit depth; same light/outline/shadow treatment. |
| Monty Mole | Brown body, white muzzle/belly and eyes visible; duplicate specular shells excluded; wider bounding limit gives a readable size without entering the right panel. |

Local review sets (generated files are ignored):

- `scratch/visual-review/20261023-093154/`: Chocobo
- `scratch/visual-review/20261023-093900/`: Mog with registry loader
- `scratch/visual-review/20261023-093957/`: Monty Mole with final width limit

## Live clients

- Chrome DevTools confirmed the selector contains all three IDs. Selecting
  Chocobo changed `active_id` and the video kept playing (171 decoded frames).
- Office display (`192.168.1.87`) stayed `PLAYING` while switching Chocobo →
  Monty Mole → Mog. Cast media session ID stayed the same and media time advanced.
- `GET /api/notifications` returned identical contents before/after those switches.
- Left the running service and Office display on **Mog**.

These are verified examples, not a claim that arbitrary files will import perfectly.
Follow `ADDING_CHARACTERS.md` and visually validate every new model.
