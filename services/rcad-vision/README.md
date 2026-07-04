# rCAD Vision

OpenCV vision-inspection microservice for the rCAD process digital twin.

`POST /inspect` takes a camera frame (base64 JPEG/PNG, data-URI prefix allowed)
and runs a real OpenCV quality check on a centre ROI:

- **red-defect ratio** — fraction of red pixels (the classic "red = bad" marker)
- **brightness** — a covered/dark camera fails
- **sharpness** (Laplacian variance) — reported as a blur metric

Returns `{ok, result: PASS|FAIL, reason, defect_ratio, sharpness, brightness,
annotated}` where `annotated` is the frame with the ROI + verdict drawn on it.

The web client (Process Twin "Inspection cam" tile) grabs a webcam frame every
~1.5 s, calls this service, and feeds the verdict back into the twin via
`/api/telemetry/ingest` (overriding the mock inspection), so the 3D part colour
and QC counts reflect the real camera.

## Run

```bash
cd services/rcad-vision
uv sync
uv run uvicorn app:app --port 8001
```

The web frontend proxies `/vision` → `http://127.0.0.1:8001`.
