//! Document model for rCAD
//!
//! Represents a complete CAD document with feature tree, metadata, and settings.

use crate::{EntityId, Error, FeatureId, Result, Units, feature::Feature, history::History};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A complete CAD document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Document unique identifier
    pub id: EntityId,

    /// Document name
    pub name: String,

    /// Document description
    pub description: String,

    /// Default units for the document
    pub units: Units,

    /// Feature tree - ordered map preserving creation order
    pub features: IndexMap<FeatureId, Feature>,

    /// Feature parent relationships (child -> parent)
    pub feature_parents: HashMap<FeatureId, FeatureId>,

    /// Named parameters for the document
    pub parameters: HashMap<String, ParameterValue>,

    /// Document metadata
    pub metadata: DocumentMetadata,

    /// Undo/redo history
    #[serde(skip)]
    pub history: History,

    /// Whether the document has unsaved changes
    #[serde(skip)]
    pub is_dirty: bool,
}

impl Document {
    /// Create a new empty document
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: EntityId::new(),
            name: name.into(),
            description: String::new(),
            units: Units::default(),
            features: IndexMap::new(),
            feature_parents: HashMap::new(),
            parameters: HashMap::new(),
            metadata: DocumentMetadata::default(),
            history: History::new(),
            is_dirty: false,
        }
    }

    /// Add a feature to the document
    pub fn add_feature(&mut self, feature: Feature) -> FeatureId {
        let id = feature.id;
        self.features.insert(id, feature);
        self.is_dirty = true;
        id
    }

    /// Add a feature as a child of another feature
    pub fn add_feature_child(&mut self, feature: Feature, parent: FeatureId) -> Result<FeatureId> {
        if !self.features.contains_key(&parent) {
            return Err(Error::FeatureNotFound(parent));
        }
        let id = feature.id;
        self.features.insert(id, feature);
        self.feature_parents.insert(id, parent);
        self.is_dirty = true;
        Ok(id)
    }

    /// Get a feature by ID
    pub fn get_feature(&self, id: FeatureId) -> Option<&Feature> {
        self.features.get(&id)
    }

    /// Get a mutable feature by ID
    pub fn get_feature_mut(&mut self, id: FeatureId) -> Option<&mut Feature> {
        self.is_dirty = true;
        self.features.get_mut(&id)
    }

    /// Remove a feature and all its children
    pub fn remove_feature(&mut self, id: FeatureId) -> Result<Feature> {
        // First, collect all children to remove
        let children: Vec<FeatureId> = self.feature_parents
            .iter()
            .filter(|(_, &parent)| parent == id)
            .map(|(&child, _)| child)
            .collect();

        // Remove children recursively
        for child in children {
            self.remove_feature(child)?;
        }

        // Remove parent relationship
        self.feature_parents.remove(&id);

        // Remove the feature
        self.features
            .swap_remove(&id)
            .ok_or(Error::FeatureNotFound(id))
            .map(|f| {
                self.is_dirty = true;
                f
            })
    }

    /// Get the parent of a feature
    pub fn get_parent(&self, id: FeatureId) -> Option<FeatureId> {
        self.feature_parents.get(&id).copied()
    }

    /// Get all children of a feature
    pub fn get_children(&self, id: FeatureId) -> Vec<FeatureId> {
        self.feature_parents
            .iter()
            .filter(|(_, &parent)| parent == id)
            .map(|(&child, _)| child)
            .collect()
    }

    /// Get root features (features without parents)
    pub fn get_root_features(&self) -> Vec<FeatureId> {
        self.features
            .keys()
            .filter(|id| !self.feature_parents.contains_key(id))
            .copied()
            .collect()
    }

    /// Set a document parameter
    pub fn set_parameter(&mut self, name: impl Into<String>, value: ParameterValue) {
        self.parameters.insert(name.into(), value);
        self.is_dirty = true;
    }

    /// Get a document parameter
    pub fn get_parameter(&self, name: &str) -> Option<&ParameterValue> {
        self.parameters.get(name)
    }

    /// Get all features in dependency order
    pub fn features_in_order(&self) -> impl Iterator<Item = (&FeatureId, &Feature)> {
        self.features.iter()
    }

    /// Recompute all features based on their parameters
    pub fn recompute(&mut self) -> Result<()> {
        // TODO: Implement full parametric recomputation
        // This would involve:
        // 1. Topologically sorting features by dependencies
        // 2. Evaluating each feature's parameters
        // 3. Regenerating geometry
        Ok(())
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

/// Document metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentMetadata {
    /// Author name
    pub author: String,

    /// Company/organization
    pub company: String,

    /// Creation timestamp (Unix epoch)
    pub created_at: u64,

    /// Last modification timestamp
    pub modified_at: u64,

    /// Document version
    pub version: u32,

    /// Custom properties
    pub custom: HashMap<String, String>,
}

/// Parameter value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterValue {
    /// Numeric value with optional unit
    Number(f64),

    /// Integer value
    Integer(i64),

    /// Boolean value
    Boolean(bool),

    /// String value
    String(String),

    /// Expression that evaluates to a number
    Expression(String),
}

impl ParameterValue {
    /// Try to get the value as a number
    pub fn as_number(&self) -> Option<f64> {
        match self {
            ParameterValue::Number(n) => Some(*n),
            ParameterValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to get the value as an integer
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            ParameterValue::Integer(i) => Some(*i),
            ParameterValue::Number(n) => Some(*n as i64),
            _ => None,
        }
    }

    /// Try to get the value as a boolean
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            ParameterValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get the value as a string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ParameterValue::String(s) => Some(s),
            ParameterValue::Expression(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::{Feature, FeatureData, PrimitiveFeature};

    #[test]
    fn test_document_creation() {
        let doc = Document::new("Test Document");
        assert_eq!(doc.name, "Test Document");
        assert!(doc.features.is_empty());
    }

    #[test]
    fn test_add_feature() {
        let mut doc = Document::new("Test");
        let feature = Feature::new(
            "Box1",
            FeatureData::Primitive(PrimitiveFeature::Box {
                width: 100.0,
                height: 100.0,
                depth: 100.0,
            }),
        );
        let id = doc.add_feature(feature);
        assert!(doc.get_feature(id).is_some());
    }

    #[test]
    fn test_feature_hierarchy() {
        let mut doc = Document::new("Test");

        let parent = Feature::new(
            "Parent",
            FeatureData::Primitive(PrimitiveFeature::Box {
                width: 100.0,
                height: 100.0,
                depth: 100.0,
            }),
        );
        let parent_id = doc.add_feature(parent);

        let child = Feature::new(
            "Child",
            FeatureData::Primitive(PrimitiveFeature::Sphere { radius: 50.0 }),
        );
        let child_id = doc.add_feature_child(child, parent_id).unwrap();

        assert_eq!(doc.get_parent(child_id), Some(parent_id));
        assert!(doc.get_children(parent_id).contains(&child_id));
    }
}
