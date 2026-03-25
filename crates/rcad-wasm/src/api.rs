//! Public API for JavaScript

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use js_sys::{Promise, Uint8Array, Float32Array, Uint32Array};
use web_sys::HtmlCanvasElement;
use std::sync::Arc;
use parking_lot::RwLock;

use rcad_core::{Document, Feature, FeatureData, FeatureId, PrimitiveFeature};
use rcad_geometry::{Mesh, Solid, primitives, tessellation};

/// CAD Document wrapper for JavaScript
#[wasm_bindgen]
pub struct CADDocument {
    inner: Arc<RwLock<Document>>,
    solids: Arc<RwLock<Vec<(FeatureId, Solid)>>>,
}

#[wasm_bindgen]
impl CADDocument {
    /// Create a new empty document
    #[wasm_bindgen(constructor)]
    pub fn new(name: Option<String>) -> Self {
        let doc = Document::new(name.unwrap_or_else(|| "Untitled".to_string()));
        Self {
            inner: Arc::new(RwLock::new(doc)),
            solids: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get document name
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.inner.read().name.clone()
    }

    /// Set document name
    #[wasm_bindgen(setter)]
    pub fn set_name(&self, name: String) {
        self.inner.write().name = name;
    }

    /// Get the number of features
    #[wasm_bindgen(getter)]
    pub fn feature_count(&self) -> usize {
        self.inner.read().features.len()
    }

    /// Create a box primitive
    #[wasm_bindgen]
    pub fn create_box(&mut self, width: f64, height: f64, depth: f64) -> Result<String, JsValue> {
        let solid = primitives::create_box(width, height, depth)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        let feature = Feature::new(
            format!("Box_{}", self.inner.read().features.len()),
            FeatureData::Primitive(PrimitiveFeature::Box { width, height, depth }),
        );

        let id = feature.id;
        self.inner.write().add_feature(feature);
        self.solids.write().push((id, solid));

        Ok(format!("{:?}", id.0))
    }

    /// Create a cylinder primitive
    #[wasm_bindgen]
    pub fn create_cylinder(&mut self, radius: f64, height: f64) -> Result<String, JsValue> {
        let solid = primitives::create_cylinder(radius, height)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        let feature = Feature::new(
            format!("Cylinder_{}", self.inner.read().features.len()),
            FeatureData::Primitive(PrimitiveFeature::Cylinder { radius, height }),
        );

        let id = feature.id;
        self.inner.write().add_feature(feature);
        self.solids.write().push((id, solid));

        Ok(format!("{:?}", id.0))
    }

    /// Create a sphere primitive
    #[wasm_bindgen]
    pub fn create_sphere(&mut self, radius: f64) -> Result<String, JsValue> {
        let solid = primitives::create_sphere(radius)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        let feature = Feature::new(
            format!("Sphere_{}", self.inner.read().features.len()),
            FeatureData::Primitive(PrimitiveFeature::Sphere { radius }),
        );

        let id = feature.id;
        self.inner.write().add_feature(feature);
        self.solids.write().push((id, solid));

        Ok(format!("{:?}", id.0))
    }

    /// Create a cone primitive
    #[wasm_bindgen]
    pub fn create_cone(&mut self, bottom_radius: f64, top_radius: f64, height: f64) -> Result<String, JsValue> {
        let solid = primitives::create_cone(bottom_radius, top_radius, height)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        let feature = Feature::new(
            format!("Cone_{}", self.inner.read().features.len()),
            FeatureData::Primitive(PrimitiveFeature::Cone {
                bottom_radius,
                top_radius,
                height,
            }),
        );

        let id = feature.id;
        self.inner.write().add_feature(feature);
        self.solids.write().push((id, solid));

        Ok(format!("{:?}", id.0))
    }

    /// Create a torus primitive
    #[wasm_bindgen]
    pub fn create_torus(&mut self, major_radius: f64, minor_radius: f64) -> Result<String, JsValue> {
        let solid = primitives::create_torus(major_radius, minor_radius)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        let feature = Feature::new(
            format!("Torus_{}", self.inner.read().features.len()),
            FeatureData::Primitive(PrimitiveFeature::Torus {
                major_radius,
                minor_radius,
            }),
        );

        let id = feature.id;
        self.inner.write().add_feature(feature);
        self.solids.write().push((id, solid));

        Ok(format!("{:?}", id.0))
    }

    /// Perform boolean union
    #[wasm_bindgen]
    pub fn boolean_union(&mut self, target_id: &str, tool_id: &str) -> Result<String, JsValue> {
        let target_uuid = parse_uuid(target_id)?;
        let tool_uuid = parse_uuid(tool_id)?;

        let solids = self.solids.read();

        let target = solids
            .iter()
            .find(|(id, _)| id.0 == target_uuid)
            .map(|(_, s)| s)
            .ok_or_else(|| JsValue::from_str("Target not found"))?;

        let tool = solids
            .iter()
            .find(|(id, _)| id.0 == tool_uuid)
            .map(|(_, s)| s)
            .ok_or_else(|| JsValue::from_str("Tool not found"))?;

        let result = rcad_geometry::boolean_union(target, tool)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        drop(solids);

        let target_feature_id = FeatureId(target_uuid);
        let tool_feature_id = FeatureId(tool_uuid);

        let feature = Feature::new(
            format!("Union_{}", self.inner.read().features.len()),
            FeatureData::Boolean(rcad_core::feature::BooleanFeature::Union {
                target: target_feature_id,
                tools: vec![tool_feature_id],
            }),
        );

        let id = feature.id;
        self.inner.write().add_feature(feature);
        self.solids.write().push((id, result));

        Ok(format!("{:?}", id.0))
    }

    /// Perform boolean subtraction
    #[wasm_bindgen]
    pub fn boolean_subtract(&mut self, target_id: &str, tool_id: &str) -> Result<String, JsValue> {
        let target_uuid = parse_uuid(target_id)?;
        let tool_uuid = parse_uuid(tool_id)?;

        let solids = self.solids.read();

        let target = solids
            .iter()
            .find(|(id, _)| id.0 == target_uuid)
            .map(|(_, s)| s)
            .ok_or_else(|| JsValue::from_str("Target not found"))?;

        let tool = solids
            .iter()
            .find(|(id, _)| id.0 == tool_uuid)
            .map(|(_, s)| s)
            .ok_or_else(|| JsValue::from_str("Tool not found"))?;

        let result = rcad_geometry::boolean_subtract(target, tool)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        drop(solids);

        let target_feature_id = FeatureId(target_uuid);
        let tool_feature_id = FeatureId(tool_uuid);

        let feature = Feature::new(
            format!("Subtract_{}", self.inner.read().features.len()),
            FeatureData::Boolean(rcad_core::feature::BooleanFeature::Subtract {
                target: target_feature_id,
                tools: vec![tool_feature_id],
            }),
        );

        let id = feature.id;
        self.inner.write().add_feature(feature);
        self.solids.write().push((id, result));

        Ok(format!("{:?}", id.0))
    }

    /// Tessellate a feature to mesh data
    #[wasm_bindgen]
    pub fn tessellate(&self, feature_id: &str) -> Result<MeshData, JsValue> {
        let uuid = parse_uuid(feature_id)?;

        let solids = self.solids.read();
        let solid = solids
            .iter()
            .find(|(id, _)| id.0 == uuid)
            .map(|(_, s)| s)
            .ok_or_else(|| JsValue::from_str("Feature not found"))?;

        let mesh = tessellation::tessellate_default(solid)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        Ok(MeshData::from_mesh(&mesh))
    }

    /// Export to STL
    #[wasm_bindgen]
    pub fn export_stl(&self, feature_id: &str, binary: bool) -> Result<Uint8Array, JsValue> {
        let uuid = parse_uuid(feature_id)?;

        let solids = self.solids.read();
        let solid = solids
            .iter()
            .find(|(id, _)| id.0 == uuid)
            .map(|(_, s)| s)
            .ok_or_else(|| JsValue::from_str("Feature not found"))?;

        let mesh = tessellation::tessellate_default(solid)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        let mut buffer = Vec::new();
        rcad_io::stl::export(&mut buffer, &mesh, binary)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        Ok(Uint8Array::from(buffer.as_slice()))
    }

    /// Export to OBJ
    #[wasm_bindgen]
    pub fn export_obj(&self, feature_id: &str) -> Result<String, JsValue> {
        let uuid = parse_uuid(feature_id)?;

        let solids = self.solids.read();
        let solid = solids
            .iter()
            .find(|(id, _)| id.0 == uuid)
            .map(|(_, s)| s)
            .ok_or_else(|| JsValue::from_str("Feature not found"))?;

        let mesh = tessellation::tessellate_default(solid)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        let mut buffer = Vec::new();
        rcad_io::obj::export(&mut buffer, &mesh)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;

        String::from_utf8(buffer).map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    /// Get all feature IDs
    #[wasm_bindgen]
    pub fn get_feature_ids(&self) -> Vec<JsValue> {
        self.inner
            .read()
            .features
            .keys()
            .map(|id| JsValue::from_str(&format!("{:?}", id.0)))
            .collect()
    }

    /// Get feature info
    #[wasm_bindgen]
    pub fn get_feature_info(&self, feature_id: &str) -> Result<JsValue, JsValue> {
        let uuid = parse_uuid(feature_id)?;
        let feature_id = FeatureId(uuid);

        let doc = self.inner.read();
        let feature = doc
            .get_feature(feature_id)
            .ok_or_else(|| JsValue::from_str("Feature not found"))?;

        let info = serde_json::json!({
            "id": format!("{:?}", feature.id.0),
            "name": feature.name,
            "type": feature.feature_type(),
            "suppressed": feature.suppressed,
        });

        serde_wasm_bindgen::to_value(&info).map_err(|e| JsValue::from_str(&format!("{:?}", e)))
    }

    /// Undo last action
    #[wasm_bindgen]
    pub fn undo(&mut self) -> bool {
        let doc = self.inner.read();
        doc.history.can_undo()
        // In a full implementation, would execute undo commands
    }

    /// Redo last undone action
    #[wasm_bindgen]
    pub fn redo(&mut self) -> bool {
        let doc = self.inner.read();
        doc.history.can_redo()
        // In a full implementation, would execute redo commands
    }

    /// Check if document has unsaved changes
    #[wasm_bindgen(getter)]
    pub fn is_dirty(&self) -> bool {
        self.inner.read().is_dirty
    }
}

/// Mesh data for JavaScript
#[wasm_bindgen]
pub struct MeshData {
    positions: Vec<f32>,
    normals: Vec<f32>,
    indices: Vec<u32>,
}

#[wasm_bindgen]
impl MeshData {
    /// Get vertex positions as Float32Array
    #[wasm_bindgen(getter)]
    pub fn positions(&self) -> Float32Array {
        Float32Array::from(self.positions.as_slice())
    }

    /// Get vertex normals as Float32Array
    #[wasm_bindgen(getter)]
    pub fn normals(&self) -> Float32Array {
        Float32Array::from(self.normals.as_slice())
    }

    /// Get triangle indices as Uint32Array
    #[wasm_bindgen(getter)]
    pub fn indices(&self) -> Uint32Array {
        Uint32Array::from(self.indices.as_slice())
    }

    /// Get vertex count
    #[wasm_bindgen(getter)]
    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    /// Get triangle count
    #[wasm_bindgen(getter)]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

impl MeshData {
    fn from_mesh(mesh: &Mesh) -> Self {
        Self {
            positions: mesh.positions.clone(),
            normals: mesh.normals.clone(),
            indices: mesh.indices.clone(),
        }
    }
}

/// Parse UUID from string
fn parse_uuid(s: &str) -> Result<uuid::Uuid, JsValue> {
    // Handle the debug format "UUID(...)"
    let cleaned = s
        .trim_start_matches("UUID(")
        .trim_end_matches(')')
        .trim();

    uuid::Uuid::parse_str(cleaned).map_err(|e| JsValue::from_str(&format!("Invalid UUID: {}", e)))
}

/// CAD Renderer wrapper
#[wasm_bindgen]
pub struct CADRenderer {
    // In a full implementation, this would hold the RenderEngine
}

#[wasm_bindgen]
impl CADRenderer {
    /// Create a new renderer
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {}
    }

    /// Initialize with a canvas
    #[wasm_bindgen]
    pub fn init(&mut self, canvas: HtmlCanvasElement) -> Promise {
        future_to_promise(async move {
            // In a full implementation:
            // 1. Get WebGPU adapter
            // 2. Create device and surface
            // 3. Initialize RenderEngine

            log::info!("Renderer initialized for canvas");
            Ok(JsValue::TRUE)
        })
    }

    /// Render a frame
    #[wasm_bindgen]
    pub fn render(&mut self, mesh_data: &MeshData) -> Result<(), JsValue> {
        // In a full implementation, would render the mesh
        Ok(())
    }

    /// Resize viewport
    #[wasm_bindgen]
    pub fn resize(&mut self, width: u32, height: u32) {
        log::info!("Resized to {}x{}", width, height);
    }

    /// Orbit camera
    #[wasm_bindgen]
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        // Would update camera orbit
    }

    /// Pan camera
    #[wasm_bindgen]
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        // Would update camera pan
    }

    /// Zoom camera
    #[wasm_bindgen]
    pub fn zoom(&mut self, delta: f32) {
        // Would update camera zoom
    }

    /// Set view to front
    #[wasm_bindgen]
    pub fn set_front_view(&mut self) {
        // Would set camera to front view
    }

    /// Set view to back
    #[wasm_bindgen]
    pub fn set_back_view(&mut self) {}

    /// Set view to left
    #[wasm_bindgen]
    pub fn set_left_view(&mut self) {}

    /// Set view to right
    #[wasm_bindgen]
    pub fn set_right_view(&mut self) {}

    /// Set view to top
    #[wasm_bindgen]
    pub fn set_top_view(&mut self) {}

    /// Set view to bottom
    #[wasm_bindgen]
    pub fn set_bottom_view(&mut self) {}

    /// Set view to isometric
    #[wasm_bindgen]
    pub fn set_isometric_view(&mut self) {}

    /// Fit view to content
    #[wasm_bindgen]
    pub fn fit_to_content(&mut self) {}
}
