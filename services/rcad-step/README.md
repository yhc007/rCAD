# rCAD STEP (OpenCASCADE)

STEP import service backed by **OpenCASCADE (OCCT)**, the reference STEP kernel.

The pure-Rust `truck-stepio` parser only covers a subset of STEP and fails on
complex commercial AP242 assemblies (the "big STEP won't open" problem). This
sidecar uses OCCT to read a STEP file into an XCAF document — preserving the
assembly structure, part names, and **colours** — tessellates it, and writes a
binary glTF (`.glb`). The rCAD server then runs that glb through its existing
glTF import pipeline, so complex STEP comes in as coloured parts with no new
client code.

- `POST /convert` — raw STEP bytes in the request body → `.glb` bytes back
  (`model/gltf-binary`), with `X-Step-Shapes` / `X-Step-Deflection` headers.
- `GET /health` — liveness + kernel name.

OCCT is Z-up; the writer converts to glTF/Y-up (rCAD is Y-up). Tessellation
deflection is relative to the model's bounding-box diagonal.

## Run

```bash
cd services/rcad-step
uv sync              # installs cadquery-ocp (OCCT 7.9 wheels; ~large, one-off)
uv run uvicorn app:app --port 8002
```

The rCAD server (`POST /api/import/step`) calls this directly over HTTP
(`STEP_SERVICE_URL`, default `http://127.0.0.1:8002/convert`) and falls back to
`truck-stepio` if it is down — so this service is optional but needed for large
/ commercial STEP files.
