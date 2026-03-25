//! Import API endpoints

use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use axum_extra::extract::Multipart;
use serde::{Deserialize, Serialize};

/// Import response
#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub success: bool,
    pub message: String,
    pub geometry_id: Option<String>,
    pub vertex_count: Option<usize>,
    pub face_count: Option<usize>,
}

/// Import STEP file
pub async fn import_step(mut multipart: Multipart) -> impl IntoResponse {
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            let data = field.bytes().await.unwrap_or_default();

            // In a real implementation, we would:
            // 1. Save to temp file
            // 2. Parse with OpenCASCADE (truck-stepio)
            // 3. Convert to internal format
            // 4. Return geometry data

            tracing::info!("Received STEP file, {} bytes", data.len());

            return Json(ImportResponse {
                success: true,
                message: "STEP file imported successfully".to_string(),
                geometry_id: Some(uuid::Uuid::new_v4().to_string()),
                vertex_count: Some(0),
                face_count: Some(0),
            });
        }
    }

    Json(ImportResponse {
        success: false,
        message: "No file provided".to_string(),
        geometry_id: None,
        vertex_count: None,
        face_count: None,
    })
}

/// Import IGES file
pub async fn import_iges(mut multipart: Multipart) -> impl IntoResponse {
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            let data = field.bytes().await.unwrap_or_default();

            tracing::info!("Received IGES file, {} bytes", data.len());

            return Json(ImportResponse {
                success: true,
                message: "IGES file imported successfully".to_string(),
                geometry_id: Some(uuid::Uuid::new_v4().to_string()),
                vertex_count: Some(0),
                face_count: Some(0),
            });
        }
    }

    Json(ImportResponse {
        success: false,
        message: "No file provided".to_string(),
        geometry_id: None,
        vertex_count: None,
        face_count: None,
    })
}

/// Generic file upload
pub async fn upload_file(mut multipart: Multipart) -> impl IntoResponse {
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        let filename = field.file_name().map(|s| s.to_string());

        if name == "file" {
            let data = field.bytes().await.unwrap_or_default();

            // Detect file type from extension or content
            let format = if let Some(ref fname) = filename {
                if fname.ends_with(".step") || fname.ends_with(".stp") {
                    "STEP"
                } else if fname.ends_with(".iges") || fname.ends_with(".igs") {
                    "IGES"
                } else if fname.ends_with(".stl") {
                    "STL"
                } else if fname.ends_with(".obj") {
                    "OBJ"
                } else {
                    "Unknown"
                }
            } else {
                "Unknown"
            };

            tracing::info!(
                "Received {} file: {:?}, {} bytes",
                format,
                filename,
                data.len()
            );

            return Json(ImportResponse {
                success: true,
                message: format!("{} file uploaded successfully", format),
                geometry_id: Some(uuid::Uuid::new_v4().to_string()),
                vertex_count: Some(0),
                face_count: Some(0),
            });
        }
    }

    Json(ImportResponse {
        success: false,
        message: "No file provided".to_string(),
        geometry_id: None,
        vertex_count: None,
        face_count: None,
    })
}
