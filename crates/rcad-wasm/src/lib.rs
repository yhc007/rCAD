//! # rcad-wasm
//!
//! WebAssembly bindings for rCAD.
//! Provides the JavaScript API for the web frontend.

mod api;
mod bridge;

use wasm_bindgen::prelude::*;

/// Initialize the WASM module
#[wasm_bindgen(start)]
pub fn init() {
    // Set up panic hook for better error messages
    console_error_panic_hook::set_once();

    // Initialize logging
    console_log::init_with_level(log::Level::Info).ok();

    log::info!("rCAD WASM module initialized");
}

/// Get the version of the rCAD library
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// Re-export main API types
pub use api::*;
