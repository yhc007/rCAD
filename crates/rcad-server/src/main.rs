//! rCAD Backend Server
//!
//! Axum-based server providing:
//! - STEP/IGES import/export (via OpenCASCADE)
//! - Omniverse synchronization
//! - Heavy computation offloading

mod api;
mod services;

use axum::{
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

    // Run server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("rCAD Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Create the application router
pub fn create_router() -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(health_check))
        // API routes
        .nest("/api", api_routes())
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}

/// API routes
fn api_routes() -> Router {
    Router::new()
        // Import endpoints
        .route("/import/step", post(api::import::import_step))
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
