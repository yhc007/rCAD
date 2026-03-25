//! # rcad-core
//!
//! Core CAD data structures for rCAD including document model,
//! parametric features, geometric constraints, and undo/redo history.

pub mod constraint;
pub mod document;
pub mod feature;
pub mod history;

pub use constraint::*;
pub use document::*;
pub use feature::*;
pub use history::*;

use glam::{DVec2, DVec3, DMat4};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for entities in the CAD system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub Uuid);

impl EntityId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for features
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeatureId(pub Uuid);

impl FeatureId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for FeatureId {
    fn default() -> Self {
        Self::new()
    }
}

/// 3D point in world coordinates
pub type Point3D = DVec3;

/// 2D point in sketch coordinates
pub type Point2D = DVec2;

/// 4x4 transformation matrix
pub type Transform = DMat4;

/// Units of measurement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Units {
    #[default]
    Millimeters,
    Centimeters,
    Meters,
    Inches,
    Feet,
}

impl Units {
    /// Conversion factor to millimeters
    pub fn to_mm(&self) -> f64 {
        match self {
            Units::Millimeters => 1.0,
            Units::Centimeters => 10.0,
            Units::Meters => 1000.0,
            Units::Inches => 25.4,
            Units::Feet => 304.8,
        }
    }

    /// Convert a value from this unit to millimeters
    pub fn convert_to_mm(&self, value: f64) -> f64 {
        value * self.to_mm()
    }

    /// Convert a value from millimeters to this unit
    pub fn convert_from_mm(&self, value: f64) -> f64 {
        value / self.to_mm()
    }
}

/// Result type for rcad-core operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for rcad-core
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Entity not found: {0:?}")]
    EntityNotFound(EntityId),

    #[error("Feature not found: {0:?}")]
    FeatureNotFound(FeatureId),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Constraint error: {0}")]
    ConstraintError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("History error: {0}")]
    HistoryError(String),
}
