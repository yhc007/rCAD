//! # rcad-io
//!
//! File format support for rCAD including STEP, IGES, STL, OBJ, glTF, and USD.

pub mod gltf;
pub mod obj;
pub mod stl;

#[cfg(feature = "step")]
pub mod step;

pub mod usd;

use rcad_geometry::Mesh;
use thiserror::Error;

/// Result type for I/O operations
pub type Result<T> = std::result::Result<T, IoError>;

/// I/O error types
#[derive(Debug, Error)]
pub enum IoError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Write error: {0}")]
    WriteError(String),

    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Supported file formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// STEP AP203/AP214/AP242
    Step,
    /// IGES
    Iges,
    /// STL (ASCII or Binary)
    Stl,
    /// Wavefront OBJ
    Obj,
    /// glTF 2.0
    Gltf,
    /// USD/USDA/USDC
    Usd,
}

impl FileFormat {
    /// Detect format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "step" | "stp" | "p21" => Some(Self::Step),
            "iges" | "igs" => Some(Self::Iges),
            "stl" => Some(Self::Stl),
            "obj" => Some(Self::Obj),
            "gltf" | "glb" => Some(Self::Gltf),
            "usd" | "usda" | "usdc" | "usdz" => Some(Self::Usd),
            _ => None,
        }
    }

    /// Get file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Iges => "iges",
            Self::Stl => "stl",
            Self::Obj => "obj",
            Self::Gltf => "gltf",
            Self::Usd => "usda",
        }
    }

    /// Get MIME type for this format
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Step => "application/step",
            Self::Iges => "application/iges",
            Self::Stl => "application/sla",
            Self::Obj => "text/plain",
            Self::Gltf => "model/gltf+json",
            Self::Usd => "model/vnd.usd+usda",
        }
    }

    /// Check if format supports B-Rep geometry
    pub fn supports_brep(&self) -> bool {
        matches!(self, Self::Step | Self::Iges)
    }

    /// Check if format is mesh-only
    pub fn is_mesh_format(&self) -> bool {
        matches!(self, Self::Stl | Self::Obj | Self::Gltf)
    }
}

/// Import options
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Scale factor to apply (default 1.0)
    pub scale: f64,

    /// Whether to merge identical vertices
    pub merge_vertices: bool,

    /// Tolerance for merging vertices
    pub merge_tolerance: f64,

    /// Whether to compute normals if missing
    pub compute_normals: bool,

    /// Whether to flip normals
    pub flip_normals: bool,
}

impl ImportOptions {
    pub fn new() -> Self {
        Self {
            scale: 1.0,
            merge_vertices: true,
            merge_tolerance: 1e-6,
            compute_normals: true,
            flip_normals: false,
        }
    }
}

/// Export options
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    /// Format-specific options as key-value pairs
    pub format_options: std::collections::HashMap<String, String>,

    /// Whether to use binary format (where applicable)
    pub binary: bool,

    /// Compression level (0-9, where applicable)
    pub compression: u8,

    /// Whether to include materials
    pub include_materials: bool,

    /// Whether to include textures
    pub include_textures: bool,
}

impl ExportOptions {
    pub fn new() -> Self {
        Self {
            format_options: std::collections::HashMap::new(),
            binary: true,
            compression: 6,
            include_materials: true,
            include_textures: true,
        }
    }

    pub fn ascii() -> Self {
        Self {
            binary: false,
            ..Self::new()
        }
    }
}

/// Imported model data
#[derive(Debug, Clone)]
pub struct ImportedModel {
    /// Meshes
    pub meshes: Vec<ImportedMesh>,

    /// Materials (if any)
    pub materials: Vec<ImportedMaterial>,

    /// Scene hierarchy (mesh indices and transforms)
    pub nodes: Vec<ImportedNode>,
}

impl Default for ImportedModel {
    fn default() -> Self {
        Self {
            meshes: Vec::new(),
            materials: Vec::new(),
            nodes: Vec::new(),
        }
    }
}

/// Imported mesh data
#[derive(Debug, Clone)]
pub struct ImportedMesh {
    /// Mesh name
    pub name: String,

    /// Geometry data
    pub mesh: Mesh,

    /// Material index (if any)
    pub material_index: Option<usize>,
}

/// Imported material
#[derive(Debug, Clone)]
pub struct ImportedMaterial {
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

impl Default for ImportedMaterial {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0, 0.0, 0.0],
        }
    }
}

/// Scene node with transform
#[derive(Debug, Clone)]
pub struct ImportedNode {
    /// Node name
    pub name: String,

    /// Mesh indices attached to this node
    pub mesh_indices: Vec<usize>,

    /// Local transform matrix (column-major)
    pub transform: [[f32; 4]; 4],

    /// Child node indices
    pub children: Vec<usize>,
}

impl Default for ImportedNode {
    fn default() -> Self {
        Self {
            name: String::new(),
            mesh_indices: Vec::new(),
            transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            children: Vec::new(),
        }
    }
}
