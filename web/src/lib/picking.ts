// CPU ray-pick: turn a screen click into a world-space ray and find the nearest
// mesh it hits. Backend-agnostic (works the same under WebGPU or WebGL) since it
// only needs the view-projection matrix and the triangle soup.

import type { MeshGeometry } from './meshParsers';

/** Invert a column-major 4x4 matrix (gl-matrix convention). Null if singular. */
export function mat4Inverse(m: Float32Array | number[]): Float32Array | null {
  const a00 = m[0], a01 = m[1], a02 = m[2], a03 = m[3];
  const a10 = m[4], a11 = m[5], a12 = m[6], a13 = m[7];
  const a20 = m[8], a21 = m[9], a22 = m[10], a23 = m[11];
  const a30 = m[12], a31 = m[13], a32 = m[14], a33 = m[15];

  const b00 = a00 * a11 - a01 * a10;
  const b01 = a00 * a12 - a02 * a10;
  const b02 = a00 * a13 - a03 * a10;
  const b03 = a01 * a12 - a02 * a11;
  const b04 = a01 * a13 - a03 * a11;
  const b05 = a02 * a13 - a03 * a12;
  const b06 = a20 * a31 - a21 * a30;
  const b07 = a20 * a32 - a22 * a30;
  const b08 = a20 * a33 - a23 * a30;
  const b09 = a21 * a32 - a22 * a31;
  const b10 = a21 * a33 - a23 * a31;
  const b11 = a22 * a33 - a23 * a32;

  let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
  if (!det) return null;
  det = 1.0 / det;

  const out = new Float32Array(16);
  out[0] = (a11 * b11 - a12 * b10 + a13 * b09) * det;
  out[1] = (a02 * b10 - a01 * b11 - a03 * b09) * det;
  out[2] = (a31 * b05 - a32 * b04 + a33 * b03) * det;
  out[3] = (a22 * b04 - a21 * b05 - a23 * b03) * det;
  out[4] = (a12 * b08 - a10 * b11 - a13 * b07) * det;
  out[5] = (a00 * b11 - a02 * b08 + a03 * b07) * det;
  out[6] = (a32 * b02 - a30 * b05 - a33 * b01) * det;
  out[7] = (a20 * b05 - a22 * b02 + a23 * b01) * det;
  out[8] = (a10 * b10 - a11 * b08 + a13 * b06) * det;
  out[9] = (a01 * b08 - a00 * b10 - a03 * b06) * det;
  out[10] = (a30 * b04 - a31 * b02 + a33 * b00) * det;
  out[11] = (a21 * b02 - a20 * b04 - a23 * b00) * det;
  out[12] = (a11 * b07 - a10 * b09 - a12 * b06) * det;
  out[13] = (a00 * b09 - a01 * b07 + a02 * b06) * det;
  out[14] = (a31 * b01 - a30 * b03 - a32 * b00) * det;
  out[15] = (a20 * b03 - a21 * b01 + a22 * b00) * det;
  return out;
}

/** Multiply a column-major matrix by (x,y,z,w) and do the perspective divide. */
export function unproject(
  inv: Float32Array,
  x: number,
  y: number,
  z: number
): [number, number, number] {
  const ox = inv[0] * x + inv[4] * y + inv[8] * z + inv[12];
  const oy = inv[1] * x + inv[5] * y + inv[9] * z + inv[13];
  const oz = inv[2] * x + inv[6] * y + inv[10] * z + inv[14];
  const ow = inv[3] * x + inv[7] * y + inv[11] * z + inv[15];
  return [ox / ow, oy / ow, oz / ow];
}

// Möller–Trumbore, double-sided. Returns distance t along dir, or null.
function rayTriangle(
  ox: number, oy: number, oz: number,
  dx: number, dy: number, dz: number,
  ax: number, ay: number, az: number,
  bx: number, by: number, bz: number,
  cx: number, cy: number, cz: number
): number | null {
  const e1x = bx - ax, e1y = by - ay, e1z = bz - az;
  const e2x = cx - ax, e2y = cy - ay, e2z = cz - az;
  const px = dy * e2z - dz * e2y;
  const py = dz * e2x - dx * e2z;
  const pz = dx * e2y - dy * e2x;
  const det = e1x * px + e1y * py + e1z * pz;
  if (Math.abs(det) < 1e-9) return null;
  const invDet = 1 / det;

  const tx = ox - ax, ty = oy - ay, tz = oz - az;
  const u = (tx * px + ty * py + tz * pz) * invDet;
  if (u < 0 || u > 1) return null;

  const qx = ty * e1z - tz * e1y;
  const qy = tz * e1x - tx * e1z;
  const qz = tx * e1y - ty * e1x;
  const v = (dx * qx + dy * qy + dz * qz) * invDet;
  if (v < 0 || u + v > 1) return null;

  const t = (e2x * qx + e2y * qy + e2z * qz) * invDet;
  return t > 1e-6 ? t : null;
}

/**
 * Find the nearest mesh hit by the ray through normalized device coords
 * (ndcX, ndcY in [-1, 1]). Returns the mesh's index in `meshes`, or null.
 */
export function pickMesh(
  meshes: MeshGeometry[],
  viewProj: Float32Array,
  ndcX: number,
  ndcY: number
): number | null {
  const inv = mat4Inverse(viewProj);
  if (!inv) return null;

  const [ox, oy, oz] = unproject(inv, ndcX, ndcY, -1); // near plane
  const [fx, fy, fz] = unproject(inv, ndcX, ndcY, 1); // far plane
  let dx = fx - ox, dy = fy - oy, dz = fz - oz;
  const len = Math.hypot(dx, dy, dz);
  if (len === 0) return null;
  dx /= len; dy /= len; dz /= len;

  let best = Infinity;
  let bestIdx: number | null = null;
  for (let mi = 0; mi < meshes.length; mi++) {
    const p = meshes[mi].positions;
    const triCount = Math.floor(meshes[mi].vertexCount / 3);
    for (let t = 0; t < triCount; t++) {
      const b = t * 9;
      const hit = rayTriangle(
        ox, oy, oz, dx, dy, dz,
        p[b], p[b + 1], p[b + 2],
        p[b + 3], p[b + 4], p[b + 5],
        p[b + 6], p[b + 7], p[b + 8]
      );
      if (hit !== null && hit < best) {
        best = hit;
        bestIdx = mi;
      }
    }
  }
  return bestIdx;
}
