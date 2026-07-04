# rCAD Vision

OpenCV vision-inspection microservice for the rCAD process digital twin.

`POST /inspect` takes a camera frame (base64 JPEG/PNG, data-URI prefix allowed)
and runs a real OpenCV quality check on a centre ROI:

- **red-defect ratio** — fraction of red pixels (the classic "red = bad" marker)
- **brightness** — a covered/dark camera fails
- **sharpness** (Laplacian variance) — reported as a blur metric

Returns `{ok, result: PASS|FAIL, reason, defect_ratio, sharpness, brightness,
annotated}` where `annotated` is the frame with the ROI + verdict drawn on it.

## Two camera sources

- **Browser webcam** — the client captures a frame and POSTs `/inspect` (base64).
- **Industrial camera (RTSP / MJPEG URL)** — the browser can't play RTSP, so the
  service opens the stream itself with OpenCV:
  - `POST /camera {url}` — connect to an `rtsp://…` or `http://…/mjpeg` stream
    (or `{url:null}` to disconnect); a background thread keeps the latest frame.
  - `GET /camera` — `{url, connected, error}`.
  - `POST /inspect_source` — inspect the current industrial-camera frame.
  - `GET /snapshot` — latest frame as JPEG.
  - `GET /stream` — the camera re-encoded as same-origin MJPEG, so the browser
    can preview an RTSP camera it could never play directly.

  RTSP uses TCP transport (`OPENCV_FFMPEG_CAPTURE_OPTIONS=rtsp_transport;tcp`).

The web client (Process Twin "Inspection cam" tile) picks a source (webcam or an
RTSP/MJPEG URL), inspects a frame every ~1.5 s, and feeds the verdict back into
the twin via `/api/telemetry/ingest`, so the 3D part colour, QC counts, and the
inspect-station routing (pack vs reject) reflect the real camera.

## Run

```bash
cd services/rcad-vision
uv sync
uv run uvicorn app:app --port 8001
```

The web frontend proxies `/vision` → `http://127.0.0.1:8001`.
