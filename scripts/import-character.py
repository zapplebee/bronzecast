"""Convert a Collada character to the runtime's Y-up OBJ/MTL format using Blender.

blender --background --factory-startup --python scripts/import-character.py -- INPUT.dae OUTPUT_DIR

Preserves evaluated mesh placement, UVs and PNG textures. Vertex-color-only models
are converted to per-face palette materials, so tobj can render their colors.
Original source assets are never modified. Requires Blender with Collada support.
"""
import pathlib
import sys

import bpy

source, destination = map(pathlib.Path, sys.argv[sys.argv.index('--') + 1:])
source, destination = source.resolve(), destination.resolve()
destination.mkdir(parents=True, exist_ok=True)
if (destination / 'model.obj').exists():
    raise SystemExit('Refusing to overwrite an existing model.obj; use a new output folder.')
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
bpy.ops.wm.collada_import(filepath=str(source))
bpy.context.scene.frame_set(1)
meshes = [o for o in bpy.context.scene.objects if o.type == 'MESH']
print('Imported meshes:', [(o.name, len(o.data.vertices), list(o.data.color_attributes.keys())) for o in meshes])
if not meshes:
    raise SystemExit('Source contains no meshes')

for image in bpy.data.images:
    if image.source != 'FILE':
        continue
    image.reload()
    # Collada images can be lazy-loaded in a background Blender process.
    if len(image.pixels) == 0:
        raise RuntimeError(f'Missing image: {image.filepath}')
    name = pathlib.Path(image.filepath).stem + '.png'
    image.filepath_raw = str(destination / name)
    image.file_format = 'PNG'
    image.save()

# Bake modifiers/armature placement before exporting a static rest-pose asset.
bpy.ops.object.select_all(action='DESELECT')
for obj in meshes:
    obj.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]
bpy.ops.object.convert(target='MESH')
for obj in [o for o in bpy.context.selected_objects if o.type == 'MESH']:
    mesh = obj.data
    color = mesh.color_attributes.active_color
    if color is None:
        continue
    # OBJ has no portable vertex-color standard: use a deterministic palette.
    palette = {}
    for polygon in mesh.polygons:
        samples = [color.data[loop if color.domain == 'CORNER' else mesh.loops[loop].vertex_index].color
                   for loop in polygon.loop_indices]
        rgba = tuple(round(sum(c[channel] for c in samples) / len(samples), 3) for channel in range(4))
        if rgba not in palette:
            material = bpy.data.materials.new(f'palette_{len(palette):03}')
            material.diffuse_color = rgba
            mesh.materials.append(material)
            palette[rgba] = len(mesh.materials) - 1
        polygon.material_index = palette[rgba]
    print(obj.name, 'palette colors:', len(palette))

bpy.ops.wm.obj_export(
    filepath=str(destination / 'model.obj'), export_selected_objects=True,
    forward_axis='NEGATIVE_Z', up_axis='Y', export_materials=True,
    export_normals=True, export_uv=True, export_triangulated_mesh=True,
    path_mode='RELATIVE', apply_modifiers=True,
)
print('Exported:', destination / 'model.obj')
