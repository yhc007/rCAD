// Pure mesh transforms used for imported meshes (which have no WASM B-Rep solid
// to move) and for the gizmo's live drag preview. Kept here so the renderer, the
// PropertyPanel and the command-history replay all share identical math.

import type { MeshGeometry } from './meshParsers';

/** Translate every vertex by d (imported meshes have no solid to move). */
export function offsetMesh(g: MeshGeometry, d: readonly [number, number, number]): MeshGeometry {
  const positions = new Float32Array(g.positions.length);
  for (let i = 0; i < g.positions.length; i += 3) {
    positions[i] = g.positions[i] + d[0];
    positions[i + 1] = g.positions[i + 1] + d[1];
    positions[i + 2] = g.positions[i + 2] + d[2];
  }
  return { positions, normals: g.normals, vertexCount: g.vertexCount };
}

/** Rotate a vector about world axis 0=X/1=Y/2=Z by `a` radians. */
export function rotAxis(
  axis: number,
  a: number,
  x: number,
  y: number,
  z: number
): [number, number, number] {
  const c = Math.cos(a), s = Math.sin(a);
  if (axis === 0) return [x, y * c - z * s, y * s + z * c];
  if (axis === 1) return [x * c + z * s, y, -x * s + z * c];
  return [x * c - y * s, x * s + y * c, z];
}

/** Rotate an imported mesh in place about its bounding-box centre. */
export function rotateMeshAboutCenter(
  g: MeshGeometry,
  axis: number,
  a: number
): MeshGeometry {
  let minX = Infinity, minY = Infinity, minZ = Infinity;
  let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;
  const p = g.positions;
  for (let i = 0; i < p.length; i += 3) {
    minX = Math.min(minX, p[i]); maxX = Math.max(maxX, p[i]);
    minY = Math.min(minY, p[i + 1]); maxY = Math.max(maxY, p[i + 1]);
    minZ = Math.min(minZ, p[i + 2]); maxZ = Math.max(maxZ, p[i + 2]);
  }
  const cx = (minX + maxX) / 2, cy = (minY + maxY) / 2, cz = (minZ + maxZ) / 2;
  const positions = new Float32Array(p.length);
  const normals = new Float32Array(g.normals.length);
  for (let i = 0; i < p.length; i += 3) {
    const [rx, ry, rz] = rotAxis(axis, a, p[i] - cx, p[i + 1] - cy, p[i + 2] - cz);
    positions[i] = rx + cx; positions[i + 1] = ry + cy; positions[i + 2] = rz + cz;
    const [nx, ny, nz] = rotAxis(axis, a, g.normals[i], g.normals[i + 1], g.normals[i + 2]);
    normals[i] = nx; normals[i + 1] = ny; normals[i + 2] = nz;
  }
  return { positions, normals, vertexCount: g.vertexCount };
}
