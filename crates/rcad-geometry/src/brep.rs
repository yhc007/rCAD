//! B-Rep (Boundary Representation) operations using truck
//!
//! This module provides a wrapper around truck's B-Rep functionality.

use crate::{BoundingBox, Point3, Transform, Vector3};
use truck_modeling::*;

// Re-export the truck types used
pub use truck_modeling::cgmath::Point3 as CgPoint3;
pub use truck_modeling::cgmath::Vector3 as CgVector3;

/// Type aliases for truck types
pub type TruckPoint = cgmath::Point3<f64>;
pub type TruckVector = cgmath::Vector3<f64>;

/// A solid body represented as B-Rep
#[derive(Debug, Clone)]
pub struct Solid {
    /// The underlying truck solid (truck_modeling::Solid is already a type alias)
    pub inner: truck_modeling::Solid,
}

impl Solid {
    /// Create a new solid from a truck solid
    pub fn new(inner: truck_modeling::Solid) -> Self {
        Self { inner }
    }

    /// Get the number of faces
    pub fn face_count(&self) -> usize {
        self.inner.face_iter().count()
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> usize {
        self.inner.edge_iter().count()
    }

    /// Get the number of vertices
    pub fn vertex_count(&self) -> usize {
        self.inner.vertex_iter().count()
    }

    /// Check if the solid is valid
    pub fn is_valid(&self) -> bool {
        // Basic validity check
        self.face_count() > 0
    }

    /// Get the bounding box
    pub fn bounding_box(&self) -> Option<BoundingBox> {
        let mut bbox = BoundingBox::default();
        let mut has_points = false;

        for vertex in self.inner.vertex_iter() {
            let point = vertex.point();
            bbox.expand_to_include(glam::DVec3::new(point.x, point.y, point.z));
            has_points = true;
        }

        if has_points {
            Some(bbox)
        } else {
            None
        }
    }

    /// Transform the solid using a glam matrix
    pub fn transform(&mut self, matrix: &Transform) {
        // Convert glam DMat4 to cgmath Matrix4
        let cols = matrix.to_cols_array_2d();
        let cg_matrix = cgmath::Matrix4::new(
            cols[0][0], cols[0][1], cols[0][2], cols[0][3],
            cols[1][0], cols[1][1], cols[1][2], cols[1][3],
            cols[2][0], cols[2][1], cols[2][2], cols[2][3],
            cols[3][0], cols[3][1], cols[3][2], cols[3][3],
        );

        // Use builder::transformed for transformation
        self.inner = builder::transformed(&self.inner, cg_matrix);
    }

    /// Translate the solid
    pub fn translate(&mut self, offset: Vector3) {
        let matrix = Transform::from_translation(offset);
        self.transform(&matrix);
    }

    /// Rotate the solid around an axis
    pub fn rotate(&mut self, axis: Vector3, angle: f64) {
        let quat = glam::DQuat::from_axis_angle(axis.normalize(), angle);
        let matrix = Transform::from_quat(quat);
        self.transform(&matrix);
    }

    /// Scale the solid uniformly
    pub fn scale(&mut self, factor: f64) {
        let matrix = Transform::from_scale(glam::DVec3::splat(factor));
        self.transform(&matrix);
    }

    /// Scale the solid non-uniformly
    pub fn scale_xyz(&mut self, x: f64, y: f64, z: f64) {
        let matrix = Transform::from_scale(glam::DVec3::new(x, y, z));
        self.transform(&matrix);
    }

    /// Get face IDs (indices)
    pub fn face_ids(&self) -> Vec<usize> {
        (0..self.face_count()).collect()
    }

    /// Get edge IDs (indices)
    pub fn edge_ids(&self) -> Vec<usize> {
        (0..self.edge_count()).collect()
    }
}

/// A shell (collection of faces)
#[derive(Debug, Clone)]
pub struct RcadShell {
    pub inner: truck_modeling::Shell,
}

impl RcadShell {
    pub fn new(inner: truck_modeling::Shell) -> Self {
        Self { inner }
    }

    pub fn face_count(&self) -> usize {
        self.inner.face_iter().count()
    }
}

/// A wire (collection of edges forming a loop)
#[derive(Debug, Clone)]
pub struct RcadWire {
    pub inner: truck_modeling::Wire,
}

impl RcadWire {
    pub fn new(inner: truck_modeling::Wire) -> Self {
        Self { inner }
    }

    pub fn edge_count(&self) -> usize {
        self.inner.edge_iter().count()
    }

    /// Check if the wire is closed
    pub fn is_closed(&self) -> bool {
        let edges: Vec<_> = self.inner.edge_iter().collect();
        if edges.is_empty() {
            return false;
        }

        let first = edges.first().unwrap().front().point();
        let last = edges.last().unwrap().back().point();

        let diff = first - last;
        diff.x.abs() < crate::TOLERANCE
            && diff.y.abs() < crate::TOLERANCE
            && diff.z.abs() < crate::TOLERANCE
    }
}

/// Edge representation
#[derive(Debug, Clone)]
pub struct RcadEdge {
    pub inner: truck_modeling::Edge,
}

impl RcadEdge {
    pub fn new(inner: truck_modeling::Edge) -> Self {
        Self { inner }
    }

    /// Get the start point
    pub fn start(&self) -> Point3 {
        let p = self.inner.front().point();
        glam::DVec3::new(p.x, p.y, p.z)
    }

    /// Get the end point
    pub fn end(&self) -> Point3 {
        let p = self.inner.back().point();
        glam::DVec3::new(p.x, p.y, p.z)
    }

    /// Get the length of the edge
    pub fn length(&self) -> f64 {
        (self.end() - self.start()).length()
    }
}

/// Vertex representation
#[derive(Debug, Clone)]
pub struct RcadVertex {
    pub inner: truck_modeling::Vertex,
}

impl RcadVertex {
    pub fn new(inner: truck_modeling::Vertex) -> Self {
        Self { inner }
    }

    /// Get the point coordinates
    pub fn point(&self) -> Point3 {
        let p = self.inner.point();
        glam::DVec3::new(p.x, p.y, p.z)
    }
}

/// Helper to convert glam point to truck point
pub fn to_truck_point(p: Point3) -> TruckPoint {
    TruckPoint::new(p.x, p.y, p.z)
}

/// Helper to convert truck point to glam point
pub fn from_truck_point(p: TruckPoint) -> Point3 {
    glam::DVec3::new(p.x, p.y, p.z)
}

/// Helper to convert glam vector to truck vector
pub fn to_truck_vector(v: Vector3) -> TruckVector {
    TruckVector::new(v.x, v.y, v.z)
}

/// Helper to convert truck vector to glam vector
pub fn from_truck_vector(v: TruckVector) -> Vector3 {
    glam::DVec3::new(v.x, v.y, v.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn test_solid_bounding_box() {
        let solid = primitives::create_box(10.0, 20.0, 30.0).unwrap();
        let bbox = solid.bounding_box().unwrap();

        assert!((bbox.size().x - 10.0).abs() < 0.01);
        assert!((bbox.size().y - 20.0).abs() < 0.01);
        assert!((bbox.size().z - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_solid_transform() {
        let mut solid = primitives::create_box(10.0, 10.0, 10.0).unwrap();
        solid.translate(glam::DVec3::new(5.0, 0.0, 0.0));

        let bbox = solid.bounding_box().unwrap();
        let center = bbox.center();

        assert!((center.x - 5.0).abs() < 0.01);
    }
}
