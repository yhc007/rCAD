// Serialize the renderer's flat (non-indexed) MeshGeometry to STL / OBJ.
//
// Every renderable feature — primitive, boolean result, or imported file — is
// stored as a MeshGeometry, so exporting from that is uniform and matches
// exactly what's on screen (no WASM round-trip, works for imports too).

import type { MeshGeometry } from './meshParsers';

function faceNormal(
  ax: number, ay: number, az: number,
  bx: number, by: number, bz: number,
  cx: number, cy: number, cz: number
): [number, number, number] {
  const ux = bx - ax, uy = by - ay, uz = bz - az;
  const vx = cx - ax, vy = cy - ay, vz = cz - az;
  let nx = uy * vz - uz * vy;
  let ny = uz * vx - ux * vz;
  let nz = ux * vy - uy * vx;
  const len = Math.hypot(nx, ny, nz);
  if (len > 0) { nx /= len; ny /= len; nz /= len; }
  return [nx, ny, nz];
}

/** Binary STL (84-byte preamble + 50 bytes/triangle). */
export function geometryToSTL(g: MeshGeometry): Blob {
  const triCount = Math.floor(g.vertexCount / 3);
  const buffer = new ArrayBuffer(84 + triCount * 50);
  const dv = new DataView(buffer);
  dv.setUint32(80, triCount, true);

  let o = 84;
  const p = g.positions;
  for (let t = 0; t < triCount; t++) {
    const b = t * 9;
    const [nx, ny, nz] = faceNormal(
      p[b], p[b + 1], p[b + 2],
      p[b + 3], p[b + 4], p[b + 5],
      p[b + 6], p[b + 7], p[b + 8]
    );
    dv.setFloat32(o, nx, true);
    dv.setFloat32(o + 4, ny, true);
    dv.setFloat32(o + 8, nz, true);
    o += 12;
    for (let v = 0; v < 9; v++) {
      dv.setFloat32(o, p[b + v], true);
      o += 4;
    }
    dv.setUint16(o, 0, true); // attribute byte count
    o += 2;
  }
  return new Blob([buffer], { type: 'model/stl' });
}

/** Wavefront OBJ. Geometry is non-indexed, so one v/vn per triangle corner. */
export function geometryToOBJ(g: MeshGeometry): Blob {
  const n = g.vertexCount;
  const lines: string[] = ['# Exported from rCAD'];
  const p = g.positions;
  const nm = g.normals;
  for (let i = 0; i < n; i++) {
    lines.push(`v ${p[i * 3]} ${p[i * 3 + 1]} ${p[i * 3 + 2]}`);
  }
  for (let i = 0; i < n; i++) {
    lines.push(`vn ${nm[i * 3]} ${nm[i * 3 + 1]} ${nm[i * 3 + 2]}`);
  }
  for (let t = 0; t < Math.floor(n / 3); t++) {
    const a = t * 3 + 1, b = t * 3 + 2, c = t * 3 + 3; // OBJ is 1-based
    lines.push(`f ${a}//${a} ${b}//${b} ${c}//${c}`);
  }
  return new Blob([lines.join('\n') + '\n'], { type: 'text/plain' });
}

/** Trigger a browser download for a Blob. */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}
