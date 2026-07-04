"""OpenCV vision-inspection microservice for the rCAD process twin.

Two camera sources feed the same inspection:

- **Browser webcam** — the client captures a frame and POSTs it to `/inspect`
  (base64). Used by the Process Twin webcam tile.
- **Industrial camera (RTSP / MJPEG URL)** — the service itself opens the stream
  with OpenCV (`POST /camera {url}`), grabs frames in a background thread, and
  serves them back: `/inspect_source` inspects the current frame, `/snapshot`
  is the latest JPEG, and `/stream` re-encodes it as same-origin MJPEG so the
  browser can preview an RTSP camera it could never play directly.

The inspection itself (a centre ROI: red-defect ratio + brightness) is shared by
both, returning a PASS/FAIL verdict the twin routes on.
"""

import base64
import os
import threading
import time

import cv2
import numpy as np
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import Response, StreamingResponse
from pydantic import BaseModel

# Prefer TCP for RTSP (more reliable than the default UDP) before any capture.
os.environ.setdefault("OPENCV_FFMPEG_CAPTURE_OPTIONS", "rtsp_transport;tcp")

app = FastAPI(title="rCAD Vision")
app.add_middleware(
    CORSMiddleware, allow_origins=["*"], allow_methods=["*"], allow_headers=["*"]
)

# Decision thresholds.
DEFECT_RATIO = 0.08  # fraction of red pixels in the ROI to call a defect
MIN_BRIGHTNESS = 25.0  # a covered/dark camera fails


def inspect_bgr(img):
    """Run the quality check on a BGR frame; returns the verdict dict."""
    h, w = img.shape[:2]
    x0, y0 = int(w * 0.25), int(h * 0.25)
    x1, y1 = int(w * 0.75), int(h * 0.75)
    roi = img[y0:y1, x0:x1]

    # Red-defect ratio (two hue ranges wrap around 0/180 in HSV).
    hsv = cv2.cvtColor(roi, cv2.COLOR_BGR2HSV)
    m1 = cv2.inRange(hsv, (0, 80, 60), (10, 255, 255))
    m2 = cv2.inRange(hsv, (170, 80, 60), (180, 255, 255))
    red = int(cv2.countNonZero(cv2.bitwise_or(m1, m2)))
    total = roi.shape[0] * roi.shape[1]
    defect_ratio = red / total if total else 0.0

    gray = cv2.cvtColor(roi, cv2.COLOR_BGR2GRAY)
    sharpness = float(cv2.Laplacian(gray, cv2.CV_64F).var())
    brightness = float(gray.mean())

    result, reason = "PASS", "ok"
    if defect_ratio > DEFECT_RATIO:
        result, reason = "FAIL", "red defect detected"
    elif brightness < MIN_BRIGHTNESS:
        result, reason = "FAIL", "too dark"
    # sharpness is reported (blur metric) but not used to fail flat frames

    # Annotated frame for the UI overlay.
    ann = img.copy()
    color = (0, 200, 0) if result == "PASS" else (0, 0, 220)
    cv2.rectangle(ann, (x0, y0), (x1, y1), color, 2)
    cv2.putText(
        ann,
        f"{result} {defect_ratio * 100:.0f}%",
        (x0, max(y0 - 8, 16)),
        cv2.FONT_HERSHEY_SIMPLEX,
        0.6,
        color,
        2,
    )
    ok, buf = cv2.imencode(".jpg", ann, [cv2.IMWRITE_JPEG_QUALITY, 70])
    annotated = (
        "data:image/jpeg;base64," + base64.b64encode(buf).decode() if ok else None
    )

    return {
        "ok": True,
        "result": result,
        "reason": reason,
        "defect_ratio": round(defect_ratio, 3),
        "sharpness": round(sharpness, 1),
        "brightness": round(brightness, 1),
        "annotated": annotated,
    }


class Camera:
    """Background grabber for an industrial camera (RTSP / MJPEG / device URL).

    Opens the stream with OpenCV and keeps only the latest frame, reconnecting
    on failure. A URL of None disconnects.
    """

    def __init__(self):
        self.url = None
        self._frame = None
        self._lock = threading.Lock()
        self.connected = False
        self.error = None
        self._thread = None

    def set_url(self, url):
        with self._lock:
            self.url = url
            self._frame = None
            self.connected = False
            self.error = None if url else "disconnected"
        if url and self._thread is None:
            self._thread = threading.Thread(target=self._loop, daemon=True)
            self._thread.start()

    def _loop(self):
        cap = None
        cur = None
        while True:
            url = self.url
            if not url:
                if cap is not None:
                    cap.release()
                    cap = None
                    cur = None
                time.sleep(0.2)
                continue
            if cap is None or cur != url:
                if cap is not None:
                    cap.release()
                cur = url
                cap = cv2.VideoCapture(url)
                try:
                    cap.set(cv2.CAP_PROP_BUFFERSIZE, 1)  # keep only the freshest frame
                except cv2.error:
                    pass
                if not cap.isOpened():
                    with self._lock:
                        self.connected = False
                        self.error = f"cannot open {url}"
                    cap.release()
                    cap = None
                    time.sleep(1.0)
                    continue
            ok, frame = cap.read()
            if not ok or frame is None:
                with self._lock:
                    self.connected = False
                    self.error = "stream read failed"
                cap.release()
                cap = None
                time.sleep(0.5)
                continue
            with self._lock:
                self._frame = frame
                self.connected = True
                self.error = None
            time.sleep(0.03)  # ~30 fps ceiling

    def latest(self):
        with self._lock:
            return None if self._frame is None else self._frame.copy()


camera = Camera()


class InspectRequest(BaseModel):
    image: str  # base64-encoded JPEG/PNG (data URI prefix allowed)


class CameraRequest(BaseModel):
    url: str | None = None  # RTSP/MJPEG/device URL; null or empty disconnects


@app.get("/health")
def health():
    return {"status": "ok", "opencv": cv2.__version__}


@app.post("/inspect")
def inspect(req: InspectRequest):
    """Inspect a frame the browser captured (base64)."""
    data = req.image.split(",", 1)[-1]  # strip any data URI prefix
    try:
        raw = base64.b64decode(data)
        arr = np.frombuffer(raw, np.uint8)
        img = cv2.imdecode(arr, cv2.IMREAD_COLOR)
    except Exception as e:  # noqa: BLE001
        return {"ok": False, "error": f"decode failed: {e}"}
    if img is None:
        return {"ok": False, "error": "could not decode image"}
    return inspect_bgr(img)


@app.post("/camera")
def set_camera(req: CameraRequest):
    """Point the service at an industrial camera stream (or disconnect)."""
    url = (req.url or "").strip() or None
    camera.set_url(url)
    return {"ok": True, "url": camera.url}


@app.get("/camera")
def camera_status():
    return {"url": camera.url, "connected": camera.connected, "error": camera.error}


@app.post("/inspect_source")
def inspect_source():
    """Inspect the current frame from the configured industrial camera."""
    img = camera.latest()
    if img is None:
        return {"ok": False, "error": camera.error or "no camera frame yet"}
    return inspect_bgr(img)


@app.get("/snapshot")
def snapshot():
    img = camera.latest()
    if img is None:
        return Response(status_code=503)
    ok, buf = cv2.imencode(".jpg", img, [cv2.IMWRITE_JPEG_QUALITY, 75])
    if not ok:
        return Response(status_code=503)
    return Response(content=buf.tobytes(), media_type="image/jpeg")


@app.get("/stream")
def stream():
    """Re-encode the industrial camera as same-origin MJPEG for the browser."""

    def gen():
        boundary = b"--frame\r\n"
        while True:
            img = camera.latest()
            if img is not None:
                ok, buf = cv2.imencode(".jpg", img, [cv2.IMWRITE_JPEG_QUALITY, 70])
                if ok:
                    yield boundary + b"Content-Type: image/jpeg\r\n\r\n" + buf.tobytes() + b"\r\n"
            time.sleep(0.05)

    return StreamingResponse(
        gen(), media_type="multipart/x-mixed-replace; boundary=frame"
    )
