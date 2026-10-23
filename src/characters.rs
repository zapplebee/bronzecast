//! Character registry, asset loading, and selection API.
//!
//! To add a character, edit characters/manifest.json and add assets, not this file.
//! All GPU work stays on Macroquad's main thread. HTTP handlers only set an ID.
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub const MANIFEST_PATH: &str = "characters/manifest.json";
pub const FLOOR_Y: f32 = -2.41;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterSpec {
    pub id: String,
    pub name: String,
    pub obj: String,
    pub rotation_degrees: [f32; 3],
    pub height: f32,
    pub max_width: f32,
    pub max_depth: f32,
    pub bounce: bool,
    #[serde(default)]
    pub exclude_groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub default_character: String,
    pub characters: Vec<CharacterSpec>,
}

pub struct LoadedCharacter {
    pub spec: CharacterSpec,
    pub meshes: Vec<Mesh>,
}

#[derive(Clone, Serialize)]
pub struct Selection {
    pub active_id: String,
    pub requested_id: String,
    pub characters: Vec<CharacterSpec>,
}
pub type SharedSelection = Arc<Mutex<Selection>>;

fn local_asset(base: &Path, relative: &str) -> Result<PathBuf, String> {
    let base = base.canonicalize().map_err(|e| e.to_string())?;
    let path = base
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("{relative}: {e}"))?;
    if !path.starts_with(&base) || !path.is_file() {
        return Err(format!(
            "Asset must be a file inside {}: {relative}",
            base.display()
        ));
    }
    Ok(path)
}

impl Manifest {
    pub fn read(path: &Path) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let manifest: Self =
            serde_json::from_slice(&data).map_err(|e| format!("{}: {e}", path.display()))?;
        manifest.validate(path.parent().unwrap_or(Path::new(".")))?;
        Ok(manifest)
    }

    fn validate(&self, base: &Path) -> Result<(), String> {
        let mut ids = HashSet::new();
        for spec in &self.characters {
            if spec.id.is_empty()
                || !spec
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                || !ids.insert(&spec.id)
            {
                return Err(format!("Invalid or duplicate character ID: {}", spec.id));
            }
            if spec.name.trim().is_empty()
                || spec.name.chars().count() > 24
                || spec.name.chars().any(char::is_control)
            {
                return Err(format!(
                    "{}: name must be 1–24 printable characters",
                    spec.id
                ));
            }
            for value in [spec.height, spec.max_width, spec.max_depth] {
                if !value.is_finite() || value <= 0.0 || value > 5.0 {
                    return Err(format!(
                        "{}: dimensions must be >0 and <=5 world units",
                        spec.id
                    ));
                }
            }
            if !spec.rotation_degrees.iter().all(|x| x.is_finite()) {
                return Err(format!("{}: rotation must be finite", spec.id));
            }
            local_asset(base, &spec.obj)?;
        }
        if !ids.contains(&self.default_character) {
            return Err("default_character must name a registered character".into());
        }
        Ok(())
    }
}

fn read_obj(
    base: &Path,
    spec: &CharacterSpec,
) -> Result<(Vec<tobj::Model>, Vec<tobj::Material>, PathBuf), String> {
    let path = local_asset(base, &spec.obj)?;
    let (mut models, materials) = tobj::load_obj(
        &path,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
    )
    .map_err(|e| format!("{}: {e}", spec.id))?;
    let materials = materials.map_err(|e| format!("{} materials: {e}", spec.id))?;
    for excluded in &spec.exclude_groups {
        if !models.iter().any(|m| &m.name == excluded) {
            return Err(format!(
                "{}: excluded group does not exist: {excluded}",
                spec.id
            ));
        }
    }
    models.retain(|m| !spec.exclude_groups.contains(&m.name));
    if models.is_empty() || models.iter().all(|m| m.mesh.indices.is_empty()) {
        return Err(format!("{}: no visible geometry", spec.id));
    }
    for model in &models {
        let mesh = &model.mesh;
        if mesh
            .positions
            .iter()
            .chain(mesh.normals.iter())
            .chain(mesh.texcoords.iter())
            .any(|v| !v.is_finite())
            || mesh
                .indices
                .iter()
                .any(|&i| i as usize >= mesh.positions.len() / 3)
            || mesh.material_id.is_some_and(|i| i >= materials.len())
        {
            return Err(format!("{}: invalid geometry in {}", spec.id, model.name));
        }
    }
    Ok((models, materials, path.parent().unwrap().to_owned()))
}

/// Call only from the rendering thread after Macroquad initialization.
pub fn load_all() -> Result<(Vec<LoadedCharacter>, SharedSelection), String> {
    let manifest = Manifest::read(Path::new(MANIFEST_PATH))?;
    let mut loaded = Vec::new();
    for spec in &manifest.characters {
        loaded.push(load(Path::new("characters"), spec)?);
    }
    let selection = Arc::new(Mutex::new(Selection {
        active_id: manifest.default_character.clone(),
        requested_id: manifest.default_character,
        characters: manifest.characters,
    }));
    Ok((loaded, selection))
}

fn load(base: &Path, spec: &CharacterSpec) -> Result<LoadedCharacter, String> {
    let (models, materials, folder) = read_obj(base, spec)?;
    let mut texture_cache: HashMap<PathBuf, Texture2D> = HashMap::new();
    let mut textures = Vec::new();
    for material in &materials {
        let texture = if let Some(filename) = &material.diffuse_texture {
            let path = local_asset(&folder, filename)?;
            if let Some(texture) = texture_cache.get(&path) {
                texture.clone()
            } else {
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                // Decode explicitly so invalid images produce a helpful error rather than a GPU panic.
                let image = Image::from_file_with_format(&bytes, None)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                let texture = Texture2D::from_image(&image);
                texture_cache.insert(path, texture.clone());
                texture
            }
        } else {
            let c = material.diffuse.unwrap_or([1.0; 3]);
            Texture2D::from_image(&Image::gen_image_color(
                1,
                1,
                Color::new(c[0], c[1], c[2], 1.0),
            ))
        };
        texture.set_filter(FilterMode::Nearest);
        textures.push(texture);
    }
    // Rotate first, then measure all groups together. Never center individual body parts.
    let [x, y, z] = spec.rotation_degrees.map(f32::to_radians);
    let rotation = Quat::from_rotation_z(z) * Quat::from_rotation_y(y) * Quat::from_rotation_x(x);
    let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for model in &models {
        for p in model.mesh.positions.chunks_exact(3) {
            let p = rotation * vec3(p[0], p[1], p[2]);
            min = min.min(p);
            max = max.max(p);
        }
    }
    let extent = max - min;
    if !extent.is_finite() || extent.min_element() <= 0.0 {
        return Err(format!("{}: invalid bounds", spec.id));
    }
    let scale = (spec.height / extent.y)
        .min(spec.max_width / extent.x)
        .min(spec.max_depth / extent.z);
    let origin = vec3((min.x + max.x) * 0.5, min.y, (min.z + max.z) * 0.5);
    let mut meshes = Vec::new();
    for model in models {
        let mesh = model.mesh;
        let positions: Vec<_> = mesh
            .positions
            .chunks_exact(3)
            .map(|p| {
                (rotation * vec3(p[0], p[1], p[2]) - origin) * scale
                    + vec3(0.0, FLOOR_Y + 0.01, 0.0)
            })
            .collect();
        let mut normals = vec![Vec3::ZERO; positions.len()];
        for f in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [f[0] as usize, f[1] as usize, f[2] as usize];
            let n = (positions[b] - positions[a]).cross(positions[c] - positions[a]);
            normals[a] += n;
            normals[b] += n;
            normals[c] += n;
        }
        for batch in mesh.indices.chunks(60_000) {
            let vertices: Vec<_> = batch
                .iter()
                .map(|&i| {
                    let i = i as usize;
                    let normal = if mesh.normals.len() >= i * 3 + 3 {
                        rotation
                            * vec3(
                                mesh.normals[i * 3],
                                mesh.normals[i * 3 + 1],
                                mesh.normals[i * 3 + 2],
                            )
                    } else {
                        normals[i]
                    };
                    let uv = if mesh.texcoords.len() >= i * 2 + 2 {
                        vec2(mesh.texcoords[i * 2], 1.0 - mesh.texcoords[i * 2 + 1])
                    } else {
                        Vec2::ZERO
                    };
                    Vertex {
                        position: positions[i],
                        normal: normal.normalize_or_zero().extend(0.0),
                        uv,
                        color: WHITE.into(),
                    }
                })
                .collect();
            meshes.push(Mesh {
                indices: (0..vertices.len() as u16).collect(),
                vertices,
                texture: mesh.material_id.and_then(|i| textures.get(i)).cloned(),
            });
        }
    }
    println!(
        "Loaded {}: {} draw batches, {:.2} units tall",
        spec.id,
        meshes.len(),
        extent.y * scale
    );
    Ok(LoadedCharacter {
        spec: spec.clone(),
        meshes,
    })
}

/// At the start of each rendered frame, atomically acknowledge the latest selection.
pub fn active_index(loaded: &[LoadedCharacter], shared: &SharedSelection) -> usize {
    let mut state = shared.lock().unwrap();
    let index = loaded
        .iter()
        .position(|c| c.spec.id == state.requested_id)
        .expect("validated character ID");
    state.active_id = state.requested_id.clone();
    index
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwitchRequest {
    id: String,
}

async fn status(State(shared): State<SharedSelection>) -> Json<Selection> {
    Json(shared.lock().unwrap().clone())
}

async fn switch(
    State(shared): State<SharedSelection>,
    Json(request): Json<SwitchRequest>,
) -> Result<(StatusCode, Json<Selection>), (StatusCode, String)> {
    let mut state = shared.lock().unwrap();
    if !state.characters.iter().any(|c| c.id == request.id) {
        return Err((
            StatusCode::NOT_FOUND,
            format!(
                "Unknown character '{}'; see GET /api/characters",
                request.id
            ),
        ));
    }
    state.requested_id = request.id;
    Ok((StatusCode::ACCEPTED, Json(state.clone())))
}

pub fn routes(shared: SharedSelection) -> Router {
    Router::new()
        .route("/api/characters", get(status))
        .route("/api/character", get(status).post(switch))
        .with_state(shared)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_assets_are_complete() {
        let manifest = Manifest::read(Path::new(MANIFEST_PATH)).unwrap();
        assert!(manifest.characters.len() >= 3);
        for spec in &manifest.characters {
            let (models, materials, folder) = read_obj(Path::new("characters"), spec).unwrap();
            assert!(models.iter().any(|m| !m.mesh.indices.is_empty()));
            for material in materials {
                if let Some(filename) = material.diffuse_texture {
                    let path = local_asset(&folder, &filename).unwrap();
                    let bytes = std::fs::read(path).unwrap();
                    Image::from_file_with_format(&bytes, None).unwrap();
                }
            }
        }
    }

    #[tokio::test]
    async fn switching_validates_ids_and_preserves_active_until_render() {
        let manifest = Manifest::read(Path::new(MANIFEST_PATH)).unwrap();
        let state = Arc::new(Mutex::new(Selection {
            active_id: "mog".into(),
            requested_id: "mog".into(),
            characters: manifest.characters,
        }));
        let result = switch(
            State(state.clone()),
            Json(SwitchRequest {
                id: "chocobo".into(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(result.0, StatusCode::ACCEPTED);
        assert_eq!(state.lock().unwrap().active_id, "mog");
        assert_eq!(state.lock().unwrap().requested_id, "chocobo");
        let loaded: Vec<_> = state
            .lock()
            .unwrap()
            .characters
            .iter()
            .map(|spec| LoadedCharacter {
                spec: spec.clone(),
                meshes: Vec::new(),
            })
            .collect();
        let index = active_index(&loaded, &state);
        assert_eq!(loaded[index].spec.id, "chocobo");
        assert_eq!(state.lock().unwrap().active_id, "chocobo");
        let error = switch(
            State(state.clone()),
            Json(SwitchRequest {
                id: "../../secret".into(),
            }),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.0, StatusCode::NOT_FOUND);
        assert_eq!(state.lock().unwrap().requested_id, "chocobo");
    }

    #[test]
    fn duplicate_ids_and_invalid_dimensions_are_rejected() {
        let mut manifest = Manifest::read(Path::new(MANIFEST_PATH)).unwrap();
        manifest.characters.push(manifest.characters[0].clone());
        assert!(manifest.validate(Path::new("characters")).is_err());
        manifest.characters.pop();
        manifest.characters[0].height = 0.0;
        assert!(manifest.validate(Path::new("characters")).is_err());
    }

    #[test]
    fn asset_paths_cannot_escape_character_folder() {
        assert!(local_asset(Path::new("characters/mog"), "../../Cargo.toml").is_err());
    }
}
