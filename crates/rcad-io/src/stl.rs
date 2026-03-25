//! STL file format support
//!
//! Supports both ASCII and binary STL formats.

use crate::{ImportOptions, ImportedMesh, ImportedModel, IoError, Result};
use rcad_geometry::Mesh;
use std::io::{BufRead, BufReader, Read, Write};

/// Import an STL file
pub fn import<R: Read>(reader: R, options: &ImportOptions) -> Result<ImportedModel> {
    let mut reader = BufReader::new(reader);

    // Peek at first bytes to determine format
    let mut header = [0u8; 80];
    reader.read_exact(&mut header)?;

    // Check if it looks like ASCII
    let is_ascii = header.starts_with(b"solid") && !header[5..].starts_with(b" \0");

    let mesh = if is_ascii {
        // Re-read as ASCII
        let mut full_reader = BufReader::new(std::io::Cursor::new(header.to_vec()).chain(reader));
        import_ascii(&mut full_reader, options)?
    } else {
        import_binary(&mut reader, &header, options)?
    };

    let imported_mesh = ImportedMesh {
        name: "STL Import".to_string(),
        mesh,
        material_index: None,
    };

    Ok(ImportedModel {
        meshes: vec![imported_mesh],
        ..Default::default()
    })
}

fn import_ascii<R: BufRead>(reader: &mut R, options: &ImportOptions) -> Result<Mesh> {
    let mut mesh = Mesh::new();
    let mut line = String::new();
    let mut current_normal = [0.0f32; 3];
    let mut vertex_count = 0;

    while reader.read_line(&mut line)? > 0 {
        let trimmed = line.trim();

        if trimmed.starts_with("facet normal") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 5 {
                current_normal = [
                    parts[2].parse().unwrap_or(0.0),
                    parts[3].parse().unwrap_or(0.0),
                    parts[4].parse().unwrap_or(0.0),
                ];
            }
        } else if trimmed.starts_with("vertex") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                let x: f32 = parts[1].parse().unwrap_or(0.0) * options.scale as f32;
                let y: f32 = parts[2].parse().unwrap_or(0.0) * options.scale as f32;
                let z: f32 = parts[3].parse().unwrap_or(0.0) * options.scale as f32;

                mesh.positions.push(x);
                mesh.positions.push(y);
                mesh.positions.push(z);

                mesh.normals.push(current_normal[0]);
                mesh.normals.push(current_normal[1]);
                mesh.normals.push(current_normal[2]);

                mesh.indices.push(vertex_count);
                vertex_count += 1;
            }
        }

        line.clear();
    }

    if options.flip_normals {
        mesh.flip_normals();
    }

    Ok(mesh)
}

fn import_binary<R: Read>(reader: &mut R, header: &[u8; 80], options: &ImportOptions) -> Result<Mesh> {
    let mut mesh = Mesh::new();

    // Read triangle count
    let mut count_bytes = [0u8; 4];
    reader.read_exact(&mut count_bytes)?;
    let triangle_count = u32::from_le_bytes(count_bytes);

    // Reserve space
    let vertex_count = triangle_count as usize * 3;
    mesh.positions.reserve(vertex_count * 3);
    mesh.normals.reserve(vertex_count * 3);
    mesh.indices.reserve(vertex_count);

    // Read triangles
    for i in 0..triangle_count {
        // Normal (3 floats)
        let mut normal_bytes = [0u8; 12];
        reader.read_exact(&mut normal_bytes)?;
        let normal = [
            f32::from_le_bytes([normal_bytes[0], normal_bytes[1], normal_bytes[2], normal_bytes[3]]),
            f32::from_le_bytes([normal_bytes[4], normal_bytes[5], normal_bytes[6], normal_bytes[7]]),
            f32::from_le_bytes([normal_bytes[8], normal_bytes[9], normal_bytes[10], normal_bytes[11]]),
        ];

        // 3 vertices (3 floats each)
        for j in 0..3 {
            let mut vertex_bytes = [0u8; 12];
            reader.read_exact(&mut vertex_bytes)?;

            let x = f32::from_le_bytes([vertex_bytes[0], vertex_bytes[1], vertex_bytes[2], vertex_bytes[3]])
                * options.scale as f32;
            let y = f32::from_le_bytes([vertex_bytes[4], vertex_bytes[5], vertex_bytes[6], vertex_bytes[7]])
                * options.scale as f32;
            let z = f32::from_le_bytes([vertex_bytes[8], vertex_bytes[9], vertex_bytes[10], vertex_bytes[11]])
                * options.scale as f32;

            mesh.positions.push(x);
            mesh.positions.push(y);
            mesh.positions.push(z);

            mesh.normals.push(normal[0]);
            mesh.normals.push(normal[1]);
            mesh.normals.push(normal[2]);

            mesh.indices.push(i * 3 + j);
        }

        // Attribute byte count (ignored)
        let mut attr_bytes = [0u8; 2];
        reader.read_exact(&mut attr_bytes)?;
    }

    if options.flip_normals {
        mesh.flip_normals();
    }

    Ok(mesh)
}

/// Export to STL format
pub fn export<W: Write>(writer: &mut W, mesh: &Mesh, binary: bool) -> Result<()> {
    if binary {
        export_binary(writer, mesh)
    } else {
        export_ascii(writer, mesh)
    }
}

fn export_ascii<W: Write>(writer: &mut W, mesh: &Mesh) -> Result<()> {
    writeln!(writer, "solid rcad_export")?;

    for i in 0..mesh.triangle_count() {
        let idx = i * 3;
        let i0 = mesh.indices[idx] as usize;
        let i1 = mesh.indices[idx + 1] as usize;
        let i2 = mesh.indices[idx + 2] as usize;

        // Get vertices
        let v0 = get_vertex(mesh, i0);
        let v1 = get_vertex(mesh, i1);
        let v2 = get_vertex(mesh, i2);

        // Compute face normal
        let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
        let normal = cross(e1, e2);
        let normal = normalize(normal);

        writeln!(writer, "  facet normal {} {} {}", normal[0], normal[1], normal[2])?;
        writeln!(writer, "    outer loop")?;
        writeln!(writer, "      vertex {} {} {}", v0[0], v0[1], v0[2])?;
        writeln!(writer, "      vertex {} {} {}", v1[0], v1[1], v1[2])?;
        writeln!(writer, "      vertex {} {} {}", v2[0], v2[1], v2[2])?;
        writeln!(writer, "    endloop")?;
        writeln!(writer, "  endfacet")?;
    }

    writeln!(writer, "endsolid rcad_export")?;

    Ok(())
}

fn export_binary<W: Write>(writer: &mut W, mesh: &Mesh) -> Result<()> {
    // Header (80 bytes)
    let header = b"Binary STL exported by rCAD                                                    ";
    writer.write_all(header)?;

    // Triangle count
    let triangle_count = mesh.triangle_count() as u32;
    writer.write_all(&triangle_count.to_le_bytes())?;

    // Triangles
    for i in 0..mesh.triangle_count() {
        let idx = i * 3;
        let i0 = mesh.indices[idx] as usize;
        let i1 = mesh.indices[idx + 1] as usize;
        let i2 = mesh.indices[idx + 2] as usize;

        let v0 = get_vertex(mesh, i0);
        let v1 = get_vertex(mesh, i1);
        let v2 = get_vertex(mesh, i2);

        // Compute face normal
        let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
        let normal = cross(e1, e2);
        let normal = normalize(normal);

        // Write normal
        writer.write_all(&normal[0].to_le_bytes())?;
        writer.write_all(&normal[1].to_le_bytes())?;
        writer.write_all(&normal[2].to_le_bytes())?;

        // Write vertices
        for v in &[v0, v1, v2] {
            writer.write_all(&v[0].to_le_bytes())?;
            writer.write_all(&v[1].to_le_bytes())?;
            writer.write_all(&v[2].to_le_bytes())?;
        }

        // Attribute byte count
        writer.write_all(&0u16.to_le_bytes())?;
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_ascii() {
        let mut mesh = Mesh::new();
        mesh.positions = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.5, 1.0, 0.0];
        mesh.normals = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        mesh.indices = vec![0, 1, 2];

        let mut output = Vec::new();
        export_ascii(&mut output, &mesh).unwrap();

        let content = String::from_utf8(output).unwrap();
        assert!(content.contains("solid rcad_export"));
        assert!(content.contains("facet normal"));
        assert!(content.contains("endsolid rcad_export"));
    }
}
