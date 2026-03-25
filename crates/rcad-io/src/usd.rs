//! USD (Universal Scene Description) file format support
//!
//! Exports geometry to USDA (ASCII) format for Omniverse compatibility.

use crate::{ExportOptions, IoError, Result};
use rcad_geometry::Mesh;
use std::io::Write;

/// Export to USDA (ASCII USD) format
pub fn export<W: Write>(
    writer: &mut W,
    meshes: &[(&Mesh, &str, Option<UsdMaterial>)],
    options: &ExportOptions,
) -> Result<()> {
    // Write header
    writeln!(writer, "#usda 1.0")?;
    writeln!(writer, "(")?;
    writeln!(writer, "    defaultPrim = \"Root\"")?;
    writeln!(writer, "    metersPerUnit = 0.001")?;
    writeln!(writer, "    upAxis = \"Y\"")?;
    writeln!(writer, ")")?;
    writeln!(writer)?;

    // Write root xform
    writeln!(writer, "def Xform \"Root\"")?;
    writeln!(writer, "{{")?;

    // Write meshes
    for (idx, (mesh, name, material)) in meshes.iter().enumerate() {
        let safe_name = sanitize_name(name);
        write_mesh(writer, mesh, &safe_name, idx, material.as_ref())?;
    }

    writeln!(writer, "}}")?;

    Ok(())
}

fn sanitize_name(name: &str) -> String {
    // USD prim names must start with a letter or underscore and contain only
    // letters, digits, and underscores
    let mut result = String::new();

    for (i, c) in name.chars().enumerate() {
        if i == 0 {
            if c.is_ascii_alphabetic() || c == '_' {
                result.push(c);
            } else {
                result.push('_');
                if c.is_ascii_alphanumeric() {
                    result.push(c);
                }
            }
        } else if c.is_ascii_alphanumeric() || c == '_' {
            result.push(c);
        } else {
            result.push('_');
        }
    }

    if result.is_empty() {
        result = "Mesh".to_string();
    }

    result
}

fn write_mesh<W: Write>(
    writer: &mut W,
    mesh: &Mesh,
    name: &str,
    index: usize,
    material: Option<&UsdMaterial>,
) -> Result<()> {
    let prim_name = if index == 0 {
        name.to_string()
    } else {
        format!("{}_{}", name, index)
    };

    writeln!(writer, "    def Mesh \"{}\"", prim_name)?;
    writeln!(writer, "    {{")?;

    // Write extent (bounding box)
    let (min, max) = compute_bounds(mesh);
    writeln!(
        writer,
        "        float3[] extent = [({}, {}, {}), ({}, {}, {})]",
        min[0], min[1], min[2], max[0], max[1], max[2]
    )?;

    // Write face vertex counts (all triangles = 3)
    let face_count = mesh.triangle_count();
    writeln!(writer, "        int[] faceVertexCounts = [")?;
    write!(writer, "            ")?;
    for i in 0..face_count {
        if i > 0 {
            write!(writer, ", ")?;
            if i % 20 == 0 {
                writeln!(writer)?;
                write!(writer, "            ")?;
            }
        }
        write!(writer, "3")?;
    }
    writeln!(writer)?;
    writeln!(writer, "        ]")?;

    // Write face vertex indices
    writeln!(writer, "        int[] faceVertexIndices = [")?;
    write!(writer, "            ")?;
    for (i, idx) in mesh.indices.iter().enumerate() {
        if i > 0 {
            write!(writer, ", ")?;
            if i % 20 == 0 {
                writeln!(writer)?;
                write!(writer, "            ")?;
            }
        }
        write!(writer, "{}", idx)?;
    }
    writeln!(writer)?;
    writeln!(writer, "        ]")?;

    // Write points (vertex positions)
    writeln!(writer, "        point3f[] points = [")?;
    for i in 0..mesh.vertex_count() {
        let idx = i * 3;
        if i > 0 {
            writeln!(writer, ",")?;
        }
        write!(
            writer,
            "            ({}, {}, {})",
            mesh.positions[idx],
            mesh.positions[idx + 1],
            mesh.positions[idx + 2]
        )?;
    }
    writeln!(writer)?;
    writeln!(writer, "        ]")?;

    // Write normals
    writeln!(writer, "        normal3f[] normals = [")?;
    for i in 0..mesh.vertex_count() {
        let idx = i * 3;
        if i > 0 {
            writeln!(writer, ",")?;
        }
        write!(
            writer,
            "            ({}, {}, {})",
            mesh.normals[idx],
            mesh.normals[idx + 1],
            mesh.normals[idx + 2]
        )?;
    }
    writeln!(writer)?;
    writeln!(writer, "        ] (")?;
    writeln!(writer, "            interpolation = \"vertex\"")?;
    writeln!(writer, "        )")?;

    // Write UVs if present
    if let Some(ref uvs) = mesh.uvs {
        writeln!(writer, "        texCoord2f[] primvars:st = [")?;
        let uv_count = uvs.len() / 2;
        for i in 0..uv_count {
            let idx = i * 2;
            if i > 0 {
                writeln!(writer, ",")?;
            }
            write!(writer, "            ({}, {})", uvs[idx], uvs[idx + 1])?;
        }
        writeln!(writer)?;
        writeln!(writer, "        ] (")?;
        writeln!(writer, "            interpolation = \"vertex\"")?;
        writeln!(writer, "        )")?;
    }

    // Write material binding if provided
    if let Some(mat) = material {
        write_material(writer, mat, &prim_name)?;
    }

    writeln!(writer, "    }}")?;

    Ok(())
}

fn write_material<W: Write>(writer: &mut W, material: &UsdMaterial, mesh_name: &str) -> Result<()> {
    let mat_name = format!("{}_Material", mesh_name);

    writeln!(writer)?;
    writeln!(writer, "        def Material \"{}\"", mat_name)?;
    writeln!(writer, "        {{")?;

    // USD Preview Surface shader
    writeln!(writer, "            token outputs:surface.connect = </{}/Root/{}/{}_Shader.outputs:surface>",
             "Root", mesh_name, mat_name)?;

    writeln!(writer)?;
    writeln!(
        writer,
        "            def Shader \"{}_Shader\"",
        mat_name
    )?;
    writeln!(writer, "            {{")?;
    writeln!(
        writer,
        "                uniform token info:id = \"UsdPreviewSurface\""
    )?;

    // Base color
    writeln!(
        writer,
        "                color3f inputs:diffuseColor = ({}, {}, {})",
        material.base_color[0], material.base_color[1], material.base_color[2]
    )?;

    // Metallic
    writeln!(
        writer,
        "                float inputs:metallic = {}",
        material.metallic
    )?;

    // Roughness
    writeln!(
        writer,
        "                float inputs:roughness = {}",
        material.roughness
    )?;

    // Opacity
    writeln!(
        writer,
        "                float inputs:opacity = {}",
        material.base_color[3]
    )?;

    writeln!(writer, "                token outputs:surface")?;
    writeln!(writer, "            }}")?;
    writeln!(writer, "        }}")?;

    // Bind material to mesh
    writeln!(writer)?;
    writeln!(
        writer,
        "        rel material:binding = </{}/Root/{}/{}>",
        "Root", mesh_name, mat_name
    )?;

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

/// USD material definition
#[derive(Debug, Clone)]
pub struct UsdMaterial {
    /// Material name
    pub name: String,

    /// Base color (RGBA)
    pub base_color: [f32; 4],

    /// Metallic factor
    pub metallic: f32,

    /// Roughness factor
    pub roughness: f32,

    /// Emissive color
    pub emissive: [f32; 3],
}

impl Default for UsdMaterial {
    fn default() -> Self {
        Self {
            name: "DefaultMaterial".to_string(),
            base_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0, 0.0, 0.0],
        }
    }
}

/// Export a full USD scene with hierarchy
pub fn export_scene<W: Write>(
    writer: &mut W,
    scene: &UsdScene,
    options: &ExportOptions,
) -> Result<()> {
    // Write header
    writeln!(writer, "#usda 1.0")?;
    writeln!(writer, "(")?;
    writeln!(
        writer,
        "    defaultPrim = \"{}\"",
        scene.root_name
    )?;
    writeln!(writer, "    metersPerUnit = {}", scene.meters_per_unit)?;
    writeln!(
        writer,
        "    upAxis = \"{}\"",
        match scene.up_axis {
            UpAxis::Y => "Y",
            UpAxis::Z => "Z",
        }
    )?;
    writeln!(writer, ")")?;
    writeln!(writer)?;

    // Write root and children
    write_node(writer, &scene.root, 0)?;

    Ok(())
}

fn write_node<W: Write>(writer: &mut W, node: &UsdNode, indent: usize) -> Result<()> {
    let indent_str = "    ".repeat(indent);

    writeln!(writer, "{}def Xform \"{}\"", indent_str, node.name)?;
    writeln!(writer, "{}{{", indent_str)?;

    // Write transform if not identity
    if node.transform != [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]] {
        writeln!(
            writer,
            "{}    matrix4d xformOp:transform = (({}, {}, {}, {}), ({}, {}, {}, {}), ({}, {}, {}, {}), ({}, {}, {}, {}))",
            indent_str,
            node.transform[0][0], node.transform[0][1], node.transform[0][2], node.transform[0][3],
            node.transform[1][0], node.transform[1][1], node.transform[1][2], node.transform[1][3],
            node.transform[2][0], node.transform[2][1], node.transform[2][2], node.transform[2][3],
            node.transform[3][0], node.transform[3][1], node.transform[3][2], node.transform[3][3],
        )?;
        writeln!(
            writer,
            "{}    uniform token[] xformOpOrder = [\"xformOp:transform\"]",
            indent_str
        )?;
    }

    // Write mesh if present
    if let Some(ref mesh_data) = node.mesh {
        write_mesh(writer, &mesh_data.mesh, &node.name, 0, mesh_data.material.as_ref())?;
    }

    // Write children
    for child in &node.children {
        write_node(writer, child, indent + 1)?;
    }

    writeln!(writer, "{}}}", indent_str)?;

    Ok(())
}

/// USD scene structure
#[derive(Debug, Clone)]
pub struct UsdScene {
    /// Root prim name
    pub root_name: String,

    /// Meters per unit
    pub meters_per_unit: f32,

    /// Up axis
    pub up_axis: UpAxis,

    /// Root node
    pub root: UsdNode,
}

impl Default for UsdScene {
    fn default() -> Self {
        Self {
            root_name: "Root".to_string(),
            meters_per_unit: 0.001, // millimeters
            up_axis: UpAxis::Y,
            root: UsdNode::default(),
        }
    }
}

/// Up axis for the scene
#[derive(Debug, Clone, Copy, Default)]
pub enum UpAxis {
    #[default]
    Y,
    Z,
}

/// USD scene node
#[derive(Debug, Clone, Default)]
pub struct UsdNode {
    /// Node name
    pub name: String,

    /// Local transform (column-major 4x4)
    pub transform: [[f32; 4]; 4],

    /// Mesh data (if any)
    pub mesh: Option<UsdMeshData>,

    /// Child nodes
    pub children: Vec<UsdNode>,
}

impl UsdNode {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            mesh: None,
            children: Vec::new(),
        }
    }
}

/// Mesh data for USD export
#[derive(Debug, Clone)]
pub struct UsdMeshData {
    /// The mesh geometry
    pub mesh: Mesh,

    /// Optional material
    pub material: Option<UsdMaterial>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("test"), "test");
        assert_eq!(sanitize_name("123test"), "_123test");
        assert_eq!(sanitize_name("test-name"), "test_name");
        assert_eq!(sanitize_name("test name"), "test_name");
    }

    #[test]
    fn test_export_simple_mesh() {
        let mut mesh = Mesh::new();
        mesh.positions = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.5, 1.0, 0.0];
        mesh.normals = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        mesh.indices = vec![0, 1, 2];

        let meshes = vec![(&mesh, "TestMesh", None)];

        let mut output = Vec::new();
        export(&mut output, &meshes, &ExportOptions::default()).unwrap();

        let content = String::from_utf8(output).unwrap();
        assert!(content.contains("#usda 1.0"));
        assert!(content.contains("def Mesh"));
        assert!(content.contains("point3f[] points"));
    }
}
