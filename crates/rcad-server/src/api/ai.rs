//! AI copilot endpoint.
//!
//! A thin proxy in front of the GLM (Zhipu) chat-completions API, which is
//! OpenAI-compatible. The browser never sees the API key — it is read from the
//! `~/.GLM` file server-side. The client builds the full request (model,
//! messages, tools); this handler injects auth, applies a few safety clamps,
//! and forwards it. The agent loop (tool execution against the live document)
//! runs client-side, so this stays stateless.

use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use std::{env, fs};

const DEFAULT_URL: &str = "https://open.bigmodel.cn/api/paas/v4/chat/completions";
const DEFAULT_MODEL: &str = "glm-4-plus";
const MAX_TOKENS_CAP: u64 = 8192;

/// Read a single-token secret from a file in the user's home dir (e.g. ~/.GLM).
pub fn read_key(filename: &str) -> Option<String> {
    let home = env::var("HOME").ok()?;
    let raw = fs::read_to_string(format!("{home}/{filename}")).ok()?;
    let key = raw.trim().to_string();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

/// `POST /api/ai/chat` — forward an OpenAI-style chat request to GLM with the
/// server-side key. Supports streaming (SSE passthrough) when the client sets
/// `stream: true`. Body is the request as-is; we only sanitize it.
pub async fn chat(Json(mut body): Json<Value>) -> Response {
    let Some(key) = read_key(".GLM") else {
        return err(
            StatusCode::SERVICE_UNAVAILABLE,
            "AI is not configured: put the GLM API key in ~/.GLM on the server.",
        );
    };
    if !body.is_object() {
        return err(StatusCode::BAD_REQUEST, "request body must be a JSON object");
    }
    let obj = body.as_object_mut().unwrap();

    // Guardrails: only GLM models, bounded output.
    let model_ok = obj
        .get("model")
        .and_then(|m| m.as_str())
        .is_some_and(|m| m.starts_with("glm"));
    if !model_ok {
        let model = env::var("GLM_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        obj.insert("model".into(), json!(model));
    }
    let max_tokens = obj.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    obj.insert(
        "max_tokens".into(),
        json!(if max_tokens == 0 { 4096 } else { max_tokens.min(MAX_TOKENS_CAP) }),
    );
    if obj.get("messages").and_then(|m| m.as_array()).is_none() {
        return err(StatusCode::BAD_REQUEST, "request needs a `messages` array");
    }
    let streaming = obj.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);

    let url = env::var("GLM_BASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
    let client = reqwest::Client::new();
    let upstream = client
        .post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await;

    let resp = match upstream {
        Ok(r) => r,
        Err(e) => return err(StatusCode::BAD_GATEWAY, &format!("failed to reach GLM API: {e}")),
    };
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    // On an upstream error (non-2xx) return the body as JSON so the client can
    // show it, whether or not streaming was requested.
    if !status.is_success() {
        let payload = resp
            .json::<Value>()
            .await
            .unwrap_or_else(|e| json!({ "error": format!("upstream error: {e}") }));
        return (status, Json(payload)).into_response();
    }

    if streaming {
        // Pipe the SSE stream straight through to the browser.
        Response::builder()
            .status(status)
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .body(Body::from_stream(resp.bytes_stream()))
            .unwrap()
    } else {
        let payload = resp
            .json::<Value>()
            .await
            .unwrap_or_else(|e| json!({ "error": format!("invalid upstream response: {e}") }));
        (status, Json(payload)).into_response()
    }
}
