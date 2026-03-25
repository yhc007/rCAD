//! 2D sketching module
//!
//! Provides 2D sketch creation and manipulation for extrusion/revolution operations.

use crate::{GeometryError, Point3, Result, Vector3};
use crate::brep::{TruckPoint, RcadWire};
use glam::{DVec2, DVec3, DMat4};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use truck_modeling::*;

/// A 2D sketch on a plane
#[derive(Debug, Clone)]
pub struct Sketch {
    /// Sketch plane
    pub plane: SketchPlane,

    /// Sketch entities
    pub entities: Vec<SketchEntity>,

    /// Sketch constraints (for solver)
    pub constraints: Vec<SketchConstraint>,
}

impl Sketch {
    /// Create a new sketch on the XY plane
    pub fn new_xy() -> Self {
        Self {
            plane: SketchPlane::xy(),
            entities: Vec::new(),
            constraints: Vec::new(),
        }
    }

    /// Create a new sketch on the XZ plane
    pub fn new_xz() -> Self {
        Self {
            plane: SketchPlane::xz(),
            entities: Vec::new(),
            constraints: Vec::new(),
        }
    }

    /// Create a new sketch on the YZ plane
    pub fn new_yz() -> Self {
        Self {
            plane: SketchPlane::yz(),
            entities: Vec::new(),
            constraints: Vec::new(),
        }
    }

    /// Create a sketch on a custom plane
    pub fn new_custom(origin: Point3, normal: Vector3, x_direction: Vector3) -> Self {
        Self {
            plane: SketchPlane::custom(origin, normal, x_direction),
            entities: Vec::new(),
            constraints: Vec::new(),
        }
    }

    /// Add a line to the sketch
    pub fn add_line(&mut self, start: DVec2, end: DVec2) -> usize {
        let id = self.entities.len();
        self.entities.push(SketchEntity::Line { start, end });
        id
    }

    /// Add a circle to the sketch
    pub fn add_circle(&mut self, center: DVec2, radius: f64) -> usize {
        let id = self.entities.len();
        self.entities.push(SketchEntity::Circle { center, radius });
        id
    }

    /// Add an arc to the sketch
    pub fn add_arc(&mut self, center: DVec2, radius: f64, start_angle: f64, end_angle: f64) -> usize {
        let id = self.entities.len();
        self.entities.push(SketchEntity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        });
        id
    }

    /// Add a point to the sketch
    pub fn add_point(&mut self, position: DVec2) -> usize {
        let id = self.entities.len();
        self.entities.push(SketchEntity::Point { position });
        id
    }

    /// Add a rectangle to the sketch
    pub fn add_rectangle(&mut self, corner1: DVec2, corner2: DVec2) -> Vec<usize> {
        let mut ids = Vec::new();

        let c1 = corner1;
        let c2 = DVec2::new(corner2.x, corner1.y);
        let c3 = corner2;
        let c4 = DVec2::new(corner1.x, corner2.y);

        ids.push(self.add_line(c1, c2));
        ids.push(self.add_line(c2, c3));
        ids.push(self.add_line(c3, c4));
        ids.push(self.add_line(c4, c1));

        ids
    }

    /// Add a polygon to the sketch
    pub fn add_polygon(&mut self, vertices: &[DVec2]) -> Vec<usize> {
        if vertices.len() < 3 {
            return Vec::new();
        }

        let mut ids = Vec::new();
        for i in 0..vertices.len() {
            let next = (i + 1) % vertices.len();
            ids.push(self.add_line(vertices[i], vertices[next]));
        }
        ids
    }

    /// Add a spline to the sketch
    pub fn add_spline(&mut self, control_points: Vec<DVec2>) -> usize {
        let id = self.entities.len();
        self.entities.push(SketchEntity::Spline { control_points });
        id
    }

    /// Add a constraint
    pub fn add_constraint(&mut self, constraint: SketchConstraint) {
        self.constraints.push(constraint);
    }

    /// Convert the sketch to a wire in 3D space
    pub fn to_wire(&self) -> Result<RcadWire> {
        if self.entities.is_empty() {
            return Err(GeometryError::SketchError("Empty sketch".to_string()));
        }

        let mut edges = Vec::new();

        for entity in &self.entities {
            match entity {
                SketchEntity::Line { start, end } => {
                    let p1 = self.plane.to_3d(*start);
                    let p2 = self.plane.to_3d(*end);

                    let v1 = builder::vertex(TruckPoint::new(p1.x, p1.y, p1.z));
                    let v2 = builder::vertex(TruckPoint::new(p2.x, p2.y, p2.z));

                    edges.push(builder::line(&v1, &v2));
                }

                SketchEntity::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                } => {
                    let center_3d = self.plane.to_3d(*center);

                    let start_2d = DVec2::new(
                        center.x + radius * start_angle.cos(),
                        center.y + radius * start_angle.sin(),
                    );
                    let end_2d = DVec2::new(
                        center.x + radius * end_angle.cos(),
                        center.y + radius * end_angle.sin(),
                    );

                    let p1 = self.plane.to_3d(start_2d);
                    let p2 = self.plane.to_3d(end_2d);

                    let v1 = builder::vertex(TruckPoint::new(p1.x, p1.y, p1.z));
                    let v2 = builder::vertex(TruckPoint::new(p2.x, p2.y, p2.z));

                    edges.push(builder::circle_arc(
                        &v1,
                        &v2,
                        TruckPoint::new(center_3d.x, center_3d.y, center_3d.z),
                    ));
                }

                SketchEntity::Circle { center, radius } => {
                    // Create circle as 4 arcs
                    let center_3d = self.plane.to_3d(*center);
                    let angles = [0.0, PI / 2.0, PI, 3.0 * PI / 2.0, 2.0 * PI];

                    for i in 0..4 {
                        let start_2d = DVec2::new(
                            center.x + radius * angles[i].cos(),
                            center.y + radius * angles[i].sin(),
                        );
                        let end_2d = DVec2::new(
                            center.x + radius * angles[i + 1].cos(),
                            center.y + radius * angles[i + 1].sin(),
                        );

                        let p1 = self.plane.to_3d(start_2d);
                        let p2 = self.plane.to_3d(end_2d);

                        let v1 = builder::vertex(TruckPoint::new(p1.x, p1.y, p1.z));
                        let v2 = builder::vertex(TruckPoint::new(p2.x, p2.y, p2.z));

                        edges.push(builder::circle_arc(
                            &v1,
                            &v2,
                            TruckPoint::new(center_3d.x, center_3d.y, center_3d.z),
                        ));
                    }
                }

                SketchEntity::Point { .. } => {
                    // Points don't create edges
                }

                SketchEntity::Spline { control_points } => {
                    // Approximate spline as line segments for now
                    if control_points.len() >= 2 {
                        for i in 0..(control_points.len() - 1) {
                            let p1 = self.plane.to_3d(control_points[i]);
                            let p2 = self.plane.to_3d(control_points[i + 1]);

                            let v1 = builder::vertex(TruckPoint::new(p1.x, p1.y, p1.z));
                            let v2 = builder::vertex(TruckPoint::new(p2.x, p2.y, p2.z));

                            edges.push(builder::line(&v1, &v2));
                        }
                    }
                }
            }
        }

        if edges.is_empty() {
            return Err(GeometryError::SketchError(
                "No edges created from sketch".to_string(),
            ));
        }

        Ok(RcadWire::new(truck_modeling::Wire::from_iter(edges)))
    }

    /// Check if the sketch forms a closed loop
    pub fn is_closed(&self) -> bool {
        self.to_wire().map(|w| w.is_closed()).unwrap_or(false)
    }

    /// Get bounding box of the sketch in 2D
    pub fn bounding_box_2d(&self) -> Option<(DVec2, DVec2)> {
        if self.entities.is_empty() {
            return None;
        }

        let mut min = DVec2::splat(f64::MAX);
        let mut max = DVec2::splat(f64::MIN);

        for entity in &self.entities {
            match entity {
                SketchEntity::Line { start, end } => {
                    min = min.min(*start).min(*end);
                    max = max.max(*start).max(*end);
                }
                SketchEntity::Circle { center, radius } => {
                    min = min.min(*center - DVec2::splat(*radius));
                    max = max.max(*center + DVec2::splat(*radius));
                }
                SketchEntity::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                } => {
                    // Simplified - just use center and radius
                    min = min.min(*center - DVec2::splat(*radius));
                    max = max.max(*center + DVec2::splat(*radius));
                }
                SketchEntity::Point { position } => {
                    min = min.min(*position);
                    max = max.max(*position);
                }
                SketchEntity::Spline { control_points } => {
                    for p in control_points {
                        min = min.min(*p);
                        max = max.max(*p);
                    }
                }
            }
        }

        Some((min, max))
    }
}

/// Sketch plane definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchPlane {
    /// Origin of the plane
    pub origin: Point3,

    /// Normal vector
    pub normal: Vector3,

    /// X direction in plane
    pub x_direction: Vector3,

    /// Y direction in plane (computed from normal and x)
    pub y_direction: Vector3,
}

impl SketchPlane {
    /// XY plane at Z=0
    pub fn xy() -> Self {
        Self {
            origin: DVec3::ZERO,
            normal: DVec3::Z,
            x_direction: DVec3::X,
            y_direction: DVec3::Y,
        }
    }

    /// XZ plane at Y=0
    pub fn xz() -> Self {
        Self {
            origin: DVec3::ZERO,
            normal: DVec3::Y,
            x_direction: DVec3::X,
            y_direction: DVec3::Z,
        }
    }

    /// YZ plane at X=0
    pub fn yz() -> Self {
        Self {
            origin: DVec3::ZERO,
            normal: DVec3::X,
            x_direction: DVec3::Y,
            y_direction: DVec3::Z,
        }
    }

    /// Custom plane
    pub fn custom(origin: Point3, normal: Vector3, x_direction: Vector3) -> Self {
        let normal = normal.normalize();
        let x_dir = x_direction.normalize();
        let y_dir = normal.cross(x_dir).normalize();

        Self {
            origin,
            normal,
            x_direction: x_dir,
            y_direction: y_dir,
        }
    }

    /// Offset XY plane
    pub fn xy_offset(z: f64) -> Self {
        Self {
            origin: DVec3::new(0.0, 0.0, z),
            normal: DVec3::Z,
            x_direction: DVec3::X,
            y_direction: DVec3::Y,
        }
    }

    /// Convert a 2D point to 3D on this plane
    pub fn to_3d(&self, point: DVec2) -> Point3 {
        self.origin + self.x_direction * point.x + self.y_direction * point.y
    }

    /// Convert a 3D point to 2D on this plane
    pub fn to_2d(&self, point: Point3) -> DVec2 {
        let offset = point - self.origin;
        DVec2::new(offset.dot(self.x_direction), offset.dot(self.y_direction))
    }

    /// Get the transformation matrix for this plane
    pub fn transform(&self) -> DMat4 {
        DMat4::from_cols(
            self.x_direction.extend(0.0),
            self.y_direction.extend(0.0),
            self.normal.extend(0.0),
            self.origin.extend(1.0),
        )
    }
}

/// Sketch entity types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SketchEntity {
    /// Line segment
    Line { start: DVec2, end: DVec2 },

    /// Full circle
    Circle { center: DVec2, radius: f64 },

    /// Arc (part of a circle)
    Arc {
        center: DVec2,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },

    /// Point (for construction)
    Point { position: DVec2 },

    /// Spline curve
    Spline { control_points: Vec<DVec2> },
}

/// Sketch constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SketchConstraint {
    /// Fix a point
    Fixed { entity: usize, point_index: usize },

    /// Horizontal line
    Horizontal { entity: usize },

    /// Vertical line
    Vertical { entity: usize },

    /// Coincident points
    Coincident {
        entity1: usize,
        point1: usize,
        entity2: usize,
        point2: usize,
    },

    /// Parallel lines
    Parallel { entity1: usize, entity2: usize },

    /// Perpendicular lines
    Perpendicular { entity1: usize, entity2: usize },

    /// Equal length/radius
    Equal { entity1: usize, entity2: usize },

    /// Distance dimension
    Distance {
        entity1: usize,
        point1: usize,
        entity2: Option<usize>,
        point2: Option<usize>,
        value: f64,
    },

    /// Angle dimension
    Angle {
        entity1: usize,
        entity2: usize,
        value: f64,
    },

    /// Radius dimension
    Radius { entity: usize, value: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sketch_plane_conversion() {
        let plane = SketchPlane::xy();
        let p2d = DVec2::new(10.0, 20.0);
        let p3d = plane.to_3d(p2d);

        assert!((p3d.x - 10.0).abs() < 1e-10);
        assert!((p3d.y - 20.0).abs() < 1e-10);
        assert!((p3d.z).abs() < 1e-10);

        let p2d_back = plane.to_2d(p3d);
        assert!((p2d_back.x - p2d.x).abs() < 1e-10);
        assert!((p2d_back.y - p2d.y).abs() < 1e-10);
    }

    #[test]
    fn test_sketch_rectangle() {
        let mut sketch = Sketch::new_xy();
        let ids = sketch.add_rectangle(DVec2::new(0.0, 0.0), DVec2::new(10.0, 20.0));

        assert_eq!(ids.len(), 4);
        assert_eq!(sketch.entities.len(), 4);
    }

    #[test]
    fn test_sketch_circle() {
        let mut sketch = Sketch::new_xy();
        sketch.add_circle(DVec2::new(0.0, 0.0), 10.0);

        assert_eq!(sketch.entities.len(), 1);
    }

    #[test]
    fn test_sketch_to_wire() {
        let mut sketch = Sketch::new_xy();
        sketch.add_rectangle(DVec2::new(-5.0, -5.0), DVec2::new(5.0, 5.0));

        let wire = sketch.to_wire();
        assert!(wire.is_ok());
    }
}
