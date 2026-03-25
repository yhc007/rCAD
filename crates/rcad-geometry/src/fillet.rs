//! Fillet and chamfer operations
//!
//! Provides edge rounding and chamfering for solids.

use crate::{GeometryError, Result, Solid};

/// Fillet configuration
#[derive(Debug, Clone)]
pub struct FilletConfig {
    /// Fillet radius
    pub radius: f64,

    /// Edge indices to fillet (empty means all edges)
    pub edges: Vec<usize>,
}

impl FilletConfig {
    /// Create a new fillet configuration
    pub fn new(radius: f64) -> Self {
        Self {
            radius,
            edges: Vec::new(),
        }
    }

    /// Set specific edges to fillet
    pub fn with_edges(mut self, edges: Vec<usize>) -> Self {
        self.edges = edges;
        self
    }
}

/// Chamfer configuration
#[derive(Debug, Clone)]
pub struct ChamferConfig {
    /// Chamfer type
    pub chamfer_type: ChamferType,

    /// Edge indices to chamfer (empty means all edges)
    pub edges: Vec<usize>,
}

/// Type of chamfer
#[derive(Debug, Clone)]
pub enum ChamferType {
    /// Equal distance from edge on both sides
    EqualDistance(f64),

    /// Two different distances from edge
    TwoDistances(f64, f64),

    /// Distance and angle
    DistanceAngle { distance: f64, angle: f64 },
}

impl ChamferConfig {
    /// Create a chamfer with equal distance
    pub fn equal_distance(distance: f64) -> Self {
        Self {
            chamfer_type: ChamferType::EqualDistance(distance),
            edges: Vec::new(),
        }
    }

    /// Create a chamfer with two distances
    pub fn two_distances(d1: f64, d2: f64) -> Self {
        Self {
            chamfer_type: ChamferType::TwoDistances(d1, d2),
            edges: Vec::new(),
        }
    }

    /// Create a chamfer with distance and angle
    pub fn distance_angle(distance: f64, angle: f64) -> Self {
        Self {
            chamfer_type: ChamferType::DistanceAngle { distance, angle },
            edges: Vec::new(),
        }
    }

    /// Set specific edges to chamfer
    pub fn with_edges(mut self, edges: Vec<usize>) -> Self {
        self.edges = edges;
        self
    }
}

/// Apply fillets to a solid
///
/// Note: truck does not have built-in fillet support.
/// This is a placeholder that returns the original solid.
/// Full implementation would require:
/// 1. Identifying edge curves
/// 2. Computing rolling ball blends
/// 3. Replacing edge topology with fillet surfaces
pub fn fillet(solid: &Solid, config: &FilletConfig) -> Result<Solid> {
    if config.radius <= 0.0 {
        return Err(GeometryError::InvalidParameter(
            "Fillet radius must be positive".to_string(),
        ));
    }

    // TODO: Implement proper fillet using surface blending
    // For now, return the original solid with a warning

    // TODO: Implement proper fillet/chamfer
    eprintln!("Fillet operation not yet implemented in truck - returning original solid");
    Ok(solid.clone())
}

/// Apply chamfers to a solid
///
/// Note: truck does not have built-in chamfer support.
/// This is a placeholder that returns the original solid.
pub fn chamfer(solid: &Solid, config: &ChamferConfig) -> Result<Solid> {
    let distance = match &config.chamfer_type {
        ChamferType::EqualDistance(d) => *d,
        ChamferType::TwoDistances(d1, _) => *d1,
        ChamferType::DistanceAngle { distance, .. } => *distance,
    };

    if distance <= 0.0 {
        return Err(GeometryError::InvalidParameter(
            "Chamfer distance must be positive".to_string(),
        ));
    }

    // TODO: Implement proper chamfer
    // For now, return the original solid with a warning

    // TODO: Implement proper fillet/chamfer
    eprintln!("Chamfer operation not yet implemented in truck - returning original solid");
    Ok(solid.clone())
}

/// Apply variable radius fillet
///
/// Allows specifying different radii at different points along an edge.
pub fn variable_fillet(
    solid: &Solid,
    edge_index: usize,
    radii: &[(f64, f64)], // (parameter, radius) pairs
) -> Result<Solid> {
    if radii.is_empty() {
        return Err(GeometryError::InvalidParameter(
            "Variable fillet requires at least one radius point".to_string(),
        ));
    }

    for (_, radius) in radii {
        if *radius <= 0.0 {
            return Err(GeometryError::InvalidParameter(
                "Fillet radius must be positive".to_string(),
            ));
        }
    }

    // TODO: Implement variable radius fillet
    // TODO: Implement proper fillet/chamfer
    eprintln!("Variable fillet not yet implemented - returning original solid");
    Ok(solid.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn test_fillet_config() {
        let config = FilletConfig::new(5.0).with_edges(vec![0, 1, 2]);
        assert_eq!(config.radius, 5.0);
        assert_eq!(config.edges.len(), 3);
    }

    #[test]
    fn test_chamfer_config() {
        let config = ChamferConfig::equal_distance(3.0);
        match config.chamfer_type {
            ChamferType::EqualDistance(d) => assert_eq!(d, 3.0),
            _ => panic!("Wrong chamfer type"),
        }
    }

    #[test]
    fn test_fillet_placeholder() {
        let solid = primitives::create_box(10.0, 10.0, 10.0).unwrap();
        let config = FilletConfig::new(2.0);

        let result = fillet(&solid, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fillet_invalid_radius() {
        let solid = primitives::create_box(10.0, 10.0, 10.0).unwrap();
        let config = FilletConfig::new(-1.0);

        let result = fillet(&solid, &config);
        assert!(result.is_err());
    }
}
