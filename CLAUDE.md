# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

rCAD is a mechanical-engineering CAD application. Core CAD logic is written in Rust and compiled two ways: to **WebAssembly** for in-browser real-time modeling, and to a **native Axum server** for heavy operations (STEP/IGES via OpenCASCADE, Omniverse sync). The frontend is React/TypeScript + Vite, rendering through **WebGPU** (`wgpu`). Geometry uses the pure-Rust **truck** B-Rep kernel (no C++ in the browser).

## Build & run

The Rust workspace and the web frontend build separately, and the WASM crate bridges them.

```bash
# Rust workspace (all native crates)
cargo build                      # debug
cargo build --release            # LTO-optimized
cargo test                       # all crates
cargo test -p rcad-geometry      # one crate
cargo test -p rcad-core constraint::tests::name   # one test

# Backend server (port from $PORT, default 3001)
cargo run -p rcad-server

# WASM build -> output MUST land in web/src/wasm/pkg/ (loader imports ./pkg/rcad_wasm.js)
wasm-pack build crates/rcad-wasm --target web --out-dir ../../web/src/wasm/pkg
# size-optimized: add `--release` and use the `release-wasm` profile

# Web frontend (Vite dev server on :3001, proxies /api -> :3000)
cd web
npm install
npm run dev
npm run build                    # tsc typecheck + vite build
npm run lint                     # eslint, zero-warnings enforced
```

Note the port wiring: `rcad-server` defaults to `PORT=3001`, but `web/vite.config.ts` proxies `/api` to `localhost:3000` and itself runs on `3001`. When running both locally, set `PORT=3000 cargo run -p rcad-server` so the proxy target matches.

## Architecture

Dependency direction across the seven workspace crates (`crates/`):

```
rcad-core  ──>  rcad-geometry  ──>  rcad-io / rcad-render / rcad-omniverse
   (base data)     (truck kernel)         (formats / wgpu / nucleus)
                                                 │
                          rcad-server (native)   ├── rcad-wasm (browser)
```

- **rcad-core** — the shared data model with no geometry dependency: `Document`, `Feature` (parametric ops), `Constraint`, and `History` (command-pattern undo/redo). Every other crate builds on these types. Has a `wasm` feature flag.
- **rcad-geometry** — wraps the `truck-*` B-Rep crates. Primitives, boolean ops (`boolean.rs`), sketches, fillet/chamfer, and `tessellation.rs` (B-Rep → triangle mesh, the bridge to rendering).
- **rcad-render** — `wgpu` rendering engine. PBR (`shaders/pbr.wgsl`), GPU picking (`picking.wgsl` + `selection.rs`), camera, grid. Compiles natively and to wasm32 (target-gated `web-sys` deps).
- **rcad-io** — file formats: STL/OBJ/glTF/USD client-side; STEP via optional `step` feature (server only).
- **rcad-omniverse** — NVIDIA Omniverse: Nucleus connection, USD export, WebSocket live sync. Server-side only.
- **rcad-wasm** (`cdylib`) — the single JS entry point. Re-exports `api.rs` (the `CADDocument` / `RenderContext` classes JS calls) over `bridge.rs` type conversions. Pulls every other crate in with its `wasm` feature enabled.
- **rcad-server** — Axum REST API (`crates/rcad-server/src/api/`): `/api/import/*`, `/api/export/*`, `/api/omniverse/*`. Some geometry endpoints in `main.rs` are still placeholders.

### The WASM boundary is the key architectural seam

The browser never imports Rust crates directly — everything flows through `rcad-wasm`'s public API. When you add a CAD operation that the UI needs:

1. Implement it in `rcad-core`/`rcad-geometry`.
2. Expose a `#[wasm_bindgen]` method in `crates/rcad-wasm/src/api.rs`.
3. Add the matching method to the `CADDocument`/`RenderContext` TS interface in `web/src/wasm/loader.ts`.
4. Rebuild WASM into `web/src/wasm/pkg/` (see build commands) — the web build will not pick up Rust changes otherwise.

### Web frontend

`web/src/` — React 18 + Zustand (`stores/documentStore.ts`) + Radix UI + Tailwind. WASM is loaded once via `wasm/loader.ts`; `hooks/useCAD.ts` and `hooks/useWebGPU.ts` wrap it for components (`Canvas`, `Toolbar`, `CommandPalette`, `ModelTree`, `PropertyPanel`). Vite needs `vite-plugin-wasm` + `vite-plugin-top-level-await` and excludes `rcad-wasm` from dep optimization — preserve these when touching `vite.config.ts`.

## Conventions

- Two render paths exist: native WebGPU and a WebGL fallback (see commit history) — geometry/render changes should account for both backends.
- Errors use `thiserror` in libraries, `anyhow` at binary boundaries.
- `pkg/`, `**/pkg/`, and `*.wasm` are gitignored — the WASM build artifact in `web/src/wasm/pkg/` is generated, never hand-edited.
- Docs in `docs/architecture.md` and `docs/api-reference.md` describe intended design; treat them as the spec, but verify against code since some server endpoints are still stubs.
