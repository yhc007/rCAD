//! # rcad-geometry
//!
//! Geometry kernel for rCAD using truck B-Rep library.
//! Provides primitives, boolean operations, sketching, and tessellation.

pub mod boolean;
pub mod brep;
pub mod fillet;
pub mod primitives;
pub mod sketch;
pub mod tessellation;

pub use boolean::*;
pub use brep::*;
pub use fillet::*;
pub use primitives::*;
pub use sketch::*;
pub use tessellation::*;

use glam::{DVec3, DMat4};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type for geometry operations
pub type Result<T> = std::result::Result<T, GeometryError>;

/// Geometry errors
#[derive(Debug, Error)]
pub enum GeometryError {
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Boolean operation failed: {0}")]
    BooleanFailed(String),

    #[error("Tessellation failed: {0}")]
    TessellationFailed(String),

    #[error("Fillet/chamfer failed: {0}")]
    FilletFailed(String),

    #[error("Sketch error: {0}")]
    SketchError(String),

    #[error("Empty geometry")]
    EmptyGeometry,

    #[error("Topology error: {0}")]
    TopologyError(String),
}

/// 3D point
pub type Point3 = DVec3;

/// 3D vector
pub type Vector3 = DVec3;

/// 4x4 transformation matrix
pub type Transform = DMat4;

/// Tolerance for geometric operations
/// Note: truck-shapeops requires tolerance >= 1e-6
pub const TOLERANCE: f64 = 1e-6;

/// Angle tolerance (radians)
pub const ANGLE_TOLERANCE: f64 = 1e-9;

/// Mesh data for rendering
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    /// Vertex positions (x, y, z)
    pub positions: Vec<f32>,

    /// Vertex normals (nx, ny, nz)
    pub normals: Vec<f32>,

    /// Triangle indices
    pub indices: Vec<u32>,

    /// UV coordinates (optional)
    pub uvs: Option<Vec<f32>>,

    /// Vertex colors (optional, RGBA)
    pub colors: Option<Vec<f32>>,
}

impl Mesh {
    /// Create a new empty mesh
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of vertices
    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    /// Get the number of triangles
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Check if the mesh is empty
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Merge another mesh into this one
    pub fn merge(&mut self, other: &Mesh) {
        let vertex_offset = self.vertex_count() as u32;

        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);

        for index in &other.indices {
            self.indices.push(index + vertex_offset);
        }

        if let (Some(ref mut self_uvs), Some(ref other_uvs)) = (&mut self.uvs, &other.uvs) {
            self_uvs.extend_from_slice(other_uvs);
        }

        if let (Some(ref mut self_colors), Some(ref other_colors)) = (&mut self.colors, &other.colors) {
            self_colors.extend_from_slice(other_colors);
        }
    }

    /// Apply a transformation to all vertices
    pub fn transform(&mut self, transform: &Transform) {
        let normal_matrix = transform.inverse().transpose();

        for i in 0..self.vertex_count() {
            let idx = i * 3;

            // Transform position
            let pos = DVec3::new(
                self.positions[idx] as f64,
                self.positions[idx + 1] as f64,
                self.positions[idx + 2] as f64,
            );
            let transformed = transform.transform_point3(pos);
            self.positions[idx] = transformed.x as f32;
            self.positions[idx + 1] = transformed.y as f32;
            self.positions[idx + 2] = transformed.z as f32;

            // Transform normal
            let normal = DVec3::new(
                self.normals[idx] as f64,
                self.normals[idx + 1] as f64,
                self.normals[idx + 2] as f64,
            );
            let transformed_normal = normal_matrix.transform_vector3(normal).normalize();
            self.normals[idx] = transformed_normal.x as f32;
            self.normals[idx + 1] = transformed_normal.y as f32;
            self.normals[idx + 2] = transformed_normal.z as f32;
        }
    }

    /// Compute bounding box
    pub fn bounding_box(&self) -> Option<BoundingBox> {
        if self.is_empty() {
            return None;
        }

        let mut min = DVec3::splat(f64::MAX);
        let mut max = DVec3::splat(f64::MIN);

        for i in 0..self.vertex_count() {
            let idx = i * 3;
            let x = self.positions[idx] as f64;
            let y = self.positions[idx + 1] as f64;
            let z = self.positions[idx + 2] as f64;

            min.x = min.x.min(x);
            min.y = min.y.min(y);
            min.z = min.z.min(z);
            max.x = max.x.max(x);
            max.y = max.y.max(y);
            max.z = max.z.max(z);
        }

        Some(BoundingBox { min, max })
    }

    /// Flip all normals
    pub fn flip_normals(&mut self) {
        for normal in &mut self.normals {
            *normal = -*normal;
        }

        // Also flip triangle winding
        for i in 0..self.triangle_count() {
            let idx = i * 3;
            self.indices.swap(idx + 1, idx + 2);
        }
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: DVec3,
    pub max: DVec3,
}

impl BoundingBox {
    /// Create a new bounding box
    pub fn new(min: DVec3, max: DVec3) -> Self {
        Self { min, max }
    }

    /// Get the center of the bounding box
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }

    /// Get the size (dimensions) of the bounding box
    pub fn size(&self) -> DVec3 {
        self.max - self.min
    }

    /// Get the diagonal length
    pub fn diagonal(&self) -> f64 {
        self.size().length()
    }

    /// Check if a point is inside the bounding box
    pub fn contains(&self, point: DVec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Expand the bounding box to include a point
    pub fn expand_to_include(&mut self, point: DVec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    /// Merge with another bounding box
    pub fn merge(&mut self, other: &BoundingBox) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    /// Check if two bounding boxes intersect
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            min: DVec3::splat(f64::MAX),
            max: DVec3::splat(f64::MIN),
        }
    }
}
