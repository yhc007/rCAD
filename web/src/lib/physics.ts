// Client bridge to the Newton physics service.
//
// Builds a rigid-body scene from the current meshes, normalizes it to unit scale
// (so Newton's metric defaults behave well regardless of rCAD units), runs the
// sim, and scales the per-frame transforms back. transformGeometryByPose applies
// a frame's pose to a rest mesh so the renderer can play the result back.

import type { MeshGeometry } from './meshParsers';

export interface Simulation {
  fps: number;
  bodyIds: string[];
  centers: Record<string, [number, number, number]>; // rest AABB center (rCAD units)
  frames: number[][][]; // frames[f][b] = [px,py,pz, qx,qy,qz,qw] in rCAD units
}

function aabb(g: MeshGeometry) {
  let minX = Infinity, minY = Infinity, minZ = Infinity;
  let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;
  const p = g.positions;
  for (let i = 0; i < p.length; i += 3) {
    if (p[i] < minX) minX = p[i];
    if (p[i] > maxX) maxX = p[i];
    if (p[i + 1] < minY) minY = p[i + 1];
    if (p[i + 1] > maxY) maxY = p[i + 1];
    if (p[i + 2] < minZ) minZ = p[i + 2];
    if (p[i + 2] > maxZ) maxZ = p[i + 2];
  }
  return {
    center: [(minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2] as const,
    half: [(maxX - minX) / 2, (maxY - minY) / 2, (maxZ - minZ) / 2] as const,
  };
}

// Convex-hull collider for arbitrary geometry: the mesh re-centered to the body
// origin and normalized. Only the point cloud matters (the hull is recomputed
// server-side), so triangle winding is irrelevant.
function meshCollider(
  geom: MeshGeometry,
  center: readonly number[],
  scale: number,
  decompose: boolean
) {
  const n = geom.vertexCount;
  const positions = new Array(n * 3);
  for (let i = 0; i < n; i++) {
    positions[i * 3] = (geom.positions[i * 3] - center[0]) / scale;
    positions[i * 3 + 1] = (geom.positions[i * 3 + 1] - center[1]) / scale;
    positions[i * 3 + 2] = (geom.positions[i * 3 + 2] - center[2]) / scale;
  }
  const indices = Array.from({ length: n }, (_, i) => i);
  return { type: 'mesh', convex: true, decompose, positions, indices };
}

// Collider from the rCAD feature type. Primitives use exact analytic shapes
// (rCAD cylinders are +Z, Newton cylinders are Z-local → no axis rotation);
// cones, booleans and imports fall back to a convex hull of their mesh.
function colliderFor(
  type: string | undefined,
  hN: number[],
  geom: MeshGeometry,
  center: readonly number[],
  scale: number,
  rotated: boolean
) {
  // A rotated primitive's AABB no longer matches its analytic shape, so use a
  // convex hull of the actual (rotated) mesh — e.g. a tilted ramp.
  if (rotated) return meshCollider(geom, center, scale, false);
  switch (type) {
    case 'Box':
      return { type: 'box', hx: hN[0], hy: hN[1], hz: hN[2] };
    case 'Sphere':
      return { type: 'sphere', radius: Math.max(hN[0], hN[1], hN[2]) };
    case 'Cylinder':
      return { type: 'cylinder', radius: (hN[0] + hN[1]) / 2, half_height: hN[2] };
    default:
      // Subtract is the canonical concave case → convex-decompose it; other
      // arbitrary shapes (Cone, Union, Intersect, Import) use a single hull.
      return meshCollider(geom, center, scale, type === 'Subtract');
  }
}

export interface BodyProps {
  type: string;
  fixed: boolean;
  mass: number;
  rotated: boolean; // analytic colliders can't capture rotation → use a hull
}

// Build the sim request + the metadata needed to play it back. Fixed bodies stay
// at their rest pose (static anchors); dynamic bodies are dropped from a tower
// above the highest fixed body so they fall onto it.
function buildSimulation(
  meshes: Record<string, MeshGeometry>,
  props: Record<string, BodyProps>
) {
  const bodyIds = Object.keys(meshes);
  const info = bodyIds.map((id) => ({ id, ...aabb(meshes[id]) }));

  // Normalize so the largest body is ~1 unit (keeps Newton contact/mass stable).
  const scale = Math.max(
    1e-6,
    ...info.map((b) => Math.max(b.half[0], b.half[1], b.half[2]) * 2)
  );

  const centers: Record<string, [number, number, number]> = {};
  const bodies = [];

  // Fixed bodies at their rest position; track the highest top.
  let fixedTopN = 0;
  for (const b of info) {
    if (!props[b.id]?.fixed) continue;
    const hN = [b.half[0] / scale, b.half[1] / scale, b.half[2] / scale];
    const cN = [b.center[0] / scale, b.center[1] / scale, b.center[2] / scale];
    bodies.push({
      id: b.id,
      shape: colliderFor(
        props[b.id]?.type,
        hN,
        meshes[b.id],
        b.center,
        scale,
        props[b.id]?.rotated ?? false
      ),
      transform: [cN[0], cN[1], cN[2], 0, 0, 0, 1],
      mass: 0,
      fixed: true,
    });
    centers[b.id] = [b.center[0], b.center[1], b.center[2]];
    fixedTopN = Math.max(fixedTopN, cN[1] + hN[1]);
  }

  // Dynamic bodies dropped in a tower above the fixed bodies (and ground).
  let yCursor = Math.max(1.0, fixedTopN + 0.5);
  for (const b of info) {
    if (props[b.id]?.fixed) continue;
    const hN = [b.half[0] / scale, b.half[1] / scale, b.half[2] / scale];
    const posY = yCursor + hN[1];
    bodies.push({
      id: b.id,
      shape: colliderFor(
        props[b.id]?.type,
        hN,
        meshes[b.id],
        b.center,
        scale,
        props[b.id]?.rotated ?? false
      ),
      transform: [b.center[0] / scale, posY, b.center[2] / scale, 0, 0, 0, 1],
      mass: props[b.id]?.mass ?? 0,
      fixed: false,
    });
    yCursor = posY + hN[1] + 0.4; // gap before the next body
    centers[b.id] = [b.center[0], b.center[1], b.center[2]];
  }

  const request = {
    bodies,
    gravity: -9.81,
    fps: 60,
    frames: 180,
    substeps: 10,
    solver: 'xpbd' as const,
  };
  return { request, scale, centers, bodyIds };
}

/** Run a rigid-body simulation of the current meshes via the physics service. */
export async function runSimulation(
  meshes: Record<string, MeshGeometry>,
  props: Record<string, BodyProps>
): Promise<Simulation> {
  const { request, scale, centers, bodyIds } = buildSimulation(meshes, props);

  const res = await fetch('/physics/simulate', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(request),
  });
  if (!res.ok) throw new Error(`physics service returned ${res.status}`);
  const data: { fps: number; body_ids: string[]; frames: number[][][] } =
    await res.json();

  const idx = Object.fromEntries(data.body_ids.map((id, i) => [id, i]));
  // Reorder to bodyIds and scale positions back to rCAD units.
  const frames = data.frames.map((frame) =>
    bodyIds.map((id) => {
      const t = frame[idx[id]];
      return [t[0] * scale, t[1] * scale, t[2] * scale, t[3], t[4], t[5], t[6]];
    })
  );

  return { fps: data.fps, bodyIds, centers, frames };
}

// Rotate vector (vx,vy,vz) by quaternion (x,y,z,w).
function qrot(
  x: number, y: number, z: number, w: number,
  vx: number, vy: number, vz: number
): [number, number, number] {
  const tx = 2 * (y * vz - z * vy);
  const ty = 2 * (z * vx - x * vz);
  const tz = 2 * (x * vy - y * vx);
  return [
    vx + w * tx + (y * tz - z * ty),
    vy + w * ty + (z * tx - x * tz),
    vz + w * tz + (x * ty - y * tx),
  ];
}

/** Apply a rigid pose to a rest mesh: v' = R(q)·(v - center) + p, n' = R(q)·n. */
export function transformGeometryByPose(
  geom: MeshGeometry,
  center: [number, number, number],
  pose: number[]
): MeshGeometry {
  const [px, py, pz, qx, qy, qz, qw] = pose;
  const [cx, cy, cz] = center;
  const n = geom.vertexCount;
  const positions = new Float32Array(n * 3);
  const normals = new Float32Array(n * 3);
  const P = geom.positions;
  const N = geom.normals;
  for (let i = 0; i < n; i++) {
    const [rx, ry, rz] = qrot(qx, qy, qz, qw, P[i * 3] - cx, P[i * 3 + 1] - cy, P[i * 3 + 2] - cz);
    positions[i * 3] = rx + px;
    positions[i * 3 + 1] = ry + py;
    positions[i * 3 + 2] = rz + pz;
    const [nx, ny, nz] = qrot(qx, qy, qz, qw, N[i * 3], N[i * 3 + 1], N[i * 3 + 2]);
    normals[i * 3] = nx;
    normals[i * 3 + 1] = ny;
    normals[i * 3 + 2] = nz;
  }
  return { positions, normals, vertexCount: n };
}
