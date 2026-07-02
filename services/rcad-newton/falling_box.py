"""Phase 0 spike: headless rigid-body sim on CPU.

Drops a box onto a ground plane with Newton's XPBD solver and extracts the
per-frame rigid transform (position + quaternion). Verifies that:
  1. Newton/Warp run on macOS CPU,
  2. a rigid body falls under gravity and settles,
  3. we can pull a frame-by-frame trajectory out (what the rCAD viewer will play).

Newton uses a Z-up world; a box of half-extent hz rests at z == hz.
Run: uv run python falling_box.py
"""

import time

import numpy as np
import warp as wp

import newton

FPS = 60
SUBSTEPS = 10
FRAMES = 150  # 2.5 s
HALF = (0.5, 0.5, 0.5)  # box half-extents
DROP_Z = 3.0


def build_model():
    builder = newton.ModelBuilder()
    builder.default_shape_cfg.mu = 0.5  # friction
    builder.add_ground_plane()

    body = builder.add_body(
        xform=wp.transform(p=wp.vec3(0.0, 0.0, DROP_Z), q=wp.quat_identity()),
        label="box",
    )
    builder.add_shape_box(body, hx=HALF[0], hy=HALF[1], hz=HALF[2])
    return builder.finalize()


def main():
    print(f"warp {wp.__version__}, newton {newton.__version__}, device={wp.get_device()}")
    model = build_model()
    solver = newton.solvers.SolverXPBD(model, iterations=10)

    state_0 = model.state()
    state_1 = model.state()
    control = model.control()
    contacts = model.contacts()

    frame_dt = 1.0 / FPS
    sim_dt = frame_dt / SUBSTEPS

    frames = []  # each: [px, py, pz, qx, qy, qz, qw] for body 0
    t0 = time.time()
    for _ in range(FRAMES):
        for _ in range(SUBSTEPS):
            state_0.clear_forces()
            model.collide(state_0, contacts)
            solver.step(state_0, state_1, control, contacts, sim_dt)
            state_0, state_1 = state_1, state_0
        q = state_0.body_q.numpy()[0]  # (7,) transform of the box
        frames.append([float(v) for v in q])
    elapsed = time.time() - t0

    z0 = frames[0][2]
    zf = frames[-1][2]
    print(f"simulated {FRAMES} frames in {elapsed:.2f}s ({FRAMES / elapsed:.0f} fps on CPU)")
    print(f"box z: start={z0:.3f}  ->  end={zf:.3f}  (expected rest ~= {HALF[2]:.3f})")
    print("z trajectory (every 15 frames):",
          [round(frames[i][2], 2) for i in range(0, FRAMES, 15)])

    assert z0 > 2.0, "box did not start elevated"
    assert zf < z0, "box did not fall"
    assert abs(zf - HALF[2]) < 0.15, f"box did not settle near {HALF[2]} (got {zf:.3f})"
    print("\nPASS: rigid body fell and settled; per-frame transforms extracted.")


if __name__ == "__main__":
    main()
