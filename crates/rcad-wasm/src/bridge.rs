//! JavaScript ↔ Rust type conversions

use wasm_bindgen::prelude::*;
use js_sys::{Object, Reflect, Array};
use serde::{Deserialize, Serialize};

/// Convert a Rust struct to a JavaScript object
pub fn to_js_object<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value)
        .map_err(|e| JsValue::from_str(&format!("Serialization error: {:?}", e)))
}

/// Convert a JavaScript object to a Rust struct
pub fn from_js_object<T: for<'de> Deserialize<'de>>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value)
        .map_err(|e| JsValue::from_str(&format!("Deserialization error: {:?}", e)))
}

/// Vector3 for JS interop
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct JsVector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl JsVector3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn to_glam(&self) -> glam::DVec3 {
        glam::DVec3::new(self.x, self.y, self.z)
    }

    pub fn from_glam(v: glam::DVec3) -> Self {
        Self { x: v.x, y: v.y, z: v.z }
    }
}

/// Matrix4 for JS interop
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct JsMatrix4 {
    pub elements: [f64; 16],
}

impl JsMatrix4 {
    pub fn identity() -> Self {
        Self {
            elements: [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    pub fn to_glam(&self) -> glam::DMat4 {
        glam::DMat4::from_cols_array(&self.elements)
    }

    pub fn from_glam(m: glam::DMat4) -> Self {
        Self {
            elements: m.to_cols_array(),
        }
    }
}

/// Bounding box for JS interop
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct JsBoundingBox {
    pub min: JsVector3,
    pub max: JsVector3,
}

impl JsBoundingBox {
    pub fn new(min: JsVector3, max: JsVector3) -> Self {
        Self { min, max }
    }

    pub fn center(&self) -> JsVector3 {
        JsVector3 {
            x: (self.min.x + self.max.x) / 2.0,
            y: (self.min.y + self.max.y) / 2.0,
            z: (self.min.z + self.max.z) / 2.0,
        }
    }

    pub fn size(&self) -> JsVector3 {
        JsVector3 {
            x: self.max.x - self.min.x,
            y: self.max.y - self.min.y,
            z: self.max.z - self.min.z,
        }
    }
}

/// Color for JS interop
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct JsColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl JsColor {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

/// Material parameters for JS interop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsMaterial {
    pub base_color: JsColor,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: f32,
}

impl Default for JsMaterial {
    fn default() -> Self {
        Self {
            base_color: JsColor::rgb(0.8, 0.8, 0.8),
            metallic: 0.0,
            roughness: 0.5,
            emissive: 0.0,
        }
    }
}

/// Feature info for JS interop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsFeatureInfo {
    pub id: String,
    pub name: String,
    pub feature_type: String,
    pub suppressed: bool,
    pub transform: JsMatrix4,
    pub bounding_box: Option<JsBoundingBox>,
}

/// Document info for JS interop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsDocumentInfo {
    pub id: String,
    pub name: String,
    pub feature_count: usize,
    pub is_dirty: bool,
}

/// Convert JavaScript array to Rust Vec
pub fn js_array_to_vec<T: for<'de> Deserialize<'de>>(array: &Array) -> Result<Vec<T>, JsValue> {
    let mut result = Vec::with_capacity(array.length() as usize);

    for i in 0..array.length() {
        let item = array.get(i);
        let value: T = from_js_object(item)?;
        result.push(value);
    }

    Ok(result)
}

/// Convert Rust Vec to JavaScript array
pub fn vec_to_js_array<T: Serialize>(vec: &[T]) -> Result<Array, JsValue> {
    let array = Array::new_with_length(vec.len() as u32);

    for (i, item) in vec.iter().enumerate() {
        let js_value = to_js_object(item)?;
        array.set(i as u32, js_value);
    }

    Ok(array)
}
