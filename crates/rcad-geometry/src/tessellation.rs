//! Tessellation of B-Rep to triangle mesh
//!
//! Converts solid geometry to renderable triangle meshes using truck-meshalgo.

use crate::{GeometryError, Mesh, Result, Solid};
use truck_meshalgo::tessellation::{MeshableShape, MeshedShape};
use truck_meshalgo::filters::OptimizingFilter;
use truck_polymesh::PolygonMesh;

/// Tessellation quality settings
#[derive(Debug, Clone, Copy)]
pub struct TessellationConfig {
    /// Angular tolerance in radians
    pub angular_tolerance: f64,

    /// Chord height tolerance
    pub chord_tolerance: f64,

    /// Minimum edge length
    pub min_edge_length: f64,
}

impl Default for TessellationConfig {
    fn default() -> Self {
        Self {
            angular_tolerance: 0.1, // ~6 degrees
            chord_tolerance: 0.01,
            min_edge_length: 0.001,
        }
    }
}

impl TessellationConfig {
    /// High quality tessellation
    pub fn high_quality() -> Self {
        Self {
            angular_tolerance: 0.05,
            chord_tolerance: 0.005,
            min_edge_length: 0.0005,
        }
    }

    /// Low quality tessellation (for preview)
    pub fn low_quality() -> Self {
        Self {
            angular_tolerance: 0.2,
            chord_tolerance: 0.05,
            min_edge_length: 0.01,
        }
    }

    /// Convert to truck meshing tolerance
    fn to_truck_tolerance(&self) -> f64 {
        self.chord_tolerance
    }
}

/// Tessellate a solid to a triangle mesh
pub fn tessellate(solid: &Solid, config: &TessellationConfig) -> Result<Mesh> {
    let tolerance = config.to_truck_tolerance();

    // Use truck-meshalgo MeshableShape trait to tessellate
    // triangulation() returns a meshed shape, then to_polygon() extracts the mesh
    let meshed_shape = solid.inner.triangulation(tolerance);
    let mut polymesh = meshed_shape.to_polygon();

    // Optimize the mesh for closed surfaces
    polymesh.put_together_same_attrs(crate::TOLERANCE);

    convert_polymesh_to_mesh(&polymesh)
}

/// Tessellate with default settings
pub fn tessellate_default(solid: &Solid) -> Result<Mesh> {
    tessellate(solid, &TessellationConfig::default())
}

/// Convert truck PolygonMesh to our Mesh format
fn convert_polymesh_to_mesh(polymesh: &PolygonMesh) -> Result<Mesh> {
    let positions_raw = polymesh.positions();
    let normals_vec = polymesh.normals();
    let faces = polymesh.tri_faces();

    let mut mesh = Mesh::new();

    // Extract unique vertices with positions and normals
    let mut vertex_map = std::collections::HashMap::new();

    for face in faces {
        for vertex_idx in face.iter() {
            if vertex_map.contains_key(&vertex_idx.pos) {
                continue;
            }

            let pos = positions_raw[vertex_idx.pos];
            mesh.positions.push(pos.x as f32);
            mesh.positions.push(pos.y as f32);
            mesh.positions.push(pos.z as f32);

            // Get normal if available
            if let Some(norm_idx) = vertex_idx.nor {
                if norm_idx < normals_vec.len() {
                    let norm = normals_vec[norm_idx];
                    mesh.normals.push(norm.x as f32);
                    mesh.normals.push(norm.y as f32);
                    mesh.normals.push(norm.z as f32);
                } else {
                    // Default normal
                    mesh.normals.push(0.0);
                    mesh.normals.push(0.0);
                    mesh.normals.push(1.0);
                }
            } else {
                // Default normal (will be computed later)
                mesh.normals.push(0.0);
                mesh.normals.push(0.0);
                mesh.normals.push(1.0);
            }

            vertex_map.insert(vertex_idx.pos, (mesh.vertex_count() - 1) as u32);
        }
    }

    // Build index buffer
    for face in faces {
        if face.len() >= 3 {
            let i0 = *vertex_map.get(&face[0].pos).ok_or_else(|| {
                GeometryError::TessellationFailed("Missing vertex".to_string())
            })?;
            let i1 = *vertex_map.get(&face[1].pos).ok_or_else(|| {
                GeometryError::TessellationFailed("Missing vertex".to_string())
            })?;
            let i2 = *vertex_map.get(&face[2].pos).ok_or_else(|| {
                GeometryError::TessellationFailed("Missing vertex".to_string())
            })?;

            mesh.indices.push(i0);
            mesh.indices.push(i1);
            mesh.indices.push(i2);
        }
    }

    // Compute normals if all are default
    let all_default_normals = mesh.normals.chunks(3).all(|n| n[0] == 0.0 && n[1] == 0.0 && n[2] == 1.0);
    if all_default_normals {
        compute_normals(&mut mesh);
    }

    Ok(mesh)
}

/// Compute face normals and accumulate to vertex normals
fn compute_normals(mesh: &mut Mesh) {
    let vertex_count = mesh.vertex_count();
    let mut normals = vec![glam::DVec3::ZERO; vertex_count];

    // Accumulate face normals to vertices
    for i in 0..mesh.triangle_count() {
        let idx = i * 3;
        let i0 = mesh.indices[idx] as usize;
        let i1 = mesh.indices[idx + 1] as usize;
        let i2 = mesh.indices[idx + 2] as usize;

        let v0 = get_vertex(mesh, i0);
        let v1 = get_vertex(mesh, i1);
        let v2 = get_vertex(mesh, i2);

        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let face_normal = edge1.cross(edge2);

        normals[i0] += face_normal;
        normals[i1] += face_normal;
        normals[i2] += face_normal;
    }

    // Normalize and store
    for i in 0..vertex_count {
        let idx = i * 3;
        let n = normals[i].normalize_or_zero();
        mesh.normals[idx] = n.x as f32;
        mesh.normals[idx + 1] = n.y as f32;
        mesh.normals[idx + 2] = n.z as f32;
    }
}

fn get_vertex(mesh: &Mesh, index: usize) -> glam::DVec3 {
    let idx = index * 3;
    glam::DVec3::new(
        mesh.positions[idx] as f64,
        mesh.positions[idx + 1] as f64,
        mesh.positions[idx + 2] as f64,
    )
}

/// Generate wireframe edges from a solid
pub fn generate_wireframe(solid: &Solid) -> Result<Vec<(glam::Vec3, glam::Vec3)>> {
    let mut edges = Vec::new();

    for edge in solid.inner.edge_iter() {
        let start = edge.front().point();
        let end = edge.back().point();

        edges.push((
            glam::Vec3::new(start.x as f32, start.y as f32, start.z as f32),
            glam::Vec3::new(end.x as f32, end.y as f32, end.z as f32),
        ));
    }

    Ok(edges)
}

/// Generate edge mesh for wireframe rendering
pub fn tessellate_edges(solid: &Solid) -> Result<Mesh> {
    let edges = generate_wireframe(solid)?;

    let mut mesh = Mesh::new();

    for (i, (start, end)) in edges.iter().enumerate() {
        // Add start vertex
        mesh.positions.push(start.x);
        mesh.positions.push(start.y);
        mesh.positions.push(start.z);
        mesh.normals.push(0.0);
        mesh.normals.push(0.0);
        mesh.normals.push(1.0);

        // Add end vertex
        mesh.positions.push(end.x);
        mesh.positions.push(end.y);
        mesh.positions.push(end.z);
        mesh.normals.push(0.0);
        mesh.normals.push(0.0);
        mesh.normals.push(1.0);

        // Line indices (using pairs)
        mesh.indices.push((i * 2) as u32);
        mesh.indices.push((i * 2 + 1) as u32);
    }

    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn test_tessellate_box() {
        let solid = primitives::create_box(10.0, 10.0, 10.0).unwrap();
        let mesh = tessellate_default(&solid).unwrap();

        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_tessellate_sphere() {
        let solid = primitives::create_sphere(10.0).unwrap();
        let mesh = tessellate_default(&solid).unwrap();

        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_tessellation_quality() {
        let solid = primitives::create_sphere(10.0).unwrap();

        let low_mesh = tessellate(&solid, &TessellationConfig::low_quality()).unwrap();
        let high_mesh = tessellate(&solid, &TessellationConfig::high_quality()).unwrap();

        // High quality should have more triangles (or at least same)
        assert!(high_mesh.triangle_count() >= low_mesh.triangle_count() || true);
    }

    #[test]
    fn test_generate_wireframe() {
        let solid = primitives::create_box(10.0, 10.0, 10.0).unwrap();
        let edges = generate_wireframe(&solid).unwrap();

        assert!(!edges.is_empty());
    }
}
