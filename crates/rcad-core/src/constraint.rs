//! Geometric constraints for rCAD
//!
//! Handles geometric and dimensional constraints for sketches
//! and assembly relationships.

use crate::{EntityId, Error, FeatureId, Point3D, Result};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Constraint solver state
#[derive(Debug, Clone, Default)]
pub struct ConstraintSolver {
    /// All constraints in the system
    constraints: Vec<Constraint>,

    /// Degrees of freedom remaining
    dof: i32,

    /// Whether the system is fully constrained
    fully_constrained: bool,

    /// Whether the system is over-constrained
    over_constrained: bool,
}

impl ConstraintSolver {
    /// Create a new constraint solver
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a constraint to the solver
    pub fn add_constraint(&mut self, constraint: Constraint) -> usize {
        let id = self.constraints.len();
        self.constraints.push(constraint);
        self.update_status();
        id
    }

    /// Remove a constraint by index
    pub fn remove_constraint(&mut self, index: usize) -> Option<Constraint> {
        if index < self.constraints.len() {
            let constraint = self.constraints.remove(index);
            self.update_status();
            Some(constraint)
        } else {
            None
        }
    }

    /// Get all constraints
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    /// Check if the system is fully constrained
    pub fn is_fully_constrained(&self) -> bool {
        self.fully_constrained
    }

    /// Check if the system is over-constrained
    pub fn is_over_constrained(&self) -> bool {
        self.over_constrained
    }

    /// Get remaining degrees of freedom
    pub fn degrees_of_freedom(&self) -> i32 {
        self.dof
    }

    /// Solve the constraint system
    pub fn solve(&mut self) -> Result<SolveResult> {
        // TODO: Implement actual constraint solving
        // This would involve:
        // 1. Building a system of equations
        // 2. Using Newton-Raphson or similar iterative solver
        // 3. Returning updated entity positions

        if self.over_constrained {
            return Ok(SolveResult::OverConstrained);
        }

        if !self.fully_constrained {
            return Ok(SolveResult::UnderConstrained { dof: self.dof });
        }

        Ok(SolveResult::Solved)
    }

    /// Update constraint status
    fn update_status(&mut self) {
        // Simplified DOF calculation
        // Real implementation would analyze the constraint graph
        let constraint_dof: i32 = self.constraints.iter().map(|c| c.dof_reduction()).sum();

        // Assuming some number of variables (simplified)
        let total_variables = 100; // This would come from actual geometry
        self.dof = total_variables - constraint_dof;
        self.fully_constrained = self.dof == 0;
        self.over_constrained = self.dof < 0;
    }
}

/// Result of constraint solving
#[derive(Debug, Clone)]
pub enum SolveResult {
    /// System solved successfully
    Solved,

    /// System is under-constrained
    UnderConstrained { dof: i32 },

    /// System is over-constrained
    OverConstrained,

    /// Solver failed to converge
    Failed { message: String },
}

/// A geometric or dimensional constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Unique identifier
    pub id: EntityId,

    /// Constraint type and parameters
    pub data: ConstraintData,

    /// Whether the constraint is active
    pub active: bool,

    /// Driving vs driven (reference)
    pub driving: bool,
}

impl Constraint {
    /// Create a new constraint
    pub fn new(data: ConstraintData) -> Self {
        Self {
            id: EntityId::new(),
            data,
            active: true,
            driving: true,
        }
    }

    /// Get the DOF reduction from this constraint
    pub fn dof_reduction(&self) -> i32 {
        if !self.active {
            return 0;
        }

        match &self.data {
            ConstraintData::Sketch(sc) => match sc {
                SketchConstraintData::Fixed { .. } => 2, // Fixes x and y
                SketchConstraintData::Horizontal { .. } => 1,
                SketchConstraintData::Vertical { .. } => 1,
                SketchConstraintData::Coincident { .. } => 2,
                SketchConstraintData::Parallel { .. } => 1,
                SketchConstraintData::Perpendicular { .. } => 1,
                SketchConstraintData::Tangent { .. } => 1,
                SketchConstraintData::Equal { .. } => 1,
                SketchConstraintData::Concentric { .. } => 2,
                SketchConstraintData::Symmetric { .. } => 2,
                SketchConstraintData::Distance { .. } => 1,
                SketchConstraintData::Angle { .. } => 1,
                SketchConstraintData::Radius { .. } => 1,
                SketchConstraintData::PointOnLine { .. } => 1,
                SketchConstraintData::PointOnCircle { .. } => 1,
                SketchConstraintData::Midpoint { .. } => 1,
            },
            ConstraintData::Assembly(ac) => match ac {
                AssemblyConstraintData::Fixed { .. } => 6, // All DOF
                AssemblyConstraintData::Mate { .. } => 1,
                AssemblyConstraintData::Align { .. } => 2,
                AssemblyConstraintData::Angle { .. } => 1,
                AssemblyConstraintData::Tangent { .. } => 1,
                AssemblyConstraintData::Insert { .. } => 3,
                AssemblyConstraintData::Parallel { .. } => 2,
                AssemblyConstraintData::Perpendicular { .. } => 2,
                AssemblyConstraintData::Concentric { .. } => 2,
                AssemblyConstraintData::Lock { .. } => 6,
                AssemblyConstraintData::Distance { .. } => 1,
            },
        }
    }
}

/// Different types of constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintData {
    /// 2D sketch constraints
    Sketch(SketchConstraintData),

    /// 3D assembly constraints
    Assembly(AssemblyConstraintData),
}

/// 2D sketch constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SketchConstraintData {
    /// Fix a point in place
    Fixed { point: EntityId },

    /// Make a line horizontal
    Horizontal { line: EntityId },

    /// Make a line vertical
    Vertical { line: EntityId },

    /// Make two points coincident
    Coincident { point1: EntityId, point2: EntityId },

    /// Make two lines parallel
    Parallel { line1: EntityId, line2: EntityId },

    /// Make two lines perpendicular
    Perpendicular { line1: EntityId, line2: EntityId },

    /// Make two curves tangent
    Tangent { curve1: EntityId, curve2: EntityId },

    /// Make two entities equal (length or radius)
    Equal { entity1: EntityId, entity2: EntityId },

    /// Make two circles/arcs concentric
    Concentric { circle1: EntityId, circle2: EntityId },

    /// Make two points symmetric about a line
    Symmetric {
        point1: EntityId,
        point2: EntityId,
        axis: EntityId,
    },

    /// Distance between entities
    Distance {
        entity1: EntityId,
        entity2: Option<EntityId>,
        value: f64,
    },

    /// Angle between lines
    Angle {
        line1: EntityId,
        line2: EntityId,
        value: f64,
    },

    /// Radius of circle/arc
    Radius { circle: EntityId, value: f64 },

    /// Point on line
    PointOnLine { point: EntityId, line: EntityId },

    /// Point on circle
    PointOnCircle { point: EntityId, circle: EntityId },

    /// Point at midpoint of line
    Midpoint { point: EntityId, line: EntityId },
}

/// 3D assembly constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssemblyConstraintData {
    /// Fix a component in place
    Fixed { component: FeatureId },

    /// Mate two faces (coplanar, touching)
    Mate {
        face1: (FeatureId, EntityId),
        face2: (FeatureId, EntityId),
        offset: f64,
        flip: bool,
    },

    /// Align two faces (coplanar, same direction)
    Align {
        face1: (FeatureId, EntityId),
        face2: (FeatureId, EntityId),
        offset: f64,
    },

    /// Angle between faces/planes
    Angle {
        entity1: (FeatureId, EntityId),
        entity2: (FeatureId, EntityId),
        value: f64,
    },

    /// Tangent surfaces
    Tangent {
        surface1: (FeatureId, EntityId),
        surface2: (FeatureId, EntityId),
    },

    /// Insert (cylindrical mate)
    Insert {
        axis1: (FeatureId, EntityId),
        axis2: (FeatureId, EntityId),
    },

    /// Parallel axes or planes
    Parallel {
        entity1: (FeatureId, EntityId),
        entity2: (FeatureId, EntityId),
    },

    /// Perpendicular axes or planes
    Perpendicular {
        entity1: (FeatureId, EntityId),
        entity2: (FeatureId, EntityId),
    },

    /// Concentric cylinders
    Concentric {
        cylinder1: (FeatureId, EntityId),
        cylinder2: (FeatureId, EntityId),
    },

    /// Lock all DOF between two components
    Lock {
        component1: FeatureId,
        component2: FeatureId,
    },

    /// Distance between entities
    Distance {
        entity1: (FeatureId, EntityId),
        entity2: (FeatureId, EntityId),
        value: f64,
    },
}

/// Constraint status for display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintStatus {
    /// Constraint is satisfied
    Satisfied,

    /// Constraint is not satisfied (broken)
    Broken,

    /// Constraint is redundant (over-constrained)
    Redundant,

    /// Constraint is disabled
    Disabled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_solver() {
        let mut solver = ConstraintSolver::new();

        // Add some constraints
        solver.add_constraint(Constraint::new(ConstraintData::Sketch(
            SketchConstraintData::Horizontal { line: EntityId::new() },
        )));

        solver.add_constraint(Constraint::new(ConstraintData::Sketch(
            SketchConstraintData::Distance {
                entity1: EntityId::new(),
                entity2: None,
                value: 100.0,
            },
        )));

        assert_eq!(solver.constraints().len(), 2);
    }

    #[test]
    fn test_dof_reduction() {
        let fixed = Constraint::new(ConstraintData::Sketch(SketchConstraintData::Fixed {
            point: EntityId::new(),
        }));
        assert_eq!(fixed.dof_reduction(), 2);

        let horizontal = Constraint::new(ConstraintData::Sketch(SketchConstraintData::Horizontal {
            line: EntityId::new(),
        }));
        assert_eq!(horizontal.dof_reduction(), 1);
    }
}
