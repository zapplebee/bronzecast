# Asset provenance and conversion notes

Original source files in `scratch/` and the former root `mog_character/` were preserved.

| ID | Source | Runtime conversion |
|---|---|---|
| `mog` | User-supplied `mog_character/` (Final Fantasy VII Mog) | Copied OBJ, MTL, and seven PNGs unchanged into `characters/mog/`; Y-up, turned 180° around Y in the manifest. |
| `chocobo` | `scratch/PlayStation - Final Fantasy VII - Non-Playable Characters - Chocobo (Field)/PS1-FinalFantasyVII-Chocobo-Field/PS1-FinalFantasyVII-Chocobo-Field.dae` | Blender 4.3.2 via `scripts/import-character.py`; armature evaluated at frame 1, vertex colors converted to nine per-face palette materials, exported Y-up OBJ. The supplied variant has pale/white feathers, a yellow beak and blue eyes; those source colors are preserved. |
| `monty-mole` | `scratch/Monty Mole/Monty Mole/npc009_choropoo.dae` and its PNGs | Same importer; Y-up OBJ with body/eye textures. Three duplicate `(Layer 2)` specular-only meshes are excluded by exact group name in the manifest; diffuse counterparts remain. |

Monty Mole source README: “Monty Mole from Mario Party 10 ripped by DogToon64.
If used, give credit to Nintendo. Credit to DogToon64 is optional, but would be nice.”

Asset ownership and original usage terms remain with the source authors/rights holders.
