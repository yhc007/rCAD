"""Smoke test for the physics service via FastAPI TestClient.

Drops a dynamic box onto a fixed platform and checks it falls and settles on
top. Run: uv run python test_service.py
"""

from fastapi.testclient import TestClient

from app import app

client = TestClient(app)


def test_health():
    r = client.get("/health")
    assert r.status_code == 200, r.text
    body = r.json()
    assert body["status"] == "ok"
    print("health:", body)


def test_drop_onto_fixed_platform():
    scene = {
        "bodies": [
            # fixed platform: top surface at y = 0.5
            {
                "id": "platform",
                "shape": {"type": "box", "hx": 2.0, "hy": 0.25, "hz": 2.0},
                "transform": [0, 0.25, 0, 0, 0, 0, 1],
                "fixed": True,
            },
            # dynamic box dropped from y = 4
            {
                "id": "box",
                "shape": {"type": "box", "hx": 0.5, "hy": 0.5, "hz": 0.5},
                "transform": [0, 4, 0, 0, 0, 0, 1],
            },
        ],
        "fps": 60,
        "frames": 120,
        "substeps": 10,
    }
    r = client.post("/simulate", json=scene)
    assert r.status_code == 200, r.text
    d = r.json()

    assert d["body_ids"] == ["platform", "box"]
    bi = d["body_ids"].index("box")
    pi = d["body_ids"].index("platform")

    y_box_start = d["frames"][0][bi][1]
    y_box_end = d["frames"][-1][bi][1]
    y_platform_end = d["frames"][-1][pi][1]

    print(f"box y: {y_box_start:.3f} -> {y_box_end:.3f}  (expected rest ~1.0)")
    print(f"platform y stayed at: {y_platform_end:.3f} (fixed)")
    print(f"frames returned: {len(d['frames'])}, bodies/frame: {len(d['frames'][0])}")

    assert y_box_start > 3.5, "box did not start elevated"
    assert y_box_end < y_box_start, "box did not fall"
    assert 0.8 < y_box_end < 1.2, f"box did not settle on platform (got {y_box_end:.3f})"
    assert abs(y_platform_end - 0.25) < 1e-3, "fixed platform moved"
    print("\nPASS: dynamic box fell onto fixed platform; per-frame transforms returned.")


if __name__ == "__main__":
    test_health()
    test_drop_onto_fixed_platform()
