//! glTF 2.0 file format support

use crate::{
    ExportOptions, ImportOptions, ImportedMaterial, ImportedMesh, ImportedModel, ImportedNode,
    IoError, Result,
};
use rcad_geometry::Mesh;
use std::io::{Read, Write};

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

        let imported_material = ImportedMaterial {
            name: material.name().unwrap_or("Unnamed").to_string(),
            base_color: pbr.base_color_factor(),
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

    // Read normals
    if let Some(iter) = reader.read_normals() {
        for normal in iter {
            mesh.normals.push(normal[0]);
            mesh.normals.push(normal[1]);
            mesh.normals.push(normal[2]);
        }
    } else {
        // Generate default normals
        let vertex_count = mesh.vertex_count();
        mesh.normals = vec![0.0; vertex_count * 3];
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

    if options.flip_normals {
        mesh.flip_normals();
    }

    Ok(mesh)
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
