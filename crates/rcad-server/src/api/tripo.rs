//! Tripo text-to-3D endpoint.
//!
//! Generates an organic mesh from a text prompt via the Tripo API (key in
//! ~/.tripo), downloads the resulting glTF, tessellates it and returns the same
//! merged-mesh payload the client uses for STEP import. Tripo generation is
//! slow (tens of seconds) and costs credits, so the AI tool that calls this is
//! gated behind user approval.

use axum::{http::StatusCode, response::IntoResponse, Json};
use rcad_geometry::Mesh;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{io::Cursor, time::Duration};

use crate::api::import::merge_meshes;

const TRIPO_BASE: &str = "https://api.tripo3d.ai/v2/openapi";
const TARGET_SIZE: f32 = 50.0; // normalise the generated model to ~50 mm
const POLL_SECS: u64 = 3;
const MAX_POLLS: u32 = 60; // ~3 minutes

#[derive(Deserialize)]
pub struct GenerateRequest {
    prompt: String,
}

fn bad(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "success": false, "error": msg.into() })))
}

/// `POST /api/tripo/generate` — text prompt → merged mesh (STEP-style payload).
pub async fn generate(Json(req): Json<GenerateRequest>) -> impl IntoResponse {
    let Some(key) = super::ai::read_key(".tripo") else {
        return bad(
            StatusCode::SERVICE_UNAVAILABLE,
            "3D generation is not configured: put the Tripo API key in ~/.tripo on the server.",
        );
    };
    let prompt = req.prompt.trim().to_string();
    if prompt.is_empty() {
        return bad(StatusCode::BAD_REQUEST, "prompt is required");
    }

    let client = reqwest::Client::new();

    // 1) Create the task.
    let task_id = match create_task(&client, &key, &prompt).await {
        Ok(id) => id,
        Err(e) => return bad(StatusCode::BAD_GATEWAY, e),
    };
    tracing::info!("Tripo task {task_id} created for prompt: {prompt}");

    // 2) Poll until it finishes.
    let model_url = match poll_task(&client, &key, &task_id).await {
        Ok(url) => url,
        Err(e) => return bad(StatusCode::BAD_GATEWAY, e),
    };

    // 3) Download + tessellate the glTF.
    let bytes = match client.get(&model_url).send().await.and_then(|r| r.error_for_status()) {
        Ok(r) => match r.bytes().await {
            Ok(b) => b,
            Err(e) => return bad(StatusCode::BAD_GATEWAY, format!("download failed: {e}")),
        },
        Err(e) => return bad(StatusCode::BAD_GATEWAY, format!("download failed: {e}")),
    };

    // NB: ImportOptions::new() sets scale=1.0; the derived Default would be 0.0
    // and zero out every vertex position.
    let opts = rcad_io::ImportOptions::new();
    let model = match rcad_io::gltf::import(Cursor::new(bytes.as_ref()), &opts) {
        Ok(m) => m,
        Err(e) => return bad(StatusCode::UNPROCESSABLE_ENTITY, format!("glTF parse failed: {e}")),
    };
    let mut meshes: Vec<Mesh> = model.meshes.into_iter().map(|m| m.mesh).collect();
    if meshes.is_empty() {
        return bad(StatusCode::UNPROCESSABLE_ENTITY, "generated model had no meshes");
    }
    normalize(&mut meshes, TARGET_SIZE);

    let resp = merge_meshes(&meshes);
    tracing::info!("Tripo import: {} verts, {} tris", resp.vertex_count, resp.triangle_count);
    (StatusCode::OK, Json(serde_json::to_value(resp).unwrap()))
}

async fn create_task(client: &reqwest::Client, key: &str, prompt: &str) -> Result<String, String> {
    let resp = client
        .post(format!("{TRIPO_BASE}/task"))
        .bearer_auth(key)
        .json(&json!({ "type": "text_to_model", "prompt": prompt }))
        .send()
        .await
        .map_err(|e| format!("Tripo create failed: {e}"))?;
    let data: Value = resp.json().await.map_err(|e| format!("bad Tripo response: {e}"))?;
    data["data"]["task_id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no task_id from Tripo: {data}"))
}

async fn poll_task(client: &reqwest::Client, key: &str, task_id: &str) -> Result<String, String> {
    for _ in 0..MAX_POLLS {
        tokio::time::sleep(Duration::from_secs(POLL_SECS)).await;
        let resp = client
            .get(format!("{TRIPO_BASE}/task/{task_id}"))
            .bearer_auth(key)
            .send()
            .await
            .map_err(|e| format!("Tripo poll failed: {e}"))?;
        let data: Value = resp.json().await.map_err(|e| format!("bad Tripo response: {e}"))?;
        let status = data["data"]["status"].as_str().unwrap_or("");
        match status {
            "success" => {
                let out = &data["data"]["output"];
                let url = out["pbr_model"]
                    .as_str()
                    .or_else(|| out["model"].as_str())
                    .or_else(|| out["base_model"].as_str());
                return url
                    .map(|s| s.to_string())
                    .ok_or_else(|| format!("no model URL in Tripo output: {out}"));
            }
            "failed" | "cancelled" | "banned" | "expired" => {
                return Err(format!("Tripo generation {status}"));
            }
            _ => continue, // queued / running
        }
    }
    Err("Tripo generation timed out".into())
}

/// Centre the meshes at the origin and scale so the largest dimension is `target`.
fn normalize(meshes: &mut [Mesh], target: f32) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for m in meshes.iter() {
        for v in m.positions.chunks(3) {
            for k in 0..3 {
                min[k] = min[k].min(v[k]);
                max[k] = max[k].max(v[k]);
            }
        }
    }
    let dim = (0..3).map(|k| max[k] - min[k]).fold(0.0_f32, f32::max);
    if dim <= 1e-6 {
        return;
    }
    let s = target / dim;
    let c = [
        (min[0] + max[0]) / 2.0,
        (min[1] + max[1]) / 2.0,
        (min[2] + max[2]) / 2.0,
    ];
    for m in meshes.iter_mut() {
        for v in m.positions.chunks_mut(3) {
            for k in 0..3 {
                v[k] = (v[k] - c[k]) * s;
            }
        }
    }
}
