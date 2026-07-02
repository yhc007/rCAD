"""Show that the collider shape actually changes the physics.

Drops a body onto a tilted fixed ramp with two different colliders (sphere vs
box of the same size) and reports how far each travels down the slope. A sphere
should roll noticeably farther than a box.

Run: uv run python compare_colliders.py
"""

import math

from app import SimRequest, run_sim

# Ramp tilted ~18 deg about Z so its surface slopes along X.
a = math.radians(18)
ramp_q = [0.0, 0.0, math.sin(-a / 2), math.cos(-a / 2)]


def scene(collider_type: str) -> SimRequest:
    ball_shape = (
        {"type": "sphere", "radius": 0.4}
        if collider_type == "sphere"
        else {"type": "box", "hx": 0.4, "hy": 0.4, "hz": 0.4}
    )
    return SimRequest.model_validate(
        {
            "bodies": [
                {
                    "id": "ramp",
                    "shape": {"type": "box", "hx": 3.0, "hy": 0.1, "hz": 1.5},
                    "transform": [0, 1.0, 0, *ramp_q],
                    "fixed": True,
                },
                {
                    "id": "ball",
                    "shape": ball_shape,
                    "transform": [0.0, 2.5, 0, 0, 0, 0, 1],
                },
            ],
            "fps": 60,
            "frames": 150,
            "friction": 0.4,
        }
    )


def travel(collider_type: str) -> float:
    out = run_sim(scene(collider_type))
    bi = out.body_ids.index("ball")
    x0 = out.frames[0][bi][0]
    xf = out.frames[-1][bi][0]
    return abs(xf - x0)


sphere = travel("sphere")
box = travel("box")
print(f"sphere collider travelled: {sphere:.3f}")
print(f"box    collider travelled: {box:.3f}")
print(f"ratio (sphere/box): {sphere / max(box, 1e-6):.1f}x")
assert sphere > box * 2, "sphere should roll noticeably farther than a box"
print("\nPASS: collider shape changes the simulation (sphere rolls, box does not).")
