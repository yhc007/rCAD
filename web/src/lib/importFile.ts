// Unified file import: STL/OBJ are parsed client-side; STEP and glTF/GLB are
// sent to the Rust server and returned as a tessellated mesh; IGES is
// unsupported.

import { parseSTL, parseOBJ, type MeshGeometry, type ImportedMesh } from './meshParsers';

interface ServerMesh {
  success: boolean;
  message?: string;
  positions: number[];
  normals: number[];
  indices: number[];
}

// De-index a server (indexed) mesh into the renderer's flat geometry.
function deindex(d: ServerMesh): MeshGeometry {
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

async function importViaServer(file: File, endpoint: string, label: string): Promise<MeshGeometry> {
  const form = new FormData();
  form.append('file', file);
  const res = await fetch(endpoint, { method: 'POST', body: form });
  if (!res.ok) throw new Error(`server returned ${res.status}`);
  const data: ServerMesh = await res.json();
  if (!data.success) throw new Error(data.message || `${label} import failed`);
  if (!data.indices?.length) throw new Error(`${label} produced no geometry`);
  return deindex(data);
}

/** Import any supported file into a named mesh. Throws on unsupported types. */
export async function importMeshFile(file: File): Promise<ImportedMesh> {
  const ext = file.name.toLowerCase().split('.').pop();
  switch (ext) {
    case 'stl':
      return { name: file.name, geometry: parseSTL(await file.arrayBuffer()) };
    case 'obj':
      return { name: file.name, geometry: parseOBJ(await file.text()) };
    case 'step':
    case 'stp':
      return { name: file.name, geometry: await importViaServer(file, '/api/import/step', 'STEP') };
    case 'gltf':
    case 'glb':
      return { name: file.name, geometry: await importViaServer(file, '/api/import/gltf', 'glTF') };
    case 'iges':
    case 'igs':
      throw new Error('IGES is not supported — convert to STEP, glTF/GLB, STL, or OBJ.');
    default:
      throw new Error(`Unsupported file type: .${ext} (expected STL, OBJ, STEP, or glTF/GLB)`);
  }
}
