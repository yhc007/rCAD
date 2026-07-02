# rcad-newton

Newton physics microservice for rCAD. Precomputes rigid-body simulations on CPU
(macOS) and returns per-frame transforms for the rCAD viewer to play back.

- Runs Newton in **Y-up** so coordinates match rCAD directly (no axis conversion).
- macOS is **CPU-only** (Newton/Warp); first request compiles & caches kernels.

## Run

```bash
uv run uvicorn app:app --port 8000     # serve
uv run python test_service.py          # smoke test (TestClient)
uv run python falling_box.py           # Phase 0 spike
```

## API

- `GET /health` → `{status, newton, warp, device}`
- `POST /simulate` → body:
  ```json
  {
    "bodies": [
      {"id": "...", "shape": {"type": "box", "hx": 0.5, "hy": 0.5, "hz": 0.5},
       "transform": [px,py,pz, qx,qy,qz,qw], "mass": 0, "fixed": false}
    ],
    "gravity": -9.81, "fps": 60, "frames": 150, "substeps": 10, "solver": "xpbd"
  }
  ```
  returns `{fps, body_ids, frames}` where `frames[f][b] = [px,py,pz,qx,qy,qz,qw]`
  for `body_ids[b]` in Y-up world coords.

Shapes: `box` (hx,hy,hz) · `sphere` (radius) · `cylinder` (radius, half_height) ·
`mesh` (positions[], indices[]). `fixed: true` → static kinematic collider.
