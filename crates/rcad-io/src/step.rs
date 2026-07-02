//! STEP file format support (requires `step` feature)
//!
//! Imports STEP (AP203/AP214/AP242) B-Rep into renderable triangle meshes using
//! the pure-Rust truck-stepio parser + truck-meshalgo tessellation. Runs
//! server-side only (the `step` feature pulls native-only deps).

use crate::{IoError, Result};
use rcad_geometry::Mesh;
use std::path::Path;

use truck_meshalgo::tessellation::{MeshableShape, MeshedShape};
use truck_stepio::r#in::Table;

/// Tessellation chord tolerance for imported STEP surfaces.
const STEP_TOLERANCE: f64 = 0.01;

/// Import a STEP file from disk into one mesh per shell.
pub fn import<P: AsRef<Path>>(path: P) -> Result<Vec<Mesh>> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(IoError::FileNotFound(path.display().to_string()));
    }
    let data = std::fs::read(path)?;
    import_from_bytes(&data)
}

/// Import STEP from raw bytes into one mesh per shell (for server use).
pub fn import_from_bytes(data: &[u8]) -> Result<Vec<Mesh>> {
    let step_str = std::str::from_utf8(data)
        .map_err(|e| IoError::ParseError(format!("STEP is not valid UTF-8: {e}")))?;

    let table = Table::from_step(step_str)
        .ok_or_else(|| IoError::ParseError("failed to parse STEP file".to_string()))?;

    let mut meshes = Vec::new();
    let mut skipped = 0usize;
    for shell_holder in table.shell.values() {
        // STEP shell -> truck compressed shell -> tessellated mesh.
        //
        // We tessellate the *compressed* shell directly rather than going through
        // `Shell::extract`: that step rebuilds a strict B-Rep topology and rejects
        // the degenerate edges (zero-length / coincident vertices) that real
        // CAD-exported STEP routinely contains ("two same vertices cannot
        // construct an edge"). A bad shell is skipped, not fatal, so the rest of
        // the assembly still imports.
        let cshell = match table.to_compressed_shell(shell_holder) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("STEP import: skipping shell (conversion failed): {e}");
                skipped += 1;
                continue;
            }
        };

        // truck's tessellation can panic outright on degenerate faces; rayon
        // re-raises worker panics on the caller, so catch_unwind makes a single
        // bad shell skippable instead of taking down the whole request.
        let polymesh = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cshell.triangulation(STEP_TOLERANCE).to_polygon()
        }));
        let polymesh = match polymesh {
            Ok(p) => p,
            Err(_) => {
                eprintln!("STEP import: skipping shell (tessellation panicked)");
                skipped += 1;
                continue;
            }
        };
        match rcad_geometry::tessellation::polymesh_to_mesh(&polymesh) {
            Ok(mesh) if mesh.vertex_count() > 0 => meshes.push(mesh),
            Ok(_) => skipped += 1,
            Err(e) => {
                eprintln!("STEP import: skipping shell (tessellation failed): {e:?}");
                skipped += 1;
            }
        }
    }

    if skipped > 0 {
        eprintln!("STEP import: {} shell(s) skipped", skipped);
    }
    if meshes.is_empty() {
        return Err(IoError::ParseError(
            "STEP file contained no tessellatable shells".to_string(),
        ));
    }
    Ok(meshes)
}

/// Export to STEP — not implemented (truck-stepio output path not wired).
pub fn export_to_bytes(_meshes: &[&Mesh]) -> Result<Vec<u8>> {
    Err(IoError::UnsupportedFeature(
        "STEP export is not implemented".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use truck_stepio::out::{CompleteStepDisplay, StepModel};

    #[test]
    fn step_roundtrip_box() {
        // Build a box solid, write it to STEP, then import it back.
        let solid = rcad_geometry::primitives::create_box(10.0, 10.0, 10.0).unwrap();
        let compressed = solid.inner.compress();
        let step = CompleteStepDisplay::new(
            StepModel::from(&compressed),
            Default::default(),
        )
        .to_string();

        let meshes = import_from_bytes(step.as_bytes()).expect("STEP import failed");
        assert!(!meshes.is_empty(), "no meshes parsed from STEP");
        let total: usize = meshes.iter().map(|m| m.triangle_count()).sum();
        assert!(total > 0, "box STEP tessellated to zero triangles");
    }

    #[test]
    #[ignore = "writes /tmp/box.step fixture for manual endpoint testing"]
    fn dump_box_step() {
        let solid = rcad_geometry::primitives::create_box(40.0, 40.0, 40.0).unwrap();
        let compressed = solid.inner.compress();
        let step =
            CompleteStepDisplay::new(StepModel::from(&compressed), Default::default()).to_string();
        std::fs::write("/tmp/box.step", step).unwrap();
    }
}
