//! rCAD Backend Server
//!
//! Axum-based server providing:
//! - STEP/IGES import/export (via OpenCASCADE)
//! - Omniverse synchronization
//! - Heavy computation offloading

mod api;
mod process;
mod services;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rcad_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Build application
    let app = create_router();

    // Run server - try port from env or default to 3001
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("rCAD Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await
        .expect(&format!("Failed to bind to port {}", port));
    axum::serve(listener, app).await.unwrap();
}

/// Create the application router
pub fn create_router() -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Telemetry bus + mock process simulation (digital-twin stage).
    let (tx, _rx) = tokio::sync::broadcast::channel::<String>(64);
    let state = process::AppState {
        tx,
        ctrl: std::sync::Arc::new(std::sync::Mutex::new(process::Control::default())),
        latest: std::sync::Arc::new(std::sync::Mutex::new(String::from("{}"))),
        overrides: std::sync::Arc::new(std::sync::Mutex::new(serde_json::Map::new())),
        rules: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        graph: std::sync::Arc::new(std::sync::Mutex::new(process::default_graph())),
        vision_armed: std::sync::Arc::new(std::sync::Mutex::new(false)),
        verdicts: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    };
    process::spawn_sim(state.clone());
    process::spawn_mqtt(state.clone()); // real-hardware bridge when MQTT_BROKER is set

    Router::new()
        // Health check
        .route("/health", get(health_check))
        // API routes
        .nest("/api", api_routes())
        // Provide the telemetry bus + control to the handlers, then erase state.
        .with_state(state)
        // Allow large uploads — STEP assemblies routinely run tens of MB, well
        // past Axum's 2 MB default (which silently rejected big imports).
        .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}

/// API routes (state = telemetry bus + control; most handlers ignore it).
fn api_routes() -> Router<process::AppState> {
    Router::new()
        // Process digital-twin telemetry stream (mock sim → tags over WebSocket)
        .route("/telemetry/ws", get(process::telemetry_ws))
        // Supervisory line control (start/stop/reset/config) + status
        .route("/telemetry/control", post(process::control))
        .route("/telemetry/status", get(process::status))
        // External tag ingress (real-hardware source over HTTP; MQTT feeds the same path)
        .route("/telemetry/ingest", post(process::ingest))
        // Synced vision capture: arm a camera + report a per-part inspection verdict
        .route("/telemetry/inspect", post(process::inspect_report))
        // Authorable automation rules (condition → action)
        .route("/telemetry/rules", get(process::get_rules).post(process::set_rules))
        // Authorable process flow graph (multi-step: source→process→inspect→sink)
        .route("/telemetry/graph", get(process::get_graph).post(process::set_graph))
        // AI copilot (proxies the GLM chat API; key stays server-side)
        .route("/ai/chat", post(api::ai::chat))
        // Tripo text-to-3D generation → merged mesh
        .route("/tripo/generate", post(api::tripo::generate))
        // Import endpoints
        .route("/import/step", post(api::import::import_step))
        .route("/import/gltf", post(api::import::import_gltf))
        .route("/import/iges", post(api::import::import_iges))
        .route("/import/upload", post(api::import::upload_file))
        // Export endpoints
        .route("/export/step", post(api::export::export_step))
        .route("/export/iges", post(api::export::export_iges))
        .route("/export/stl", post(api::export::export_stl))
        .route("/export/gltf", post(api::export::export_gltf))
        .route("/export/usd", post(api::export::export_usd))
        // Omniverse endpoints
        .route("/omniverse/connect", post(api::omniverse::connect))
        .route("/omniverse/disconnect", post(api::omniverse::disconnect))
        .route("/omniverse/upload", post(api::omniverse::upload_to_nucleus))
        .route("/omniverse/sync/start", post(api::omniverse::start_live_sync))
        .route("/omniverse/sync/stop", post(api::omniverse::stop_live_sync))
        // Geometry operations
        .route("/geometry/boolean", post(geometry_boolean))
        .route("/geometry/tessellate", post(geometry_tessellate))
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Boolean operation endpoint
async fn geometry_boolean() -> &'static str {
    // Placeholder - would perform boolean operations using OpenCASCADE
    "Boolean operation endpoint"
}

/// Tessellation endpoint
async fn geometry_tessellate() -> &'static str {
    // Placeholder - would tessellate geometry
    "Tessellate endpoint"
}
