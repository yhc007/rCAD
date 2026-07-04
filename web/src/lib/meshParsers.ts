// Client-side STL / OBJ parsers.
//
// Both produce a flat, non-indexed triangle soup ({positions, normals} with one
// vertex per triangle corner) which is the simplest thing for the WebGL/WebGPU
// renderer to upload and draw. Normals are taken from the file when present and
// computed per-face otherwise.

export interface MeshGeometry {
  positions: Float32Array; // x,y,z per vertex (3 * vertexCount)
  normals: Float32Array; // nx,ny,nz per vertex (3 * vertexCount)
  vertexCount: number;
}

// --- shared helpers -------------------------------------------------------

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

function build(positions: number[], normals: number[]): MeshGeometry {
  return {
    positions: new Float32Array(positions),
    normals: new Float32Array(normals),
    vertexCount: positions.length / 3,
  };
}

// --- STL ------------------------------------------------------------------

/** Detect binary STL by checking the expected byte size (84 + 50*triangles). */
function isBinarySTL(buffer: ArrayBuffer): boolean {
  if (buffer.byteLength < 84) return false;
  const view = new DataView(buffer);
  const triangles = view.getUint32(80, true);
  return buffer.byteLength === 84 + triangles * 50;
}

function parseBinarySTL(buffer: ArrayBuffer): MeshGeometry {
  const view = new DataView(buffer);
  const triangles = view.getUint32(80, true);
  const positions: number[] = [];
  const normals: number[] = [];

  let offset = 84;
  for (let t = 0; t < triangles; t++) {
    let nx = view.getFloat32(offset, true);
    let ny = view.getFloat32(offset + 4, true);
    let nz = view.getFloat32(offset + 8, true);
    offset += 12;

    const verts: number[] = [];
    for (let v = 0; v < 3; v++) {
      verts.push(
        view.getFloat32(offset, true),
        view.getFloat32(offset + 4, true),
        view.getFloat32(offset + 8, true)
      );
      offset += 12;
    }
    offset += 2; // attribute byte count

    // Fall back to a computed normal when the file stores a zero normal.
    if (nx === 0 && ny === 0 && nz === 0) {
      [nx, ny, nz] = faceNormal(
        verts[0], verts[1], verts[2],
        verts[3], verts[4], verts[5],
        verts[6], verts[7], verts[8]
      );
    }
    for (let v = 0; v < 3; v++) {
      positions.push(verts[v * 3], verts[v * 3 + 1], verts[v * 3 + 2]);
      normals.push(nx, ny, nz);
    }
  }
  return build(positions, normals);
}

function parseAsciiSTL(text: string): MeshGeometry {
  const positions: number[] = [];
  const normals: number[] = [];
  const tokens = text.split(/\s+/);

  let curNormal: [number, number, number] = [0, 0, 0];
  const tri: number[] = [];

  for (let i = 0; i < tokens.length; i++) {
    const tok = tokens[i];
    if (tok === 'normal') {
      curNormal = [+tokens[i + 1], +tokens[i + 2], +tokens[i + 3]];
      i += 3;
    } else if (tok === 'vertex') {
      tri.push(+tokens[i + 1], +tokens[i + 2], +tokens[i + 3]);
      i += 3;
      if (tri.length === 9) {
        let [nx, ny, nz] = curNormal;
        if (nx === 0 && ny === 0 && nz === 0) {
          [nx, ny, nz] = faceNormal(
            tri[0], tri[1], tri[2],
            tri[3], tri[4], tri[5],
            tri[6], tri[7], tri[8]
          );
        }
        for (let v = 0; v < 3; v++) {
          positions.push(tri[v * 3], tri[v * 3 + 1], tri[v * 3 + 2]);
          normals.push(nx, ny, nz);
        }
        tri.length = 0;
      }
    }
  }
  return build(positions, normals);
}

export function parseSTL(buffer: ArrayBuffer): MeshGeometry {
  if (isBinarySTL(buffer)) return parseBinarySTL(buffer);
  return parseAsciiSTL(new TextDecoder().decode(buffer));
}

// --- OBJ ------------------------------------------------------------------

export function parseOBJ(text: string): MeshGeometry {
  const verts: number[] = []; // flat x,y,z
  const vnorms: number[] = []; // flat nx,ny,nz
  const positions: number[] = [];
  const normals: number[] = [];

  // OBJ indices are 1-based and may be negative (relative to current count).
  const resolve = (idx: number, count: number) =>
    idx > 0 ? idx - 1 : count + idx;

  const lines = text.split('\n');
  for (const raw of lines) {
    const line = raw.trim();
    if (line.length === 0 || line[0] === '#') continue;
    const parts = line.split(/\s+/);
    const tag = parts[0];

    if (tag === 'v') {
      verts.push(+parts[1], +parts[2], +parts[3]);
    } else if (tag === 'vn') {
      vnorms.push(+parts[1], +parts[2], +parts[3]);
    } else if (tag === 'f') {
      // Collect this face's corners, triangulating polygons as a fan.
      const corners = parts.slice(1).map((p) => {
        const [vi, , ni] = p.split('/');
        const vIdx = resolve(parseInt(vi, 10), verts.length / 3);
        const nIdx = ni ? resolve(parseInt(ni, 10), vnorms.length / 3) : -1;
        return { vIdx, nIdx };
      });

      for (let i = 1; i < corners.length - 1; i++) {
        const fan = [corners[0], corners[i], corners[i + 1]];
        const p = fan.map((c) => [
          verts[c.vIdx * 3], verts[c.vIdx * 3 + 1], verts[c.vIdx * 3 + 2],
        ]);

        let computed: [number, number, number] | null = null;
        if (fan.some((c) => c.nIdx < 0)) {
          computed = faceNormal(
            p[0][0], p[0][1], p[0][2],
            p[1][0], p[1][1], p[1][2],
            p[2][0], p[2][1], p[2][2]
          );
        }

        for (let k = 0; k < 3; k++) {
          positions.push(p[k][0], p[k][1], p[k][2]);
          const c = fan[k];
          if (c.nIdx >= 0) {
            normals.push(vnorms[c.nIdx * 3], vnorms[c.nIdx * 3 + 1], vnorms[c.nIdx * 3 + 2]);
          } else {
            normals.push(computed![0], computed![1], computed![2]);
          }
        }
      }
    }
  }
  return build(positions, normals);
}

export interface ImportedMesh {
  name: string;
  geometry: MeshGeometry;
  // Optional base colour (e.g. a glTF material colour), linear RGB 0..1.
  color?: [number, number, number];
}
