// Web Worker that owns the WASM CADDocument.
//
// Running the CAD kernel off the main thread means a slow or hung geometry
// operation (notably truck-shapeops booleans on curved solids) can never freeze
// the UI — the main thread simply times out and terminates this worker.
//
// The worker is stateless from the caller's point of view except for a
// stableId -> wasmId map: the main thread assigns every solid a stable id and
// keeps an op-log, so after a forced restart it can replay the log and rebuild
// identical state under the same ids.

import init, { CADDocument } from './pkg/rcad_wasm.js';

type Req =
  | { reqId: number; method: 'init' }
  | { reqId: number; method: 'reset' }
  | { reqId: number; method: 'create'; args: { id: string; kind: string; params: number[] } }
  | { reqId: number; method: 'recreate'; args: { id: string; kind: string; params: number[] } }
  | { reqId: number; method: 'boolean'; args: { id: string; op: string; targetId: string; toolId: string } }
  | { reqId: number; method: 'mesh'; args: { id: string } }
  | { reqId: number; method: 'translate'; args: { id: string; dx: number; dy: number; dz: number } }
  | { reqId: number; method: 'rotate'; args: { id: string; ax: number; ay: number; az: number; angle: number } }
  | { reqId: number; method: 'export'; args: { id: string; format: 'stl' | 'obj'; binary: boolean } };

// The tsconfig uses the DOM lib (not WebWorker), so `self` is typed as Window.
// Narrow it to the dedicated-worker postMessage signature we actually use.
const post = (message: unknown, transfer: Transferable[] = []) =>
  (self as unknown as {
    postMessage(message: unknown, transfer: Transferable[]): void;
  }).postMessage(message, transfer);

let doc: CADDocument | null = null;
const idMap = new Map<string, string>(); // stableId -> wasm feature id

async function ensureDoc(): Promise<CADDocument> {
  if (!doc) {
    await init();
    doc = new CADDocument('Untitled');
  }
  return doc;
}

function wasmId(stableId: string): string {
  const id = idMap.get(stableId);
  if (!id) throw new Error(`Unknown feature: ${stableId}`);
  return id;
}

function create(d: CADDocument, kind: string, p: number[]): string {
  switch (kind) {
    case 'box':
      return d.create_box(p[0], p[1], p[2]);
    case 'cylinder':
      return d.create_cylinder(p[0], p[1]);
    case 'sphere':
      return d.create_sphere(p[0]);
    case 'cone':
      return d.create_cone(p[0], p[1], p[2]);
    case 'torus':
      return d.create_torus(p[0], p[1]);
    default:
      throw new Error(`Unknown primitive: ${kind}`);
  }
}

function boolean(d: CADDocument, op: string, target: string, tool: string): string {
  switch (op) {
    case 'union':
      return d.boolean_union(target, tool);
    case 'subtract':
      return d.boolean_subtract(target, tool);
    case 'intersect':
      return d.boolean_intersect(target, tool);
    default:
      throw new Error(`Unknown boolean op: ${op}`);
  }
}

self.onmessage = async (e: MessageEvent<Req>) => {
  const msg = e.data;
  try {
    const d = await ensureDoc();
    let result: unknown = null;
    let transfer: Transferable[] = [];

    switch (msg.method) {
      case 'init':
        result = true; // ensureDoc() above already initialized the module
        break;

      case 'reset':
        doc = new CADDocument('Untitled');
        idMap.clear();
        result = true;
        break;

      case 'create': {
        const created = create(d, msg.args.kind, msg.args.params);
        idMap.set(msg.args.id, created);
        result = msg.args.id;
        break;
      }

      // Build a fresh solid for an existing stable id (dimension edit). The old
      // solid is abandoned in the document; the stable id now points at the new
      // one. Any prior placement is re-applied by the caller via translate/rotate.
      case 'recreate': {
        const created = create(d, msg.args.kind, msg.args.params);
        idMap.set(msg.args.id, created);
        result = msg.args.id;
        break;
      }

      case 'boolean': {
        const created = boolean(
          d,
          msg.args.op,
          wasmId(msg.args.targetId),
          wasmId(msg.args.toolId)
        );
        idMap.set(msg.args.id, created);
        result = msg.args.id;
        break;
      }

      case 'mesh': {
        const md = d.tessellate(wasmId(msg.args.id));
        const positions = md.positions;
        const normals = md.normals;
        const indices = md.indices;
        result = { positions, normals, indices };
        transfer = [positions.buffer, normals.buffer, indices.buffer];
        break;
      }

      case 'translate': {
        const md = d.translate_feature(
          wasmId(msg.args.id),
          msg.args.dx,
          msg.args.dy,
          msg.args.dz
        );
        const positions = md.positions;
        const normals = md.normals;
        const indices = md.indices;
        result = { positions, normals, indices };
        transfer = [positions.buffer, normals.buffer, indices.buffer];
        break;
      }

      case 'rotate': {
        const md = d.rotate_feature(
          wasmId(msg.args.id),
          msg.args.ax,
          msg.args.ay,
          msg.args.az,
          msg.args.angle
        );
        const positions = md.positions;
        const normals = md.normals;
        const indices = md.indices;
        result = { positions, normals, indices };
        transfer = [positions.buffer, normals.buffer, indices.buffer];
        break;
      }

      case 'export': {
        result =
          msg.args.format === 'stl'
            ? d.export_stl(wasmId(msg.args.id), msg.args.binary)
            : d.export_obj(wasmId(msg.args.id));
        break;
      }
    }

    post({ reqId: msg.reqId, result }, transfer);
  } catch (err) {
    post({ reqId: msg.reqId, error: String(err) });
  }
};
