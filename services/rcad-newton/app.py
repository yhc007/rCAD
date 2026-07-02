"""rCAD Newton physics microservice.

Precomputes a rigid-body simulation (CPU) and returns the per-frame transforms
for the rCAD viewer to play back. Runs Newton in Y-up so coordinates match
rCAD directly (no axis conversion needed).

Run: uv run uvicorn app:app --port 8000
"""

from typing import Literal

import coacd
import numpy as np
import warp as wp
from fastapi import FastAPI
from pydantic import BaseModel, Field

import newton

coacd.set_log_level("error")

app = FastAPI(title="rCAD Newton physics service")


# --- request / response schema -------------------------------------------

class Shape(BaseModel):
    type: Literal["box", "sphere", "cylinder", "mesh"]
    # box half-extents
    hx: float = 0.5
    hy: float = 0.5
    hz: float = 0.5
    # sphere / cylinder
    radius: float = 0.5
    half_height: float = 0.5
    # mesh (flat xyz positions + triangle indices, body-local)
    positions: list[float] | None = None
    indices: list[int] | None = None
    convex: bool = False  # build a convex-hull collider instead of a raw mesh
    decompose: bool = False  # convex-decompose (coacd) into multiple hulls


class Body(BaseModel):
    id: str
    shape: Shape
    # initial pose: [px, py, pz, qx, qy, qz, qw] in rCAD Y-up world
    transform: list[float] = Field(default_factory=lambda: [0, 0, 0, 0, 0, 0, 1])
    mass: float = 0.0  # 0 = auto-compute from shape density
    fixed: bool = False  # kinematic anchor / static collider


class SimRequest(BaseModel):
    bodies: list[Body]
    gravity: float = -9.81  # along +up (Y)
    fps: int = 60
    frames: int = 150
    substeps: int = 10
    solver: Literal["xpbd", "vbd"] = "xpbd"
    friction: float = 0.5


class SimResponse(BaseModel):
    fps: int
    body_ids: list[str]
    # frames[f][b] = [px, py, pz, qx, qy, qz, qw] for body_ids[b], Y-up
    frames: list[list[list[float]]]


# --- simulation ----------------------------------------------------------

def _xform(t: list[float]) -> wp.transform:
    return wp.transform(
        p=wp.vec3(float(t[0]), float(t[1]), float(t[2])),
        q=wp.quat(float(t[3]), float(t[4]), float(t[5]), float(t[6])),
    )


# Convex-decompose a (possibly concave) mesh into multiple convex-hull shapes so
# pockets/holes (e.g. boolean subtractions) collide correctly. Falls back to a
# single convex hull if coacd fails.
def add_convex_decomposition(builder, body, verts, idx):
    try:
        parts = coacd.run_coacd(
            coacd.Mesh(verts.astype(np.float64), idx.reshape(-1, 3)),
            threshold=0.1,
            mcts_iterations=50,
            preprocess_resolution=30,
        )
        for pv, pf in parts:
            builder.add_shape_convex_hull(
                body,
                mesh=newton.Mesh(
                    np.asarray(pv, dtype=np.float32),
                    np.asarray(pf, dtype=np.int32).flatten(),
                ),
            )
        tracing_count = len(parts)
    except Exception as e:  # pragma: no cover - robustness
        import sys

        print(f"coacd failed ({e}); falling back to convex hull", file=sys.stderr)
        builder.add_shape_convex_hull(body, mesh=newton.Mesh(verts, idx))
        tracing_count = 1
    return tracing_count


def run_sim(req: SimRequest) -> SimResponse:
    builder = newton.ModelBuilder(up_axis=newton.Axis.Y, gravity=req.gravity)
    builder.default_shape_cfg.mu = req.friction
    builder.add_ground_plane()

    ids: list[str] = []
    for b in req.bodies:
        body = builder.add_body(
            xform=_xform(b.transform),
            is_kinematic=b.fixed,
            mass=0.0 if b.fixed else b.mass,
            label=b.id,
        )
        s = b.shape
        if s.type == "box":
            builder.add_shape_box(body, hx=s.hx, hy=s.hy, hz=s.hz)
        elif s.type == "sphere":
            builder.add_shape_sphere(body, radius=s.radius)
        elif s.type == "cylinder":
            builder.add_shape_cylinder(body, radius=s.radius, half_height=s.half_height)
        elif s.type == "mesh":
            verts = np.asarray(s.positions or [], dtype=np.float32).reshape(-1, 3)
            idx = np.asarray(s.indices or [], dtype=np.int32)
            if s.decompose:
                add_convex_decomposition(builder, body, verts, idx)
            elif s.convex:
                # Convex hull (<=64 verts) — stable rigid contacts; winding-agnostic.
                builder.add_shape_convex_hull(body, mesh=newton.Mesh(verts, idx))
            else:
                builder.add_shape_mesh(body, mesh=newton.Mesh(verts, idx))
        ids.append(b.id)

    if req.solver == "vbd":
        builder.color()  # VBD needs a graph coloring of contacts

    model = builder.finalize()
    solver = (
        newton.solvers.SolverVBD(model, iterations=10)
        if req.solver == "vbd"
        else newton.solvers.SolverXPBD(model, iterations=10)
    )

    state_0 = model.state()
    state_1 = model.state()
    control = model.control()
    contacts = model.contacts()
    sim_dt = (1.0 / req.fps) / req.substeps

    out_frames: list[list[list[float]]] = []
    for _ in range(req.frames):
        for _ in range(req.substeps):
            state_0.clear_forces()
            model.collide(state_0, contacts)
            solver.step(state_0, state_1, control, contacts, sim_dt)
            state_0, state_1 = state_1, state_0
        out_frames.append(state_0.body_q.numpy().tolist())

    return SimResponse(fps=req.fps, body_ids=ids, frames=out_frames)


# --- routes --------------------------------------------------------------

@app.get("/health")
def health() -> dict:
    return {
        "status": "ok",
        "newton": newton.__version__,
        "warp": wp.__version__,
        "device": str(wp.get_device()),
    }


# Sync handler → FastAPI runs the CPU-bound sim in a threadpool.
@app.post("/simulate", response_model=SimResponse)
def simulate(req: SimRequest) -> SimResponse:
    return run_sim(req)
