// Unified file import: STL/OBJ are parsed client-side; STEP and glTF/GLB are
// sent to the Rust server and returned as a tessellated mesh; IGES is
// unsupported.

import { parseSTL, parseOBJ, type MeshGeometry, type ImportedMesh } from './meshParsers';

// De-index a server (indexed) mesh into the renderer's flat geometry.
function deindex(d: { positions: number[]; normals: number[]; indices: number[] }): MeshGeometry {
  const I = d.indices;
  const n = I.length;
  const positions = new Float32Array(n * 3);
  const normals = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    const v = I[i];
    positions[i * 3] = d.positions[v * 3];
    positions[i * 3 + 1] = d.positions[v * 3 + 1];
    positions[i * 3 + 2] = d.positions[v * 3 + 2];
    normals[i * 3] = d.normals[v * 3];
    normals[i * 3 + 1] = d.normals[v * 3 + 1];
    normals[i * 3 + 2] = d.normals[v * 3 + 2];
  }
  return { positions, normals, vertexCount: n };
}

interface GltfPart {
  name: string;
  color?: [number, number, number];
  positions: number[];
  normals: number[];
  indices: number[];
}
interface GltfResp {
  success: boolean;
  message?: string;
  parts: GltfPart[];
}

// Server import → one part per material, each keeping its colour. Used by both
// glTF/GLB and STEP (the server converts STEP to glb via OpenCASCADE first, so
// complex assemblies come back as coloured parts).
async function importServerParts(file: File, endpoint: string, label: string): Promise<ImportedMesh[]> {
  const form = new FormData();
  form.append('file', file);
  const res = await fetch(endpoint, { method: 'POST', body: form });
  if (!res.ok) throw new Error(`server returned ${res.status}`);
  const data: GltfResp = await res.json();
  if (!data.success) throw new Error(data.message || `${label} import failed`);
  if (!data.parts?.length) throw new Error(`${label} produced no geometry`);
  return data.parts.map((p, i) => ({
    name: p.name || `${file.name} (${i + 1})`,
    geometry: deindex(p),
    color: p.color,
  }));
}

/** Import any supported file into one or more named meshes. Throws on
 *  unsupported types. Most formats yield a single part; glTF yields one part
 *  per material. */
export async function importMeshFile(file: File): Promise<ImportedMesh[]> {
  const ext = file.name.toLowerCase().split('.').pop();
  switch (ext) {
    case 'stl':
      return [{ name: file.name, geometry: parseSTL(await file.arrayBuffer()) }];
    case 'obj':
      return [{ name: file.name, geometry: parseOBJ(await file.text()) }];
    case 'step':
    case 'stp':
      return importServerParts(file, '/api/import/step', 'STEP');
    case 'gltf':
    case 'glb':
      return importServerParts(file, '/api/import/gltf', 'glTF');
    case 'iges':
    case 'igs':
      throw new Error('IGES is not supported — convert to STEP, glTF/GLB, STL, or OBJ.');
    default:
      throw new Error(`Unsupported file type: .${ext} (expected STL, OBJ, STEP, or glTF/GLB)`);
  }
}
