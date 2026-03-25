//! STEP file format support (requires `step` feature)
//!
//! This module provides STEP (AP203/AP214/AP242) import/export using truck-stepio.
//! Note: This is a placeholder implementation - full STEP support requires
//! additional work to properly interface with truck-stepio's API.

use crate::{IoError, Result};
use rcad_geometry::Solid;
use std::path::Path;

/// Import a STEP file
///
/// Note: Full implementation requires proper truck-stepio integration
pub fn import<P: AsRef<Path>>(path: P) -> Result<Vec<Solid>> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(IoError::FileNotFound(path.display().to_string()));
    }

    // TODO: Implement proper STEP import using truck-stepio
    // The truck-stepio API requires specific shape type handling
    Err(IoError::UnsupportedFeature(
        "STEP import not yet implemented - requires truck-stepio integration".to_string()
    ))
}

/// Export to STEP format
///
/// Note: Full implementation requires proper truck-stepio integration
pub fn export<P: AsRef<Path>>(_path: P, _solids: &[&Solid]) -> Result<()> {
    // TODO: Implement proper STEP export using truck-stepio
    Err(IoError::UnsupportedFeature(
        "STEP export not yet implemented - requires truck-stepio integration".to_string()
    ))
}

/// Import from STEP bytes (for WASM/server use)
pub fn import_from_bytes(_data: &[u8]) -> Result<Vec<Solid>> {
    Err(IoError::UnsupportedFeature(
        "STEP import not yet implemented - requires truck-stepio integration".to_string()
    ))
}

/// Export to STEP bytes
pub fn export_to_bytes(_solids: &[&Solid]) -> Result<Vec<u8>> {
    Err(IoError::UnsupportedFeature(
        "STEP export not yet implemented - requires truck-stepio integration".to_string()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_module_exists() {
        // Basic test that module compiles
        assert!(true);
    }
}
