//! Import API endpoints

use axum::{response::IntoResponse, Json};
use axum_extra::extract::Multipart;
use serde::Serialize;

/// Import response: a merged triangle mesh ready for the client renderer.
#[derive(Debug, Serialize, Default)]
pub struct ImportResponse {
    pub success: bool,
    pub message: String,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
    pub vertex_count: usize,
    pub triangle_count: usize,
}

impl ImportResponse {
    fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            ..Default::default()
        }
    }
}

/// Read the `file` field's bytes from a multipart form.
async fn read_file_field(multipart: &mut Multipart) -> Option<Vec<u8>> {
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        if field.name() == Some("file") {
            return Some(field.bytes().await.unwrap_or_default().to_vec());
        }
    }
    None
}

/// Merge a list of meshes into one (concatenate, offsetting indices).
pub(crate) fn merge_meshes(meshes: &[rcad_geometry::Mesh]) -> ImportResponse {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for mesh in meshes {
        let base = (positions.len() / 3) as u32;
        positions.extend_from_slice(&mesh.positions);
        normals.extend_from_slice(&mesh.normals);
        indices.extend(mesh.indices.iter().map(|i| i + base));
    }
    ImportResponse {
        success: true,
        message: "imported".to_string(),
        vertex_count: positions.len() / 3,
        triangle_count: indices.len() / 3,
        positions,
        normals,
        indices,
    }
}

/// Import a STEP file → tessellated mesh (truck-stepio, pure Rust).
pub async fn import_step(mut multipart: Multipart) -> impl IntoResponse {
    let Some(data) = read_file_field(&mut multipart).await else {
        return Json(ImportResponse::error("No file provided"));
    };
    tracing::info!("Importing STEP file, {} bytes", data.len());

    match rcad_io::step::import_from_bytes(&data) {
        Ok(meshes) => {
            let resp = merge_meshes(&meshes);
            tracing::info!(
                "STEP imported: {} vertices, {} triangles",
                resp.vertex_count,
                resp.triangle_count
            );
            Json(resp)
        }
        Err(e) => {
            tracing::warn!("STEP import failed: {e}");
            Json(ImportResponse::error(format!("STEP import failed: {e}")))
        }
    }
}

/// Import a glTF/GLB file → tessellated mesh (pure-Rust `gltf`). Handy for
/// bringing in models converted from formats truck-stepio can't read (e.g. a
/// complex STEP exported to GLB by FreeCAD). Original units are preserved.
pub async fn import_gltf(mut multipart: Multipart) -> impl IntoResponse {
    let Some(data) = read_file_field(&mut multipart).await else {
        return Json(ImportResponse::error("No file provided"));
    };
    tracing::info!("Importing glTF file, {} bytes", data.len());

    let opts = rcad_io::ImportOptions::new(); // scale = 1.0, keep the model's units
    match rcad_io::gltf::import(std::io::Cursor::new(&data), &opts) {
        Ok(model) => {
            let meshes: Vec<rcad_geometry::Mesh> =
                model.meshes.into_iter().map(|m| m.mesh).collect();
            if meshes.is_empty() {
                return Json(ImportResponse::error("glTF file contained no meshes"));
            }
            let resp = merge_meshes(&meshes);
            tracing::info!(
                "glTF imported: {} vertices, {} triangles",
                resp.vertex_count,
                resp.triangle_count
            );
            Json(resp)
        }
        Err(e) => {
            tracing::warn!("glTF import failed: {e}");
            Json(ImportResponse::error(format!("glTF import failed: {e}")))
        }
    }
}

/// IGES import is not supported (no pure-Rust IGES parser; needs OpenCASCADE).
pub async fn import_iges(mut _multipart: Multipart) -> impl IntoResponse {
    Json(ImportResponse::error(
        "IGES import is not supported. Convert to STEP, STL, or OBJ.",
    ))
}

/// Generic upload — dispatches by extension (STEP only for now).
pub async fn upload_file(mut multipart: Multipart) -> impl IntoResponse {
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().map(|s| s.to_lowercase()).unwrap_or_default();
        let data = field.bytes().await.unwrap_or_default();

        if filename.ends_with(".step") || filename.ends_with(".stp") {
            return match rcad_io::step::import_from_bytes(&data) {
                Ok(meshes) => Json(merge_meshes(&meshes)),
                Err(e) => Json(ImportResponse::error(format!("STEP import failed: {e}"))),
            };
        }
        return Json(ImportResponse::error(format!(
            "Server import not supported for '{filename}' (use STEP, or import STL/OBJ client-side)"
        )));
    }
    Json(ImportResponse::error("No file provided"))
}
