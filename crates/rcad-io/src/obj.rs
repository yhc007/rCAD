//! Wavefront OBJ file format support

use crate::{ImportOptions, ImportedMesh, ImportedModel, IoError, Result};
use rcad_geometry::Mesh;
use std::io::{BufRead, BufReader, Read, Write};

/// Import an OBJ file
pub fn import<R: Read>(reader: R, options: &ImportOptions) -> Result<ImportedModel> {
    let reader = BufReader::new(reader);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut texcoords: Vec<[f32; 2]> = Vec::new();

    let mut mesh = Mesh::new();
    let mut vertex_map: std::collections::HashMap<(usize, usize, usize), u32> =
        std::collections::HashMap::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "v" => {
                // Vertex position
                if parts.len() >= 4 {
                    let x: f32 = parts[1].parse().unwrap_or(0.0) * options.scale as f32;
                    let y: f32 = parts[2].parse().unwrap_or(0.0) * options.scale as f32;
                    let z: f32 = parts[3].parse().unwrap_or(0.0) * options.scale as f32;
                    positions.push([x, y, z]);
                }
            }
            "vn" => {
                // Vertex normal
                if parts.len() >= 4 {
                    let x: f32 = parts[1].parse().unwrap_or(0.0);
                    let y: f32 = parts[2].parse().unwrap_or(0.0);
                    let z: f32 = parts[3].parse().unwrap_or(0.0);
                    normals.push([x, y, z]);
                }
            }
            "vt" => {
                // Texture coordinate
                if parts.len() >= 3 {
                    let u: f32 = parts[1].parse().unwrap_or(0.0);
                    let v: f32 = parts[2].parse().unwrap_or(0.0);
                    texcoords.push([u, v]);
                }
            }
            "f" => {
                // Face
                let mut face_indices: Vec<u32> = Vec::new();

                for i in 1..parts.len() {
                    let indices = parse_face_vertex(parts[i]);

                    // OBJ indices are 1-based
                    let pos_idx = indices.0.saturating_sub(1);
                    let tex_idx = indices.1.saturating_sub(1);
                    let norm_idx = indices.2.saturating_sub(1);

                    let key = (pos_idx, tex_idx, norm_idx);

                    let vertex_idx = if let Some(&idx) = vertex_map.get(&key) {
                        idx
                    } else {
                        let idx = mesh.vertex_count() as u32;

                        // Add position
                        if pos_idx < positions.len() {
                            let p = positions[pos_idx];
                            mesh.positions.push(p[0]);
                            mesh.positions.push(p[1]);
                            mesh.positions.push(p[2]);
                        } else {
                            mesh.positions.push(0.0);
                            mesh.positions.push(0.0);
                            mesh.positions.push(0.0);
                        }

                        // Add normal
                        if norm_idx < normals.len() {
                            let n = normals[norm_idx];
                            mesh.normals.push(n[0]);
                            mesh.normals.push(n[1]);
                            mesh.normals.push(n[2]);
                        } else {
                            mesh.normals.push(0.0);
                            mesh.normals.push(0.0);
                            mesh.normals.push(1.0);
                        }

                        // Add UV if we have them
                        if !texcoords.is_empty() {
                            if mesh.uvs.is_none() {
                                mesh.uvs = Some(Vec::new());
                            }
                            if let Some(ref mut uvs) = mesh.uvs {
                                if tex_idx < texcoords.len() {
                                    let t = texcoords[tex_idx];
                                    uvs.push(t[0]);
                                    uvs.push(t[1]);
                                } else {
                                    uvs.push(0.0);
                                    uvs.push(0.0);
                                }
                            }
                        }

                        vertex_map.insert(key, idx);
                        idx
                    };

                    face_indices.push(vertex_idx);
                }

                // Triangulate face
                for i in 1..(face_indices.len() - 1) {
                    mesh.indices.push(face_indices[0]);
                    mesh.indices.push(face_indices[i]);
                    mesh.indices.push(face_indices[i + 1]);
                }
            }
            _ => {}
        }
    }

    // Compute normals if needed
    if options.compute_normals && normals.is_empty() {
        compute_normals(&mut mesh);
    }

    if options.flip_normals {
        mesh.flip_normals();
    }

    let imported_mesh = ImportedMesh {
        name: "OBJ Import".to_string(),
        mesh,
        material_index: None,
    };

    Ok(ImportedModel {
        meshes: vec![imported_mesh],
        ..Default::default()
    })
}

fn parse_face_vertex(s: &str) -> (usize, usize, usize) {
    let parts: Vec<&str> = s.split('/').collect();

    let pos = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);

    let tex = if parts.len() > 1 && !parts[1].is_empty() {
        parts[1].parse().unwrap_or(0)
    } else {
        0
    };

    let norm = if parts.len() > 2 {
        parts[2].parse().unwrap_or(0)
    } else {
        0
    };

    (pos, tex, norm)
}

fn compute_normals(mesh: &mut Mesh) {
    let vertex_count = mesh.vertex_count();

    // Reset normals to zero
    for i in 0..vertex_count * 3 {
        mesh.normals[i] = 0.0;
    }

    // Accumulate face normals
    for i in 0..mesh.triangle_count() {
        let idx = i * 3;
        let i0 = mesh.indices[idx] as usize;
        let i1 = mesh.indices[idx + 1] as usize;
        let i2 = mesh.indices[idx + 2] as usize;

        let v0 = get_vertex(mesh, i0);
        let v1 = get_vertex(mesh, i1);
        let v2 = get_vertex(mesh, i2);

        let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
        let normal = cross(e1, e2);

        for vi in [i0, i1, i2] {
            let ni = vi * 3;
            mesh.normals[ni] += normal[0];
            mesh.normals[ni + 1] += normal[1];
            mesh.normals[ni + 2] += normal[2];
        }
    }

    // Normalize
    for i in 0..vertex_count {
        let ni = i * 3;
        let n = [mesh.normals[ni], mesh.normals[ni + 1], mesh.normals[ni + 2]];
        let normalized = normalize(n);
        mesh.normals[ni] = normalized[0];
        mesh.normals[ni + 1] = normalized[1];
        mesh.normals[ni + 2] = normalized[2];
    }
}

fn get_vertex(mesh: &Mesh, index: usize) -> [f32; 3] {
    let idx = index * 3;
    [
        mesh.positions[idx],
        mesh.positions[idx + 1],
        mesh.positions[idx + 2],
    ]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 1e-10 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// Export to OBJ format
pub fn export<W: Write>(writer: &mut W, mesh: &Mesh) -> Result<()> {
    writeln!(writer, "# Exported by rCAD")?;
    writeln!(writer, "# Vertices: {}", mesh.vertex_count())?;
    writeln!(writer, "# Triangles: {}", mesh.triangle_count())?;
    writeln!(writer)?;

    // Write vertices
    for i in 0..mesh.vertex_count() {
        let idx = i * 3;
        writeln!(
            writer,
            "v {} {} {}",
            mesh.positions[idx],
            mesh.positions[idx + 1],
            mesh.positions[idx + 2]
        )?;
    }

    writeln!(writer)?;

    // Write normals
    for i in 0..mesh.vertex_count() {
        let idx = i * 3;
        writeln!(
            writer,
            "vn {} {} {}",
            mesh.normals[idx],
            mesh.normals[idx + 1],
            mesh.normals[idx + 2]
        )?;
    }

    // Write UVs if present
    if let Some(ref uvs) = mesh.uvs {
        writeln!(writer)?;
        for i in 0..(uvs.len() / 2) {
            let idx = i * 2;
            writeln!(writer, "vt {} {}", uvs[idx], uvs[idx + 1])?;
        }
    }

    writeln!(writer)?;

    // Write faces
    let has_uvs = mesh.uvs.is_some();
    for i in 0..mesh.triangle_count() {
        let idx = i * 3;
        let i0 = mesh.indices[idx] as usize + 1;
        let i1 = mesh.indices[idx + 1] as usize + 1;
        let i2 = mesh.indices[idx + 2] as usize + 1;

        if has_uvs {
            writeln!(
                writer,
                "f {}/{}/{} {}/{}/{} {}/{}/{}",
                i0, i0, i0, i1, i1, i1, i2, i2, i2
            )?;
        } else {
            writeln!(writer, "f {}//{} {}//{} {}//{}", i0, i0, i1, i1, i2, i2)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_face_vertex() {
        assert_eq!(parse_face_vertex("1"), (1, 0, 0));
        assert_eq!(parse_face_vertex("1/2"), (1, 2, 0));
        assert_eq!(parse_face_vertex("1/2/3"), (1, 2, 3));
        assert_eq!(parse_face_vertex("1//3"), (1, 0, 3));
    }

    #[test]
    fn test_export() {
        let mut mesh = Mesh::new();
        mesh.positions = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.5, 1.0, 0.0];
        mesh.normals = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        mesh.indices = vec![0, 1, 2];

        let mut output = Vec::new();
        export(&mut output, &mesh).unwrap();

        let content = String::from_utf8(output).unwrap();
        assert!(content.contains("# Exported by rCAD"));
        assert!(content.contains("v 0"));
        assert!(content.contains("vn 0"));
        assert!(content.contains("f "));
    }
}
