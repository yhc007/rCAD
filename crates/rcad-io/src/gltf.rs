//! glTF 2.0 file format support

use crate::{
    ExportOptions, ImportOptions, ImportedMaterial, ImportedMesh, ImportedModel, ImportedNode,
    IoError, Result,
};
use rcad_geometry::Mesh;
use std::io::{Read, Write};

/// Average colour (0..1) of a material's base-colour texture, if one is embedded
/// in the glTF buffers. Server-side only (gated so the WASM build stays lean).
#[cfg(feature = "textures")]
fn base_texture_average(material: &gltf::Material, buffers: &[Vec<u8>]) -> Option<[f32; 3]> {
    let info = material.pbr_metallic_roughness().base_color_texture()?;
    let bytes: &[u8] = match info.texture().source().source() {
        gltf::image::Source::View { view, .. } => {
            let buf = buffers.get(view.buffer().index())?;
            let start = view.offset();
            buf.get(start..start + view.length())?
        }
        gltf::image::Source::Uri { .. } => return None, // GLB embeds via buffer views
    };
    let rgb = image::load_from_memory(bytes).ok()?.to_rgb8();
    let (w, h) = rgb.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    // Sample ~4k pixels for a fast average.
    let step = (((w as u64 * h as u64) / 4096).max(1)) as usize;
    let (mut sr, mut sg, mut sb, mut n) = (0u64, 0u64, 0u64, 0u64);
    for (i, p) in rgb.pixels().enumerate() {
        if i % step != 0 {
            continue;
        }
        sr += p[0] as u64;
        sg += p[1] as u64;
        sb += p[2] as u64;
        n += 1;
    }
    (n > 0).then(|| {
        [
            sr as f32 / n as f32 / 255.0,
            sg as f32 / n as f32 / 255.0,
            sb as f32 / n as f32 / 255.0,
        ]
    })
}

#[cfg(not(feature = "textures"))]
fn base_texture_average(_material: &gltf::Material, _buffers: &[Vec<u8>]) -> Option<[f32; 3]> {
    None
}

/// Import a glTF file
pub fn import<R: Read>(mut reader: R, options: &ImportOptions) -> Result<ImportedModel> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let gltf = gltf::Gltf::from_slice(&data)
        .map_err(|e| IoError::ParseError(format!("glTF parse error: {:?}", e)))?;

    let mut model = ImportedModel::default();

    // Load buffers
    let buffers: Vec<Vec<u8>> = load_buffers(&gltf, &data)?;

    // Import materials
    for material in gltf.materials() {
        let pbr = material.pbr_metallic_roughness();

        // If the base colour lives in a texture (as for Tripo-generated models),
        // fold its average into base_color so downstream consumers get a real
        // colour instead of the usually-white base_color_factor.
        let factor = pbr.base_color_factor();
        let base_color = match base_texture_average(&material, &buffers) {
            Some(avg) => [avg[0] * factor[0], avg[1] * factor[1], avg[2] * factor[2], factor[3]],
            None => factor,
        };

        let imported_material = ImportedMaterial {
            name: material.name().unwrap_or("Unnamed").to_string(),
            base_color,
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            emissive: material.emissive_factor(),
        };

        model.materials.push(imported_material);
    }

    // Import meshes
    for gltf_mesh in gltf.meshes() {
        for primitive in gltf_mesh.primitives() {
            let mesh = import_primitive(&primitive, &buffers, options)?;

            let material_index = primitive.material().index();

            let imported_mesh = ImportedMesh {
                name: gltf_mesh.name().unwrap_or("Unnamed").to_string(),
                mesh,
                material_index,
            };

            model.meshes.push(imported_mesh);
        }
    }

    // Import scene nodes
    for scene in gltf.scenes() {
        for node in scene.nodes() {
            import_node(&node, &mut model.nodes);
        }
    }

    Ok(model)
}

fn load_buffers(gltf: &gltf::Gltf, data: &[u8]) -> Result<Vec<Vec<u8>>> {
    let mut buffers = Vec::new();

    for buffer in gltf.buffers() {
        match buffer.source() {
            gltf::buffer::Source::Bin => {
                // Embedded binary data
                if let Some(blob) = gltf.blob.as_ref() {
                    buffers.push(blob.clone());
                } else {
                    return Err(IoError::ParseError("Missing embedded buffer".to_string()));
                }
            }
            gltf::buffer::Source::Uri(uri) => {
                if uri.starts_with("data:") {
                    // Base64 encoded data
                    let encoded = uri.split(',').nth(1).ok_or_else(|| {
                        IoError::ParseError("Invalid data URI".to_string())
                    })?;
                    let decoded = base64_decode(encoded)?;
                    buffers.push(decoded);
                } else {
                    return Err(IoError::UnsupportedFeature(
                        "External buffer files not supported".to_string(),
                    ));
                }
            }
        }
    }

    Ok(buffers)
}

fn base64_decode(input: &str) -> Result<Vec<u8>> {
    // Simple base64 decoder
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = Vec::new();
    let input = input.as_bytes();
    let mut buf = 0u32;
    let mut bits = 0;

    for &byte in input {
        if byte == b'=' {
            break;
        }

        let value = ALPHABET
            .iter()
            .position(|&c| c == byte)
            .ok_or_else(|| IoError::ParseError("Invalid base64".to_string()))?;

        buf = (buf << 6) | (value as u32);
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

fn import_primitive(
    primitive: &gltf::Primitive<'_>,
    buffers: &[Vec<u8>],
    options: &ImportOptions,
) -> Result<Mesh> {
    let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|v| v.as_slice()));

    let mut mesh = Mesh::new();

    // Read positions
    if let Some(iter) = reader.read_positions() {
        for pos in iter {
            mesh.positions.push(pos[0] * options.scale as f32);
            mesh.positions.push(pos[1] * options.scale as f32);
            mesh.positions.push(pos[2] * options.scale as f32);
        }
    } else {
        return Err(IoError::ParseError("Missing positions".to_string()));
    }

    // Read normals (compute them from the geometry below if the file has none —
    // zero normals become NaN in the shader and render white).
    let has_normals = reader.read_normals().is_some();
    if let Some(iter) = reader.read_normals() {
        for normal in iter {
            mesh.normals.push(normal[0]);
            mesh.normals.push(normal[1]);
            mesh.normals.push(normal[2]);
        }
    }

    // Read UVs
    if let Some(iter) = reader.read_tex_coords(0) {
        let mut uvs = Vec::new();
        for uv in iter.into_f32() {
            uvs.push(uv[0]);
            uvs.push(uv[1]);
        }
        mesh.uvs = Some(uvs);
    }

    // Read indices
    if let Some(iter) = reader.read_indices() {
        for index in iter.into_u32() {
            mesh.indices.push(index);
        }
    } else {
        // Generate sequential indices
        for i in 0..mesh.vertex_count() as u32 {
            mesh.indices.push(i);
        }
    }

    if !has_normals {
        compute_smooth_normals(&mut mesh);
    }

    if options.flip_normals {
        mesh.flip_normals();
    }

    Ok(mesh)
}

/// Fill in per-vertex normals from the triangle geometry (area-weighted sum of
/// adjacent face normals), for glTF primitives that ship without a NORMAL
/// attribute. Zero normals would otherwise NaN out the shader (white surface).
fn compute_smooth_normals(mesh: &mut Mesh) {
    let vc = mesh.vertex_count();
    let mut normals = vec![0.0f32; vc * 3];
    let pos = &mesh.positions;
    let idx = &mesh.indices;
    let mut t = 0;
    while t + 2 < idx.len() {
        let (a, b, c) = (idx[t] as usize, idx[t + 1] as usize, idx[t + 2] as usize);
        let p = |i: usize| [pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]];
        let (pa, pb, pc) = (p(a), p(b), p(c));
        let u = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let v = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
        // Cross product (not normalized → area-weighted).
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for vi in [a, b, c] {
            normals[vi * 3] += n[0];
            normals[vi * 3 + 1] += n[1];
            normals[vi * 3 + 2] += n[2];
        }
        t += 3;
    }
    for k in 0..vc {
        let (x, y, z) = (normals[k * 3], normals[k * 3 + 1], normals[k * 3 + 2]);
        let len = (x * x + y * y + z * z).sqrt();
        if len > 1e-9 {
            normals[k * 3] = x / len;
            normals[k * 3 + 1] = y / len;
            normals[k * 3 + 2] = z / len;
        } else {
            normals[k * 3 + 1] = 1.0; // degenerate → point up
        }
    }
    mesh.normals = normals;
}

fn import_node(node: &gltf::Node<'_>, nodes: &mut Vec<ImportedNode>) {
    let transform = node.transform().matrix();

    let mesh_indices: Vec<usize> = node
        .mesh()
        .map(|m| vec![m.index()])
        .unwrap_or_default();

    let node_index = nodes.len();

    let mut imported_node = ImportedNode {
        name: node.name().unwrap_or("Node").to_string(),
        mesh_indices,
        transform,
        children: Vec::new(),
    };

    // First add this node to get the correct index
    nodes.push(imported_node.clone());

    // Then process children
    for child in node.children() {
        let child_index = nodes.len();
        import_node(&child, nodes);
        nodes[node_index].children.push(child_index);
    }
}

/// Export to glTF format
pub fn export<W: Write>(
    writer: &mut W,
    meshes: &[(&Mesh, Option<&str>)],
    options: &ExportOptions,
) -> Result<()> {
    // For simplicity, export as a single glTF JSON with embedded base64 buffers
    // A full implementation would support GLB binary format

    let mut json = String::new();
    json.push_str("{\n");
    json.push_str("  \"asset\": {\n");
    json.push_str("    \"version\": \"2.0\",\n");
    json.push_str("    \"generator\": \"rCAD\"\n");
    json.push_str("  },\n");

    // Build buffer data
    let mut buffer_data = Vec::new();
    let mut accessors = Vec::new();
    let mut buffer_views = Vec::new();

    for (mesh_idx, (mesh, _)) in meshes.iter().enumerate() {
        let pos_offset = buffer_data.len();
        let pos_count = mesh.vertex_count();

        // Write positions
        for i in 0..pos_count {
            let idx = i * 3;
            buffer_data.extend_from_slice(&mesh.positions[idx].to_le_bytes());
            buffer_data.extend_from_slice(&mesh.positions[idx + 1].to_le_bytes());
            buffer_data.extend_from_slice(&mesh.positions[idx + 2].to_le_bytes());
        }

        let pos_size = buffer_data.len() - pos_offset;

        buffer_views.push(format!(
            "    {{ \"buffer\": 0, \"byteOffset\": {}, \"byteLength\": {} }}",
            pos_offset, pos_size
        ));

        // Compute bounds
        let (min, max) = compute_bounds(mesh);
        accessors.push(format!(
            "    {{ \"bufferView\": {}, \"componentType\": 5126, \"count\": {}, \"type\": \"VEC3\", \"min\": [{}, {}, {}], \"max\": [{}, {}, {}] }}",
            buffer_views.len() - 1, pos_count,
            min[0], min[1], min[2], max[0], max[1], max[2]
        ));

        let pos_accessor = accessors.len() - 1;

        // Write normals
        let norm_offset = buffer_data.len();
        for i in 0..pos_count {
            let idx = i * 3;
            buffer_data.extend_from_slice(&mesh.normals[idx].to_le_bytes());
            buffer_data.extend_from_slice(&mesh.normals[idx + 1].to_le_bytes());
            buffer_data.extend_from_slice(&mesh.normals[idx + 2].to_le_bytes());
        }

        let norm_size = buffer_data.len() - norm_offset;

        buffer_views.push(format!(
            "    {{ \"buffer\": 0, \"byteOffset\": {}, \"byteLength\": {} }}",
            norm_offset, norm_size
        ));

        accessors.push(format!(
            "    {{ \"bufferView\": {}, \"componentType\": 5126, \"count\": {}, \"type\": \"VEC3\" }}",
            buffer_views.len() - 1, pos_count
        ));

        let norm_accessor = accessors.len() - 1;

        // Write indices
        let idx_offset = buffer_data.len();
        for idx in &mesh.indices {
            buffer_data.extend_from_slice(&idx.to_le_bytes());
        }

        let idx_size = buffer_data.len() - idx_offset;

        buffer_views.push(format!(
            "    {{ \"buffer\": 0, \"byteOffset\": {}, \"byteLength\": {} }}",
            idx_offset, idx_size
        ));

        accessors.push(format!(
            "    {{ \"bufferView\": {}, \"componentType\": 5125, \"count\": {}, \"type\": \"SCALAR\" }}",
            buffer_views.len() - 1, mesh.indices.len()
        ));
    }

    // Encode buffer as base64
    let buffer_base64 = base64_encode(&buffer_data);

    // Write JSON
    json.push_str("  \"buffers\": [\n");
    json.push_str(&format!(
        "    {{ \"uri\": \"data:application/octet-stream;base64,{}\", \"byteLength\": {} }}\n",
        buffer_base64,
        buffer_data.len()
    ));
    json.push_str("  ],\n");

    json.push_str("  \"bufferViews\": [\n");
    json.push_str(&buffer_views.join(",\n"));
    json.push_str("\n  ],\n");

    json.push_str("  \"accessors\": [\n");
    json.push_str(&accessors.join(",\n"));
    json.push_str("\n  ],\n");

    // Write meshes
    json.push_str("  \"meshes\": [\n");
    let mesh_jsons: Vec<String> = meshes
        .iter()
        .enumerate()
        .map(|(i, (_, name))| {
            let pos_acc = i * 3;
            let norm_acc = i * 3 + 1;
            let idx_acc = i * 3 + 2;
            let mesh_name = name.unwrap_or("mesh");
            format!(
                "    {{ \"name\": \"{}\", \"primitives\": [{{ \"attributes\": {{ \"POSITION\": {}, \"NORMAL\": {} }}, \"indices\": {} }}] }}",
                mesh_name, pos_acc, norm_acc, idx_acc
            )
        })
        .collect();
    json.push_str(&mesh_jsons.join(",\n"));
    json.push_str("\n  ],\n");

    // Write nodes and scene
    json.push_str("  \"nodes\": [\n");
    let node_jsons: Vec<String> = (0..meshes.len())
        .map(|i| format!("    {{ \"mesh\": {} }}", i))
        .collect();
    json.push_str(&node_jsons.join(",\n"));
    json.push_str("\n  ],\n");

    json.push_str("  \"scenes\": [\n");
    let scene_nodes: Vec<String> = (0..meshes.len()).map(|i| i.to_string()).collect();
    json.push_str(&format!(
        "    {{ \"nodes\": [{}] }}\n",
        scene_nodes.join(", ")
    ));
    json.push_str("  ],\n");
    json.push_str("  \"scene\": 0\n");
    json.push_str("}\n");

    writer.write_all(json.as_bytes())?;
    Ok(())
}

fn compute_bounds(mesh: &Mesh) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];

    for i in 0..mesh.vertex_count() {
        let idx = i * 3;
        for j in 0..3 {
            min[j] = min[j].min(mesh.positions[idx + j]);
            max[j] = max[j].max(mesh.positions[idx + j]);
        }
    }

    (min, max)
}

fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = String::new();

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;

        let triple = (b0 << 16) | (b1 << 8) | b2;

        output.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        output.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            output.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }

        if chunk.len() > 2 {
            output.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use crate::ExportOptions;

    #[test]
    #[ignore = "writes /tmp/box.gltf fixture for manual endpoint testing"]
    fn dump_box_gltf() {
        let solid = rcad_geometry::primitives::create_box(30.0, 30.0, 30.0).unwrap();
        let mesh = rcad_geometry::tessellation::tessellate_default(&solid).unwrap();
        let mut f = std::fs::File::create("/tmp/box.gltf").unwrap();
        super::export(&mut f, &[(&mesh, Some("Box"))], &ExportOptions::default()).unwrap();
    }
}
