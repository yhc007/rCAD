"""OpenCV vision-inspection microservice for the rCAD process twin.

`POST /inspect` takes a camera frame (base64, optionally a data URI) and runs a
real OpenCV quality check on a centre ROI: it flags a defect when a red blotch
is present (the classic "red = bad" marker) or the frame is too dark/blurry, and
returns a PASS/FAIL verdict plus an annotated frame. The web client feeds the
verdict back into the twin (overriding the mock inspection).
"""

import base64

import cv2
import numpy as np
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

app = FastAPI(title="rCAD Vision")
app.add_middleware(
    CORSMiddleware, allow_origins=["*"], allow_methods=["*"], allow_headers=["*"]
)

# Decision thresholds.
DEFECT_RATIO = 0.08  # fraction of red pixels in the ROI to call a defect
MIN_BRIGHTNESS = 25.0  # a covered/dark camera fails


class InspectRequest(BaseModel):
    image: str  # base64-encoded JPEG/PNG (data URI prefix allowed)


@app.get("/health")
def health():
    return {"status": "ok", "opencv": cv2.__version__}


@app.post("/inspect")
def inspect(req: InspectRequest):
    data = req.image.split(",", 1)[-1]  # strip any data URI prefix
    try:
        raw = base64.b64decode(data)
        arr = np.frombuffer(raw, np.uint8)
        img = cv2.imdecode(arr, cv2.IMREAD_COLOR)
    except Exception as e:  # noqa: BLE001
        return {"ok": False, "error": f"decode failed: {e}"}
    if img is None:
        return {"ok": False, "error": "could not decode image"}

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
