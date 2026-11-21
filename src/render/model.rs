//! 3D models: read from GLB files that blender/build_models.py exports.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};

/// One corner of a triangle, as the GPU reads it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    /// 1 for parts that shine with the palette colour (eyes, cracks), 0 for
    /// parts in chrome or glass. 2 to 3 for loudspeaker cones, which are
    /// chrome or glass but move: 2 + how far this corner moves on the kick
    /// (1 in the middle of the cone, 0 at its rim).
    pub glow: f32,
}

pub struct Model {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// The folder with the GLB files: inside the app when packed as one, else the project's.
pub fn default_dir() -> PathBuf {
    super::resources::find("models", "assets/models")
}

pub fn load(path: &Path) -> Result<Model> {
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    from_bytes(&bytes).with_context(|| format!("{} is not a usable model", path.display()))
}

/// Reads every triangle mesh in a GLB file into one model, centred on the
/// origin and scaled so its longest side is 2 units.
pub fn from_bytes(bytes: &[u8]) -> Result<Model> {
    let gltf = gltf::Gltf::from_slice(bytes)?;
    let blob = gltf.blob.as_deref();
    let mut model = Model {
        vertices: Vec::new(),
        indices: Vec::new(),
    };

    let scene = gltf
        .default_scene()
        .or_else(|| gltf.scenes().next())
        .context("the file has no scene")?;
    // Nodes form a tree; each one's place is its parent's place times its own.
    let mut stack: Vec<(gltf::Node, Mat4)> = scene.nodes().map(|n| (n, Mat4::IDENTITY)).collect();
    while let Some((node, parent)) = stack.pop() {
        let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        let glow = node.name().is_some_and(|n| n.ends_with("_glow"));
        let cone = node.name().is_some_and(|n| n.ends_with("_cone"));
        let node_start = model.vertices.len();
        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    continue;
                }
                let reader = primitive.reader(|buffer| match buffer.source() {
                    gltf::buffer::Source::Bin => blob,
                    gltf::buffer::Source::Uri(_) => None, // only self-contained GLB files
                });
                let positions: Vec<[f32; 3]> = reader
                    .read_positions()
                    .context("a mesh has no positions")?
                    .collect();
                let normals: Vec<[f32; 3]> = reader
                    .read_normals()
                    .context("a mesh has no normals")?
                    .collect();
                let first = model.vertices.len() as u32;
                let normal_matrix = Mat3::from_mat4(world).inverse().transpose();
                for (p, n) in positions.iter().zip(&normals) {
                    model.vertices.push(Vertex {
                        pos: world.transform_point3(Vec3::from(*p)).into(),
                        normal: (normal_matrix * Vec3::from(*n))
                            .normalize_or(Vec3::Y)
                            .into(),
                        glow: glow as u8 as f32,
                    });
                }
                match reader.read_indices() {
                    Some(indices) => model.indices.extend(indices.into_u32().map(|i| first + i)),
                    None => model.indices.extend(first..first + positions.len() as u32),
                }
            }
        }
        if cone {
            pump_weights(&mut model.vertices[node_start..]);
        }
        stack.extend(node.children().map(|child| (child, world)));
    }
    if model.indices.len() < 3 {
        bail!("the file holds no triangles");
    }
    let count = model.vertices.len() as u32;
    if model.indices.iter().any(|i| *i >= count) {
        bail!("the file's triangles point at corners that do not exist");
    }

    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for v in &model.vertices {
        lo = lo.min(Vec3::from(v.pos));
        hi = hi.max(Vec3::from(v.pos));
    }
    let (centre, size) = ((lo + hi) / 2.0, (hi - lo).max_element());
    if !(size > 0.0 && size.is_finite()) {
        bail!("the model has no size");
    }
    for v in &mut model.vertices {
        v.pos = ((Vec3::from(v.pos) - centre) * (2.0 / size)).into();
    }
    Ok(model)
}

/// Marks a loudspeaker cone's corners with how far each moves on the kick:
/// fully in the middle, not at all at the rim, as a real cone flexes. The
/// cone faces the front (+Z), so "middle" is measured across X and Y.
fn pump_weights(cone: &mut [Vertex]) {
    if cone.is_empty() {
        return;
    }
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for v in cone.iter() {
        lo = lo.min(Vec3::from(v.pos));
        hi = hi.max(Vec3::from(v.pos));
    }
    let centre = (lo + hi) / 2.0;
    let rim = ((hi - lo).x.max((hi - lo).y) / 2.0).max(1e-6);
    for v in cone.iter_mut() {
        let r = (Vec3::from(v.pos) - centre).truncate().length() / rim;
        let weight = 1.0 - r.clamp(0.0, 1.0);
        v.glow = 2.0 + weight * weight * (3.0 - 2.0 * weight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::director::OBJECTS;

    #[test]
    fn every_object_has_a_model_that_loads() {
        for name in OBJECTS.iter().skip(1) {
            let model = load(&default_dir().join(format!("{name}.glb")))
                .unwrap_or_else(|e| panic!("{name}: {e:#}"));
            assert!(
                model.indices.len() >= 3 * 20,
                "{name}: only {} indices",
                model.indices.len()
            );
            assert_eq!(model.indices.len() % 3, 0, "{name}");
            let n = model.vertices.len() as u32;
            assert!(
                model.indices.iter().all(|i| *i < n),
                "{name}: an index points past the end"
            );
            for v in &model.vertices {
                let len = v.normal.iter().map(|c| c * c).sum::<f32>().sqrt();
                assert!((len - 1.0).abs() < 0.01, "{name}: a normal of length {len}");
                assert!(v.pos.iter().all(|c| c.is_finite()));
            }
        }
    }

    #[test]
    fn models_are_centred_and_two_units_tall_at_most() {
        for name in OBJECTS.iter().skip(1) {
            let model = load(&default_dir().join(format!("{name}.glb"))).unwrap();
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for v in &model.vertices {
                for i in 0..3 {
                    lo[i] = lo[i].min(v.pos[i]);
                    hi[i] = hi[i].max(v.pos[i]);
                }
            }
            let biggest = (0..3).map(|i| hi[i] - lo[i]).fold(0.0, f32::max);
            assert!((biggest - 2.0).abs() < 0.01, "{name}: {biggest} across");
            for i in 0..3 {
                let centre = (lo[i] + hi[i]) / 2.0;
                assert!(
                    centre.abs() < 0.01,
                    "{name}: centre off by {centre} on axis {i}"
                );
            }
        }
    }

    #[test]
    fn glowing_parts_are_found() {
        for (name, glows) in [
            ("eye", true),
            ("monolith", true),
            ("skull", false),
            ("mirrorball", false),
            ("gem", false),
            ("speakers", true),
            ("planet", true),
            ("corridor", true),
            ("crystal", false),
        ] {
            let model = load(&default_dir().join(format!("{name}.glb"))).unwrap();
            let lit = model
                .vertices
                .iter()
                .filter(|v| (0.5..1.5).contains(&v.glow))
                .count();
            assert_eq!(lit > 0, glows, "{name}: {lit} glowing corners");
            assert!(lit < model.vertices.len(), "{name}: everything glows");
        }
    }

    #[test]
    fn the_mirror_ball_is_made_of_many_flat_tiles() {
        // Flat tiles: corners of one tile share a normal, and there are
        // hundreds of different normals, one per tile and groove.
        let model = load(&default_dir().join("mirrorball.glb")).unwrap();
        let mut normals: Vec<[i32; 3]> = model
            .vertices
            .iter()
            .map(|v| v.normal.map(|c| (c * 200.0).round() as i32))
            .collect();
        normals.sort();
        normals.dedup();
        assert!(
            normals.len() > 1000,
            "only {} different normals",
            normals.len()
        );
        // The rod runs up from the ball: the model is far taller than wide.
        let height = model
            .vertices
            .iter()
            .map(|v| v.pos[1])
            .fold(f32::MIN, f32::max)
            - model
                .vertices
                .iter()
                .map(|v| v.pos[1])
                .fold(f32::MAX, f32::min);
        let width = model
            .vertices
            .iter()
            .map(|v| v.pos[0])
            .fold(f32::MIN, f32::max)
            - model
                .vertices
                .iter()
                .map(|v| v.pos[0])
                .fold(f32::MAX, f32::min);
        assert!(height > 2.0 * width, "{height} tall, {width} wide");
    }

    #[test]
    fn broken_files_are_errors() {
        assert!(from_bytes(b"not a model").is_err());
        assert!(from_bytes(&[]).is_err());
        assert!(load(Path::new("/no/such/model.glb")).is_err());
    }

    #[test]
    fn speaker_cones_pump_in_the_middle_and_hold_at_the_rim() {
        let model = load(&default_dir().join("speakers.glb")).unwrap();
        let cones: Vec<&Vertex> = model.vertices.iter().filter(|v| v.glow >= 2.0).collect();
        assert!(cones.len() > 100, "only {} cone corners", cones.len());
        assert!(cones.iter().all(|v| v.glow <= 3.0));
        assert!(cones.iter().any(|v| v.glow > 2.95), "no corner moves fully");
        assert!(cones.iter().any(|v| v.glow < 2.05), "no corner holds still");
        // Nothing else moves: the cabinet is all plain chrome or glow.
        assert!(model.vertices.iter().all(|v| v.glow < 1.5 || v.glow >= 2.0));
        let still = model.vertices.iter().filter(|v| v.glow == 0.0).count();
        assert!(still > 100, "no cabinet");
    }
}
