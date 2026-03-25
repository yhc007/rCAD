//! Parametric features for rCAD
//!
//! Features represent individual modeling operations that can be
//! parameterized, ordered, and re-evaluated.

use crate::{EntityId, FeatureId, Point3D, Transform};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A parametric feature in the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    /// Unique identifier
    pub id: FeatureId,

    /// Human-readable name
    pub name: String,

    /// Whether the feature is suppressed (excluded from computation)
    pub suppressed: bool,

    /// Feature-specific data
    pub data: FeatureData,

    /// Named parameters that can be linked to document parameters
    pub parameters: HashMap<String, FeatureParameter>,

    /// Transformation applied to this feature
    pub transform: Transform,

    /// Features this feature depends on
    pub depends_on: Vec<FeatureId>,
}

impl Feature {
    /// Create a new feature
    pub fn new(name: impl Into<String>, data: FeatureData) -> Self {
        Self {
            id: FeatureId::new(),
            name: name.into(),
            suppressed: false,
            data,
            parameters: HashMap::new(),
            transform: Transform::IDENTITY,
            depends_on: Vec::new(),
        }
    }

    /// Create a new feature with a specific ID
    pub fn with_id(id: FeatureId, name: impl Into<String>, data: FeatureData) -> Self {
        Self {
            id,
            name: name.into(),
            suppressed: false,
            data,
            parameters: HashMap::new(),
            transform: Transform::IDENTITY,
            depends_on: Vec::new(),
        }
    }

    /// Set a parameter value
    pub fn set_parameter(&mut self, name: impl Into<String>, param: FeatureParameter) {
        self.parameters.insert(name.into(), param);
    }

    /// Get a parameter value
    pub fn get_parameter(&self, name: &str) -> Option<&FeatureParameter> {
        self.parameters.get(name)
    }

    /// Add a dependency on another feature
    pub fn add_dependency(&mut self, feature_id: FeatureId) {
        if !self.depends_on.contains(&feature_id) {
            self.depends_on.push(feature_id);
        }
    }

    /// Get the feature type as a string
    pub fn feature_type(&self) -> &'static str {
        match &self.data {
            FeatureData::Primitive(p) => match p {
                PrimitiveFeature::Box { .. } => "Box",
                PrimitiveFeature::Cylinder { .. } => "Cylinder",
                PrimitiveFeature::Sphere { .. } => "Sphere",
                PrimitiveFeature::Cone { .. } => "Cone",
                PrimitiveFeature::Torus { .. } => "Torus",
            },
            FeatureData::Boolean(b) => match b {
                BooleanFeature::Union { .. } => "Union",
                BooleanFeature::Subtract { .. } => "Subtract",
                BooleanFeature::Intersect { .. } => "Intersect",
            },
            FeatureData::Sketch(_) => "Sketch",
            FeatureData::Extrude(_) => "Extrude",
            FeatureData::Revolve(_) => "Revolve",
            FeatureData::Sweep(_) => "Sweep",
            FeatureData::Loft(_) => "Loft",
            FeatureData::Fillet(_) => "Fillet",
            FeatureData::Chamfer(_) => "Chamfer",
            FeatureData::Shell(_) => "Shell",
            FeatureData::Pattern(_) => "Pattern",
            FeatureData::Mirror(_) => "Mirror",
            FeatureData::Import(_) => "Import",
        }
    }
}

/// Feature parameter that can be a direct value or linked to a document parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeatureParameter {
    /// Direct numeric value
    Value(f64),

    /// Link to a document parameter by name
    Link(String),

    /// Expression to evaluate
    Expression(String),
}

impl FeatureParameter {
    /// Get the resolved value (for now, just returns direct values)
    pub fn resolve(&self) -> Option<f64> {
        match self {
            FeatureParameter::Value(v) => Some(*v),
            FeatureParameter::Link(_) => None, // Would need document context
            FeatureParameter::Expression(_) => None, // Would need evaluation
        }
    }
}

/// Different types of features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeatureData {
    /// Primitive solid shapes
    Primitive(PrimitiveFeature),

    /// Boolean operations
    Boolean(BooleanFeature),

    /// 2D sketch
    Sketch(SketchFeature),

    /// Extrude a sketch
    Extrude(ExtrudeFeature),

    /// Revolve a sketch
    Revolve(RevolveFeature),

    /// Sweep a profile along a path
    Sweep(SweepFeature),

    /// Loft between profiles
    Loft(LoftFeature),

    /// Fillet edges
    Fillet(FilletFeature),

    /// Chamfer edges
    Chamfer(ChamferFeature),

    /// Shell (hollow out) a solid
    Shell(ShellFeature),

    /// Pattern (linear or circular)
    Pattern(PatternFeature),

    /// Mirror feature
    Mirror(MirrorFeature),

    /// Imported geometry
    Import(ImportFeature),
}

/// Primitive solid features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrimitiveFeature {
    /// Rectangular box
    Box {
        width: f64,
        height: f64,
        depth: f64,
    },

    /// Cylinder
    Cylinder {
        radius: f64,
        height: f64,
    },

    /// Sphere
    Sphere {
        radius: f64,
    },

    /// Cone
    Cone {
        bottom_radius: f64,
        top_radius: f64,
        height: f64,
    },

    /// Torus
    Torus {
        major_radius: f64,
        minor_radius: f64,
    },
}

/// Boolean operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BooleanFeature {
    /// Union (add) bodies
    Union {
        target: FeatureId,
        tools: Vec<FeatureId>,
    },

    /// Subtract (cut) bodies
    Subtract {
        target: FeatureId,
        tools: Vec<FeatureId>,
    },

    /// Intersect bodies
    Intersect {
        target: FeatureId,
        tools: Vec<FeatureId>,
    },
}

/// 2D sketch feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchFeature {
    /// Sketch plane definition
    pub plane: SketchPlane,

    /// Sketch entities (lines, arcs, circles, etc.)
    pub entities: Vec<SketchEntity>,

    /// Constraints applied to the sketch
    pub constraints: Vec<SketchConstraint>,
}

/// Sketch plane definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SketchPlane {
    /// XY plane at Z offset
    XY { z_offset: f64 },

    /// XZ plane at Y offset
    XZ { y_offset: f64 },

    /// YZ plane at X offset
    YZ { x_offset: f64 },

    /// Face of existing geometry
    Face { feature_id: FeatureId, face_id: EntityId },

    /// Custom plane
    Custom {
        origin: Point3D,
        normal: DVec3,
        x_axis: DVec3,
    },
}

/// Sketch geometric entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SketchEntity {
    /// Line segment
    Line {
        id: EntityId,
        start: [f64; 2],
        end: [f64; 2],
    },

    /// Arc (portion of a circle)
    Arc {
        id: EntityId,
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },

    /// Full circle
    Circle {
        id: EntityId,
        center: [f64; 2],
        radius: f64,
    },

    /// Ellipse
    Ellipse {
        id: EntityId,
        center: [f64; 2],
        major_radius: f64,
        minor_radius: f64,
        rotation: f64,
    },

    /// Spline curve
    Spline {
        id: EntityId,
        control_points: Vec<[f64; 2]>,
        degree: u32,
    },

    /// Point (for construction)
    Point {
        id: EntityId,
        position: [f64; 2],
    },
}

/// Sketch constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SketchConstraint {
    /// Fixed position
    Fixed { entity: EntityId },

    /// Horizontal constraint
    Horizontal { entity: EntityId },

    /// Vertical constraint
    Vertical { entity: EntityId },

    /// Coincident points
    Coincident { entity1: EntityId, entity2: EntityId },

    /// Parallel lines
    Parallel { entity1: EntityId, entity2: EntityId },

    /// Perpendicular lines
    Perpendicular { entity1: EntityId, entity2: EntityId },

    /// Tangent curves
    Tangent { entity1: EntityId, entity2: EntityId },

    /// Equal length/radius
    Equal { entity1: EntityId, entity2: EntityId },

    /// Concentric circles/arcs
    Concentric { entity1: EntityId, entity2: EntityId },

    /// Symmetric about a line
    Symmetric {
        entity1: EntityId,
        entity2: EntityId,
        axis: EntityId,
    },

    /// Distance dimension
    Distance {
        entity1: EntityId,
        entity2: Option<EntityId>,
        value: f64,
    },

    /// Angle dimension
    Angle {
        entity1: EntityId,
        entity2: EntityId,
        value: f64,
    },

    /// Radius dimension
    Radius { entity: EntityId, value: f64 },

    /// Diameter dimension
    Diameter { entity: EntityId, value: f64 },
}

/// Extrude feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtrudeFeature {
    /// Sketch to extrude
    pub sketch: FeatureId,

    /// Extrusion direction and distance
    pub direction: ExtrudeDirection,

    /// Draft angle (degrees)
    pub draft_angle: f64,

    /// Operation type for multiple bodies
    pub operation: BooleanOperation,
}

/// Extrusion direction specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtrudeDirection {
    /// Blind extrusion with fixed distance
    Blind { distance: f64 },

    /// Symmetric extrusion
    Symmetric { distance: f64 },

    /// Two-sided extrusion
    TwoSided { distance1: f64, distance2: f64 },

    /// Extrude to a face
    ToFace { feature_id: FeatureId, face_id: EntityId },

    /// Extrude through all
    ThroughAll,
}

/// Boolean operation type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum BooleanOperation {
    /// Create new body
    #[default]
    NewBody,
    /// Add to existing body
    Add,
    /// Subtract from existing body
    Subtract,
    /// Intersect with existing body
    Intersect,
}

/// Revolve feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevolveFeature {
    /// Sketch to revolve
    pub sketch: FeatureId,

    /// Axis of revolution
    pub axis: RevolveAxis,

    /// Angle to revolve (degrees)
    pub angle: f64,

    /// Operation type
    pub operation: BooleanOperation,
}

/// Axis of revolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RevolveAxis {
    /// X axis
    X,
    /// Y axis
    Y,
    /// Z axis
    Z,
    /// Sketch entity as axis
    SketchEntity(EntityId),
    /// Custom axis
    Custom { origin: Point3D, direction: DVec3 },
}

/// Sweep feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepFeature {
    /// Profile sketch
    pub profile: FeatureId,

    /// Path sketch or edge
    pub path: FeatureId,

    /// Orientation mode
    pub orientation: SweepOrientation,

    /// Operation type
    pub operation: BooleanOperation,
}

/// Sweep orientation mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SweepOrientation {
    /// Keep profile perpendicular to path
    KeepNormal,
    /// Keep profile parallel to original
    KeepParallel,
    /// Follow path curvature
    FollowPath,
}

/// Loft feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoftFeature {
    /// Profile sketches to loft between
    pub profiles: Vec<FeatureId>,

    /// Guide curves (optional)
    pub guides: Vec<FeatureId>,

    /// Operation type
    pub operation: BooleanOperation,
}

/// Fillet feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilletFeature {
    /// Target body
    pub target: FeatureId,

    /// Edges to fillet
    pub edges: Vec<EntityId>,

    /// Fillet radius
    pub radius: f64,

    /// Variable radius points (optional)
    pub variable_radius: Vec<(EntityId, f64, f64)>,
}

/// Chamfer feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChamferFeature {
    /// Target body
    pub target: FeatureId,

    /// Edges to chamfer
    pub edges: Vec<EntityId>,

    /// Chamfer type
    pub chamfer_type: ChamferType,
}

/// Chamfer specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChamferType {
    /// Equal distance from edge
    EqualDistance { distance: f64 },

    /// Two distances
    TwoDistances { distance1: f64, distance2: f64 },

    /// Distance and angle
    DistanceAngle { distance: f64, angle: f64 },
}

/// Shell (hollow) feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellFeature {
    /// Target body
    pub target: FeatureId,

    /// Wall thickness
    pub thickness: f64,

    /// Faces to remove (open faces)
    pub open_faces: Vec<EntityId>,
}

/// Pattern feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternFeature {
    /// Features to pattern
    pub source: Vec<FeatureId>,

    /// Pattern type
    pub pattern_type: PatternType,
}

/// Pattern types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    /// Linear pattern
    Linear {
        direction1: DVec3,
        count1: u32,
        spacing1: f64,
        direction2: Option<DVec3>,
        count2: Option<u32>,
        spacing2: Option<f64>,
    },

    /// Circular pattern
    Circular {
        axis: DVec3,
        center: Point3D,
        count: u32,
        angle: f64,
    },
}

/// Mirror feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorFeature {
    /// Features to mirror
    pub source: Vec<FeatureId>,

    /// Mirror plane
    pub plane: MirrorPlane,
}

/// Mirror plane definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MirrorPlane {
    /// XY plane
    XY,
    /// XZ plane
    XZ,
    /// YZ plane
    YZ,
    /// Face of geometry
    Face { feature_id: FeatureId, face_id: EntityId },
    /// Custom plane
    Custom { origin: Point3D, normal: DVec3 },
}

/// Imported geometry feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportFeature {
    /// Source file format
    pub format: ImportFormat,

    /// Original file path
    pub source_path: String,

    /// Stored geometry data (serialized)
    pub geometry_data: Vec<u8>,
}

/// Supported import formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImportFormat {
    STEP,
    IGES,
    STL,
    OBJ,
    GLTF,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_box_feature() {
        let feature = Feature::new(
            "Box1",
            FeatureData::Primitive(PrimitiveFeature::Box {
                width: 100.0,
                height: 50.0,
                depth: 25.0,
            }),
        );

        assert_eq!(feature.name, "Box1");
        assert_eq!(feature.feature_type(), "Box");
        assert!(!feature.suppressed);
    }

    #[test]
    fn test_feature_parameter() {
        let mut feature = Feature::new(
            "Cylinder1",
            FeatureData::Primitive(PrimitiveFeature::Cylinder {
                radius: 25.0,
                height: 100.0,
            }),
        );

        feature.set_parameter("radius", FeatureParameter::Value(30.0));
        feature.set_parameter("height", FeatureParameter::Link("main_height".to_string()));

        assert_eq!(feature.get_parameter("radius").unwrap().resolve(), Some(30.0));
        assert_eq!(feature.get_parameter("height").unwrap().resolve(), None);
    }
}
