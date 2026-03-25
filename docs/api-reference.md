# rCAD API Reference

## Overview

This document provides API reference for all rCAD crates. The primary interface for web applications is through the `rcad-wasm` crate, which exposes JavaScript bindings.

---

## rcad-wasm (JavaScript API)

### Initialization

```javascript
import init, { CADDocument } from 'rcad-wasm';

// Initialize the WASM module
await init();

// Create a new document
const doc = new CADDocument();
```

### CADDocument

The main entry point for CAD operations.

#### Constructor

```javascript
const doc = new CADDocument();
```

Creates a new empty CAD document.

#### Primitive Creation

##### `createBox(width, height, depth)`

Creates a box primitive.

- **Parameters:**
  - `width` (number): Box width (X dimension)
  - `height` (number): Box height (Y dimension)
  - `depth` (number): Box depth (Z dimension)
- **Returns:** `string` - Feature ID

```javascript
const boxId = doc.createBox(100, 50, 25);
```

##### `createCylinder(radius, height)`

Creates a cylinder primitive.

- **Parameters:**
  - `radius` (number): Cylinder radius
  - `height` (number): Cylinder height
- **Returns:** `string` - Feature ID

```javascript
const cylinderId = doc.createCylinder(25, 100);
```

##### `createSphere(radius)`

Creates a sphere primitive.

- **Parameters:**
  - `radius` (number): Sphere radius
- **Returns:** `string` - Feature ID

```javascript
const sphereId = doc.createSphere(50);
```

##### `createCone(baseRadius, topRadius, height)`

Creates a cone or truncated cone primitive.

- **Parameters:**
  - `baseRadius` (number): Bottom radius
  - `topRadius` (number): Top radius (0 for pointed cone)
  - `height` (number): Cone height
- **Returns:** `string` - Feature ID

```javascript
const coneId = doc.createCone(50, 0, 100);
```

##### `createTorus(majorRadius, minorRadius)`

Creates a torus primitive.

- **Parameters:**
  - `majorRadius` (number): Distance from center to tube center
  - `minorRadius` (number): Tube radius
- **Returns:** `string` - Feature ID

```javascript
const torusId = doc.createTorus(50, 10);
```

#### Boolean Operations

##### `booleanUnion(targetId, toolId)`

Performs boolean union of two solids.

- **Parameters:**
  - `targetId` (string): Target feature ID
  - `toolId` (string): Tool feature ID
- **Returns:** `string` - New feature ID

```javascript
const unionId = doc.booleanUnion(boxId, cylinderId);
```

##### `booleanSubtract(targetId, toolId)`

Subtracts tool from target.

- **Parameters:**
  - `targetId` (string): Target feature ID
  - `toolId` (string): Tool feature ID (will be subtracted)
- **Returns:** `string` - New feature ID

```javascript
const holeId = doc.booleanSubtract(boxId, cylinderId);
```

##### `booleanIntersect(targetId, toolId)`

Computes intersection of two solids.

- **Parameters:**
  - `targetId` (string): First feature ID
  - `toolId` (string): Second feature ID
- **Returns:** `string` - New feature ID

```javascript
const intersectId = doc.booleanIntersect(boxId, sphereId);
```

#### Tessellation

##### `tessellate(featureId, tolerance)`

Converts B-Rep to triangle mesh for rendering.

- **Parameters:**
  - `featureId` (string): Feature to tessellate
  - `tolerance` (number): Tessellation tolerance (smaller = finer mesh)
- **Returns:** `Float32Array` - Interleaved vertex data (position, normal, uv)

```javascript
const vertices = doc.tessellate(boxId, 0.1);
// Format: [x, y, z, nx, ny, nz, u, v, ...]
```

##### `tessellateIndexed(featureId, tolerance)`

Returns separate vertex and index arrays.

- **Parameters:**
  - `featureId` (string): Feature to tessellate
  - `tolerance` (number): Tessellation tolerance
- **Returns:** `Object` - `{ vertices: Float32Array, indices: Uint32Array }`

```javascript
const { vertices, indices } = doc.tessellateIndexed(boxId, 0.1);
```

#### Export Functions

##### `exportStl(featureId, binary)`

Exports feature as STL.

- **Parameters:**
  - `featureId` (string): Feature to export
  - `binary` (boolean): If true, returns binary STL
- **Returns:** `Uint8Array` - STL file data

```javascript
const stlData = doc.exportStl(boxId, true);
```

##### `exportObj(featureId)`

Exports feature as OBJ.

- **Parameters:**
  - `featureId` (string): Feature to export
- **Returns:** `string` - OBJ file content

```javascript
const objContent = doc.exportObj(boxId);
```

##### `exportGltf(featureId)`

Exports feature as glTF 2.0 JSON.

- **Parameters:**
  - `featureId` (string): Feature to export
- **Returns:** `string` - glTF JSON

```javascript
const gltfJson = doc.exportGltf(boxId);
```

##### `exportUsd(featureId)`

Exports feature as USDA (ASCII USD).

- **Parameters:**
  - `featureId` (string): Feature to export
- **Returns:** `string` - USDA content

```javascript
const usdContent = doc.exportUsd(boxId);
```

#### Document Operations

##### `undo()`

Undoes the last operation.

- **Returns:** `boolean` - True if undo was performed

```javascript
doc.undo();
```

##### `redo()`

Redoes the previously undone operation.

- **Returns:** `boolean` - True if redo was performed

```javascript
doc.redo();
```

##### `getFeatureTree()`

Returns the feature tree structure.

- **Returns:** `Object` - Feature tree JSON

```javascript
const tree = doc.getFeatureTree();
```

##### `deleteFeature(featureId)`

Deletes a feature and its dependents.

- **Parameters:**
  - `featureId` (string): Feature to delete

```javascript
doc.deleteFeature(boxId);
```

---

## rcad-core (Rust API)

### Document

```rust
use rcad_core::{Document, Feature, FeatureId};

// Create new document
let mut doc = Document::new();

// Add a feature
let box_id = doc.add_feature(Feature::Box {
    width: 100.0,
    height: 50.0,
    depth: 25.0,
});

// Get feature by ID
if let Some(feature) = doc.get_feature(&box_id) {
    // ...
}

// Remove feature
doc.remove_feature(&box_id);
```

### Feature Types

```rust
pub enum Feature {
    // Primitives
    Box { width: f64, height: f64, depth: f64 },
    Cylinder { radius: f64, height: f64 },
    Sphere { radius: f64 },
    Cone { base_radius: f64, top_radius: f64, height: f64 },
    Torus { major_radius: f64, minor_radius: f64 },

    // Operations
    BooleanUnion { target: FeatureId, tool: FeatureId },
    BooleanSubtract { target: FeatureId, tool: FeatureId },
    BooleanIntersect { target: FeatureId, tool: FeatureId },

    // Sketch-based
    Extrude { sketch: FeatureId, distance: f64, direction: Vector3 },
    Revolve { sketch: FeatureId, axis: Axis, angle: f64 },

    // Modifications
    Fillet { target: FeatureId, edges: Vec<EdgeId>, radius: f64 },
    Chamfer { target: FeatureId, edges: Vec<EdgeId>, distance: f64 },
    Shell { target: FeatureId, faces: Vec<FaceId>, thickness: f64 },

    // Patterns
    LinearPattern { target: FeatureId, direction: Vector3, count: u32, spacing: f64 },
    CircularPattern { target: FeatureId, axis: Axis, count: u32, angle: f64 },
    Mirror { target: FeatureId, plane: Plane },
}
```

### History (Undo/Redo)

```rust
use rcad_core::History;

let mut history = History::new();

// Begin transaction
history.begin_transaction("Create Box");

// ... make changes ...

// Commit
history.commit();

// Undo
history.undo();

// Redo
history.redo();
```

### Constraints

```rust
use rcad_core::constraint::{Constraint2D, Constraint3D};

// 2D sketch constraints
let constraint = Constraint2D::Coincident {
    point1: point_id_1,
    point2: point_id_2,
};

// 3D assembly constraints
let mate = Constraint3D::Mate {
    face1: face_id_1,
    face2: face_id_2,
    offset: 0.0,
};
```

---

## rcad-geometry (Rust API)

### Primitives

```rust
use rcad_geometry::primitives::*;

// Create primitives
let box_solid = create_box(100.0, 50.0, 25.0)?;
let cylinder = create_cylinder(25.0, 100.0)?;
let sphere = create_sphere(50.0)?;
let cone = create_cone(50.0, 10.0, 100.0)?;
let torus = create_torus(50.0, 10.0)?;
```

### Boolean Operations

```rust
use rcad_geometry::boolean::*;

let result = boolean_union(&solid1, &solid2)?;
let result = boolean_subtract(&solid1, &solid2)?;
let result = boolean_intersect(&solid1, &solid2)?;
```

### Tessellation

```rust
use rcad_geometry::tessellation::{tessellate, TessellationOptions};

let options = TessellationOptions {
    tolerance: 0.1,
    angle_tolerance: 15.0_f64.to_radians(),
};

let mesh = tessellate(&solid, &options)?;

// Access mesh data
for vertex in &mesh.vertices {
    println!("Position: {:?}", vertex.position);
    println!("Normal: {:?}", vertex.normal);
}
```

### Sketching

```rust
use rcad_geometry::sketch::{Sketch, SketchEntity, SketchPlane};

let mut sketch = Sketch::new(SketchPlane::XY);

// Add entities
sketch.add_line([0.0, 0.0], [100.0, 0.0]);
sketch.add_line([100.0, 0.0], [100.0, 50.0]);
sketch.add_arc([50.0, 50.0], 50.0, 0.0, std::f64::consts::PI);
sketch.close();

// Convert to wire for extrusion
let wire = sketch.to_wire()?;
```

---

## rcad-render (Rust API)

### Renderer Setup

```rust
use rcad_render::{RenderEngine, RenderConfig};

let config = RenderConfig {
    width: 1920,
    height: 1080,
    msaa_samples: 4,
    ..Default::default()
};

let mut engine = RenderEngine::new(&surface, config).await?;
```

### Camera Control

```rust
use rcad_render::camera::{Camera, ViewPreset};

// Orbit camera
engine.camera.orbit(delta_x, delta_y);

// Pan camera
engine.camera.pan(delta_x, delta_y);

// Zoom
engine.camera.zoom(delta);

// Set view preset
engine.camera.set_view(ViewPreset::Front);
engine.camera.set_view(ViewPreset::Isometric);
```

### Mesh Management

```rust
use rcad_render::mesh::GpuMesh;

// Create mesh from vertices and indices
let mesh = GpuMesh::new(&device, &vertices, &indices, "MyMesh");

// Add to scene
engine.add_mesh(entity_id, mesh);

// Remove from scene
engine.remove_mesh(entity_id);
```

### Materials

```rust
use rcad_render::materials::{Material, MaterialLibrary};

// Create custom material
let material = Material {
    base_color: [0.8, 0.2, 0.2, 1.0],
    metallic: 0.0,
    roughness: 0.5,
    ..Default::default()
};

// Use preset
let steel = MaterialLibrary::steel();
let aluminum = MaterialLibrary::aluminum();
```

### Selection

```rust
use rcad_render::selection::SelectionManager;

// Pick at screen coordinates
if let Some(entity_id) = engine.selection.pick(x, y) {
    engine.selection.select(entity_id);
}

// Clear selection
engine.selection.clear();

// Get selected entities
let selected = engine.selection.get_selected();
```

---

## rcad-io (Rust API)

### STL Import/Export

```rust
use rcad_io::stl;

// Import STL
let mesh = stl::import("model.stl")?;

// Export binary STL
stl::export_binary(&mesh, "output.stl")?;

// Export ASCII STL
stl::export_ascii(&mesh, "output.stl")?;
```

### OBJ Import/Export

```rust
use rcad_io::obj;

let mesh = obj::import("model.obj")?;
obj::export(&mesh, "output.obj")?;
```

### glTF Export

```rust
use rcad_io::gltf;

let gltf_json = gltf::export(&mesh)?;
std::fs::write("model.gltf", gltf_json)?;
```

### USD Export

```rust
use rcad_io::usd;

let usda = usd::export(&mesh, "MyModel")?;
std::fs::write("model.usda", usda)?;
```

---

## rcad-server (REST API)

### Import Endpoints

#### POST `/api/import/step`

Import STEP file.

**Request:** `multipart/form-data`
- `file`: STEP file

**Response:**
```json
{
  "success": true,
  "geometry_id": "abc123",
  "statistics": {
    "faces": 42,
    "edges": 126,
    "vertices": 84
  }
}
```

#### POST `/api/import/iges`

Import IGES file.

**Request:** `multipart/form-data`
- `file`: IGES file

**Response:** Same as STEP

### Export Endpoints

#### POST `/api/export/step`

Export to STEP format.

**Request:**
```json
{
  "geometry_id": "abc123",
  "options": {
    "schema": "AP214"
  }
}
```

**Response:** STEP file data

#### POST `/api/export/usd`

Export to USD format.

**Request:**
```json
{
  "geometry_id": "abc123",
  "options": {
    "format": "usda"
  }
}
```

**Response:** USD file data

### Omniverse Endpoints

#### POST `/api/omniverse/connect`

Connect to Omniverse Nucleus server.

**Request:**
```json
{
  "nucleus_url": "omniverse://localhost/",
  "username": "user",
  "api_key": "key"
}
```

**Response:**
```json
{
  "success": true,
  "session_id": "session123"
}
```

#### POST `/api/omniverse/disconnect`

Disconnect from Omniverse.

**Request:**
```json
"session123"
```

#### POST `/api/omniverse/upload`

Upload USD to Nucleus.

**Request:**
```json
{
  "session_id": "session123",
  "geometry_id": "abc123",
  "nucleus_path": "/Projects/MyProject/model.usda"
}
```

#### POST `/api/omniverse/sync/start`

Start live sync session.

**Request:**
```json
{
  "session_id": "session123",
  "nucleus_path": "/Projects/MyProject/live"
}
```

#### POST `/api/omniverse/sync/stop`

Stop live sync session.

**Request:**
```json
{
  "session_id": "session123",
  "nucleus_path": "/Projects/MyProject/live"
}
```

---

## Error Handling

### JavaScript

```javascript
try {
  const boxId = doc.createBox(100, 50, 25);
} catch (error) {
  console.error('CAD operation failed:', error.message);
}
```

### Rust

```rust
use rcad_core::Error;

match doc.add_feature(feature) {
    Ok(id) => println!("Created feature: {}", id),
    Err(Error::InvalidGeometry(msg)) => eprintln!("Invalid geometry: {}", msg),
    Err(Error::OperationFailed(msg)) => eprintln!("Operation failed: {}", msg),
    Err(e) => eprintln!("Error: {}", e),
}
```

---

## Type Definitions

### JavaScript/TypeScript

```typescript
interface Vector3 {
  x: number;
  y: number;
  z: number;
}

interface BoundingBox {
  min: Vector3;
  max: Vector3;
}

interface Material {
  baseColor: [number, number, number, number];
  metallic: number;
  roughness: number;
  emissive: [number, number, number];
}

interface FeatureNode {
  id: string;
  name: string;
  type: string;
  children: FeatureNode[];
  suppressed: boolean;
}
```

### Rust

```rust
pub type FeatureId = Uuid;
pub type EntityId = u32;

pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub struct BoundingBox {
    pub min: Vector3,
    pub max: Vector3,
}
```

---

## Examples

### Creating a Simple Part

```javascript
import init, { CADDocument } from 'rcad-wasm';

async function createPart() {
  await init();
  const doc = new CADDocument();

  // Create base block
  const baseId = doc.createBox(100, 100, 20);

  // Create cylinder for hole
  const holeId = doc.createCylinder(15, 30);

  // Subtract hole from base
  const resultId = doc.booleanSubtract(baseId, holeId);

  // Tessellate for rendering
  const mesh = doc.tessellate(resultId, 0.1);

  // Export as STL
  const stlData = doc.exportStl(resultId, true);

  return { mesh, stlData };
}
```

### Setting Up the Renderer

```javascript
import { useWebGPU } from './hooks/useWebGPU';
import { useCAD } from './hooks/useCAD';

function CADCanvas() {
  const canvasRef = useRef(null);
  const { renderer, initialized } = useWebGPU(canvasRef);
  const { doc, createBox } = useCAD();

  useEffect(() => {
    if (initialized && doc) {
      const boxId = createBox(100, 50, 25);
      const mesh = doc.tessellate(boxId, 0.1);
      renderer.addMesh('box', mesh);
    }
  }, [initialized, doc]);

  return <canvas ref={canvasRef} />;
}
```

### Connecting to Omniverse

```javascript
async function connectToOmniverse(serverUrl) {
  const response = await fetch(`${serverUrl}/api/omniverse/connect`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      nucleus_url: 'omniverse://localhost/',
      username: 'user',
      api_key: 'key'
    })
  });

  const data = await response.json();
  if (data.success) {
    return data.session_id;
  }
  throw new Error(data.message);
}
```
