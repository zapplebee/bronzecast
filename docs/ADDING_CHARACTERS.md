# Adding a character — follow this recipe

## The rule

**Adding a standard character requires assets + one JSON entry, not Rust changes.**
Use the existing three entries in `characters/manifest.json` as working examples.

- Runtime format: assembled **OBJ + MTL + PNG textures**, or MTL `Kd` palette colors.
- Geometry is Y-up after manifest rotation; it faces the camera toward negative Z.
- Models share the same shader, three rendering passes, animation, lighting, floor,
  dashboard, notifications, HLS stream, and Cast session.
- Selection is server-global. Every video client sees the same selected character.

## 1. Put assets in a dedicated folder

Choose a lower-case hyphenated ID, e.g. `new-character`. Put its complete asset set
inside `characters/new-character/`. Keep relative MTL/texture references valid.
Never point the manifest at `scratch/` or overwrite the user's original files.

For a supplied OBJ, copy its MTL and texture files together. For a Collada/DAE
source, use the checked-in Blender importer (Blender 4.3.2 was used here):

```sh
"/Applications/Blender.app/Contents/MacOS/Blender" \
  --background --factory-startup --python-exit-code 1 \
  --python scripts/import-character.py -- \
  "scratch/path/to/source.dae" "characters/new-character"
```

It emits `model.obj`, `model.mtl`, and PNGs. It refuses to overwrite `model.obj`.
It evaluates the source armature/modifiers at frame 1, so this imports a **static
assembled pose**, not skeletal animation clips. Vertex-color-only assets become
per-face averaged palette materials (Chocobo demonstrates this); textures/UVs are
preserved (Monty Mole demonstrates this). Inspect source/license notes and add a
row to `characters/SOURCES.md`.

## 2. Add one manifest entry

```json
{
  "id": "new-character",
  "name": "New Character",
  "obj": "new-character/model.obj",
  "rotation_degrees": [0, 180, 0],
  "height": 4.8,
  "max_width": 3.8,
  "max_depth": 3.5,
  "bounce": true,
  "exclude_groups": []
}
```

| Field | Meaning |
|---|---|
| `id` | Unique API identifier; only `a-z`, `0-9`, `-`. |
| `name` | UI label, 1–24 printable characters. No hardcoded character labels needed. |
| `obj` | File path **relative to `characters/`**, with MTL beside the OBJ. |
| `rotation_degrees` | `[X,Y,Z]` rotations, applied X then Y then Z **before measuring bounds**. Use Y=180 if the model shows its back. For a Z-up OBJ, typically X=-90 first, then inspect facing. |
| `height` | Desired height in world units; positive and at most 5. |
| `max_width`, `max_depth` | Positive limits (at most 5). Uniform scale is the minimum scale needed to fit all three limits. Wide models may be shorter than `height`; adjust the limits in JSON, not the camera. |
| `bounce` | `true` uses the shared bounce/hover; `false` uses a still pose. |
| `exclude_groups` | Exact OBJ object/group names to omit, e.g. duplicate specular layers. Each name must actually exist; typos fail validation. |

The loader centers **the whole model**, never individual meshes, preserves normals
and UVs, fits it to those bounds, and places its feet on the shared floor. New
models must already be assembled. Do not add per-bone assembly code to the loader.

## 3. Validate before restarting

```sh
cargo test --locked
cargo build --release --locked
```

Tests check the actual bundled OBJ/MTL/texture files, image decoding, exclusion
names, registry constraints, and switch behavior. Startup additionally checks
and loads every registered model onto the GPU; a bad entry reports an error and
stops startup rather than advertising a broken character.

The manifest is read at startup. Adding or changing an entry requires one server
restart. Switching between already-loaded entries requires **no restart**.
Notifications and the current selection are in memory; restart resets them.
Preserve any wanted notification contents with `GET /api/notifications` before
restarting, and repost those contents afterward. The selection resets to the
manifest's top-level `default_character`.

## 4. Switch through the API

```sh
# List IDs/configuration and the current/requested character.
curl http://localhost:8080/api/characters

# Switch. Returns HTTP 202 Accepted.
curl -X POST http://localhost:8080/api/character \
  -H 'Content-Type: application/json' \
  -d '{"id":"new-character"}'

# Confirm the renderer has acknowledged the request.
curl http://localhost:8080/api/character
```

`requested_id` updates on POST; `active_id` updates on the next rendering frame.
The existing HLS feed then shows it after the normal playback delay. No recast is
needed for a switch. Unknown IDs return 404 and leave the selection unchanged.
Malformed JSON/schema errors return 400/422. The endpoint accepts IDs, never file
paths or uploaded mesh data. If several requests arrive before one frame, the
latest accepted ID wins.

## 5. Visually verify — do not stop at “cargo check passed”

```sh
python3 scripts/inspect-stream.py
```

The script captures one full 32-second lighting cycle from the **encoded video**,
with a contact sheet and enlarged crops in `scratch/visual-review/`.
Check all of the following:

1. Face points toward the camera; model is upright, assembled, and has source colors.
2. Texture paths/UV orientation are right; eyes, mouth, feathers/fur are visible.
3. At bounce peak and lateral extremes, nothing enters the upper status box or
   notification panel, and feet/shadow remain near the floor.
4. All parts share the lighting, normal outlines, animation, and projected shadow.
5. Switch back to `mog`, then to the new character again. Verify both `active_id`
   and the character label in the video. Notification contents should remain.
6. Existing Cast playback should continue through switches without relaunching it.

Tune only `rotation_degrees` and dimension limits for orientation/framing. If
missing/duplicate geometry needs correction, fix the imported asset or exclusions.
Do not "fix" one model by changing the global shaders/camera.

## Code map / deliberate limits

- `src/characters.rs`: manifest validation, reusable loader, preloaded meshes,
  API selection state. GPU loading stays on the rendering thread.
- `src/main.rs`: chooses the active entry each frame; all passes use its meshes.
- `src/notifications.rs`: receives the active display name; no model-ID branches.
- `scripts/import-character.py`: offline Blender conversion only. Blender is
  not a server runtime dependency.
- Opaque diffuse textures/material colors are supported. No PBR, skeletal clips,
  specular textures, or alpha-blended hair are implemented. Convert/bake unsupported
  features offline or deliberately extend the shared loader with verification.
- All registered models are preloaded for predictable switches; use modest assets.
