# rCAD Architecture Documentation

## Overview

rCAD is a production-grade mechanical engineering CAD application built with modern web technologies:

- **Rust** compiled to **WebAssembly** for core CAD logic
- **WebGPU** (via wgpu) for high-performance 3D rendering
- **truck** B-Rep library for geometric modeling
- **React/TypeScript** for the web user interface
- **NVIDIA Omniverse** connector for collaboration and rendering

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Web Browser                               │
├─────────────────────────────────────────────────────────────────┤
│  React/TypeScript UI Layer                                       │
│  ├── Command Palette, Toolbars, Property Panels                 │
│  ├── Document Tree / Model Browser                              │
│  └── WebGPU Canvas Integration                                  │
├─────────────────────────────────────────────────────────────────┤
│  JavaScript ↔ WASM Bridge (wasm-bindgen)                        │
├─────────────────────────────────────────────────────────────────┤
│  Rust/WASM Core (truck B-Rep)                                    │
│  ├── Rendering Engine (wgpu)                                    │
│  ├── Geometry Kernel (truck - pure Rust B-Rep)                  │
│  ├── Real-time modeling operations                              │
│  ├── Constraint Solver                                          │
│  └── Feature Tree / Parametric History                          │
└──────────────────────────┬──────────────────────────────────────┘
                           │ REST/WebSocket API
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│  Rust Backend Server (Axum)                                      │
│  ├── OpenCASCADE (STEP/IGES import/export)                      │
│  ├── Omniverse Connect SDK                                      │
│  ├── USD generation and Nucleus sync                            │
│  └── Heavy computation offloading                               │
└─────────────────────────────────────────────────────────────────┘
```

## Crate Structure

### rcad-core
Core CAD data structures including:
- **Document model**: Complete CAD document representation
- **Feature system**: Parametric features (primitives, booleans, etc.)
- **Constraints**: Geometric and dimensional constraints
- **History**: Undo/redo system with command pattern

### rcad-geometry
Geometry kernel using the truck B-Rep library:
- **Primitives**: Box, cylinder, sphere, cone, torus
- **Boolean operations**: Union, subtract, intersect
- **Sketching**: 2D sketch creation and manipulation
- **Tessellation**: B-Rep to triangle mesh conversion
- **Fillet/Chamfer**: Edge blending operations

### rcad-render
WebGPU rendering engine:
- **PBR rendering**: Physically-based materials
- **Camera controls**: Orbit, pan, zoom with presets
- **Selection**: GPU-based picking system
- **Grid/Axes**: Visual aids for modeling

### rcad-io
File format support:
- **STL**: ASCII and binary import/export
- **OBJ**: Wavefront OBJ format
- **glTF**: glTF 2.0 with materials
- **USD**: Universal Scene Description for Omniverse
- **STEP/IGES**: Via server-side OpenCASCADE

### rcad-omniverse
NVIDIA Omniverse integration:
- **Connector**: Omniverse Connect SDK wrapper
- **Live Sync**: Real-time synchronization
- **Nucleus**: Server connection management

### rcad-server
Backend server (Axum):
- **Import API**: STEP/IGES file processing
- **Export API**: Multi-format export
- **Omniverse API**: Nucleus synchronization

### rcad-wasm
WebAssembly bindings:
- **wasm-bindgen** exports for JavaScript
- **Type conversions**: JS ↔ Rust bridge
- **Public API**: Document, renderer, operations

## Data Flow

### Modeling Operations

```
User Action → React UI → WASM API → rcad-core
                                      ↓
                              rcad-geometry (truck)
                                      ↓
                              rcad-render (tessellation)
                                      ↓
                              WebGPU Display
```

### File Import (STEP)

```
Upload → Server → OpenCASCADE Parse
                         ↓
                  Convert to truck B-Rep
                         ↓
                  Send to WASM client
                         ↓
                  rcad-core Document
```

### Omniverse Sync

```
Document Change → rcad-omniverse
                       ↓
                 Export to USD
                       ↓
                 Nucleus Upload
                       ↓
                 Omniverse Apps
```

## Key Design Decisions

### 1. Hybrid Architecture
- Client-side (WASM): Real-time modeling with truck
- Server-side (Native): Heavy operations with OpenCASCADE

### 2. Pure Rust Geometry Kernel
- truck compiles cleanly to WASM
- No C++ dependencies in the browser
- OpenCASCADE only for STEP/IGES on server

### 3. WebGPU Rendering
- Native-quality graphics in the browser
- Compute shaders for future acceleration
- Cross-platform compatibility

### 4. Parametric Feature Tree
- Each operation is a reversible feature
- Full undo/redo support
- Parameters can be linked to expressions

## Performance Considerations

### WASM Optimization
- Release builds with LTO enabled
- wasm-opt for size reduction
- Lazy loading of heavy modules

### Rendering
- Instance rendering for repeated geometry
- Level-of-detail for distant objects
- Frustum culling

### Memory
- Rust ownership prevents leaks
- SharedArrayBuffer for large transfers
- Incremental tessellation

## Security

- No eval() or dynamic code execution
- Sandboxed file operations
- Server validates all inputs
- CORS properly configured

## Future Extensions

1. **Multi-user collaboration** via Omniverse
2. **Constraint solver** for 2D sketches
3. **Assembly support** with mates
4. **2D drawings** generation
5. **PMI** (Product Manufacturing Information)
