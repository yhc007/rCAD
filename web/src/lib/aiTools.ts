// AI copilot tool contract.
//
// The LLM drives rCAD purely through these tools, which map 1:1 onto the
// existing command API (cad.*) + store reads. Because every write goes through
// a command, AI actions are automatically undoable, saved in .rcad, and
// replayable. Read tools ground the model in the current document.

import { cad } from '../hooks/useCAD';
import { useDocumentStore, type Feature } from '../stores/documentStore';
import type { MeshGeometry } from './meshParsers';
import { runSimulation } from './physics';

// --- Anthropic tool definitions ---------------------------------------------

export interface ToolDef {
  name: string;
  description: string;
  input_schema: Record<string, unknown>;
}

const num = { type: 'number' };
export const AI_TOOLS: ToolDef[] = [
  {
    name: 'list_features',
    description:
      'List every part in the document with its id, type, dimensions, placement, colour and bounding box. Call this first to understand the model before editing.',
    input_schema: { type: 'object', properties: {} },
  },
  {
    name: 'create_primitive',
    description:
      'Create a primitive solid at the origin. Units are millimetres, Y is up. params by shape: box=[width,height,depth], cylinder=[radius,height], sphere=[radius], cone=[bottomRadius,topRadius,height].',
    input_schema: {
      type: 'object',
      properties: {
        shape: { type: 'string', enum: ['box', 'cylinder', 'sphere', 'cone'] },
        params: { type: 'array', items: num, description: 'dimensions for the shape' },
      },
      required: ['shape', 'params'],
    },
  },
  {
    name: 'move_feature',
    description: 'Translate a part by (dx,dy,dz) millimetres. Y is up.',
    input_schema: {
      type: 'object',
      properties: { id: { type: 'string' }, dx: num, dy: num, dz: num },
      required: ['id', 'dx', 'dy', 'dz'],
    },
  },
  {
    name: 'rotate_feature',
    description: 'Rotate a part in place about its centre, around a world axis, by an angle in degrees.',
    input_schema: {
      type: 'object',
      properties: {
        id: { type: 'string' },
        axis: { type: 'string', enum: ['x', 'y', 'z'] },
        degrees: num,
      },
      required: ['id', 'axis', 'degrees'],
    },
  },
  {
    name: 'resize_feature',
    description:
      'Change the dimensions of a primitive part (box/cylinder/sphere/cone), keeping its placement. params use the same layout as create_primitive.',
    input_schema: {
      type: 'object',
      properties: { id: { type: 'string' }, params: { type: 'array', items: num } },
      required: ['id', 'params'],
    },
  },
  {
    name: 'set_material',
    description: 'Set a part\'s colour. Accepts a hex string like "#3b82f6" or rgb floats 0..1.',
    input_schema: {
      type: 'object',
      properties: {
        id: { type: 'string' },
        color: { description: 'hex string or [r,g,b] in 0..1' },
      },
      required: ['id', 'color'],
    },
  },
  {
    name: 'set_physics',
    description:
      'Set physics properties for a part used by the Newton simulation: fixed (static anchor) and/or mass (0 = auto).',
    input_schema: {
      type: 'object',
      properties: { id: { type: 'string' }, fixed: { type: 'boolean' }, mass: num },
      required: ['id'],
    },
  },
  {
    name: 'generate_mesh',
    description:
      'Generate an organic/freeform 3D mesh from a short text prompt (Tripo text-to-3D) and import it as a part. Use for shapes that are not simple primitives — gears, brackets, toys, figurines, characters. It is SLOW (tens of seconds) and costs credits, so it is approval-gated. The result is auto-scaled to ~50 mm and placed at the origin.',
    input_schema: {
      type: 'object',
      properties: {
        prompt: { type: 'string', description: 'what to generate, e.g. "a low-poly rabbit"' },
      },
      required: ['prompt'],
    },
  },
  {
    name: 'boolean_op',
    description:
      'Combine two solids: union, subtract (target − tool), or intersect. NOTE: the current kernel (truck-shapeops) frequently FAILS on overlapping solids; prefer positioning parts as an assembly over subtract. Check the result.',
    input_schema: {
      type: 'object',
      properties: {
        op: { type: 'string', enum: ['union', 'subtract', 'intersect'] },
        target_id: { type: 'string' },
        tool_id: { type: 'string' },
      },
      required: ['op', 'target_id', 'tool_id'],
    },
  },
  {
    name: 'delete_feature',
    description: 'Delete a part from the document.',
    input_schema: {
      type: 'object',
      properties: { id: { type: 'string' } },
      required: ['id'],
    },
  },
  {
    name: 'duplicate_feature',
    description:
      'Duplicate a part `count` times (default 1), each copy offset from the original by (dx,dy,dz) mm × its index — i.e. a linear array. Returns the new ids.',
    input_schema: {
      type: 'object',
      properties: {
        id: { type: 'string' },
        count: { type: 'number', description: 'number of copies (default 1)' },
        dx: num, dy: num, dz: num,
      },
      required: ['id'],
    },
  },
  {
    name: 'align',
    description:
      'Move a part along one axis so its min / center / max edge lines up with another part\'s min / center / max on that axis.',
    input_schema: {
      type: 'object',
      properties: {
        id: { type: 'string' },
        reference_id: { type: 'string' },
        axis: { type: 'string', enum: ['x', 'y', 'z'] },
        mode: { type: 'string', enum: ['min', 'center', 'max'] },
      },
      required: ['id', 'reference_id', 'axis', 'mode'],
    },
  },
  {
    name: 'stack_on',
    description:
      'Place a part centred on top of a base part: matches X/Z centres and sets the part\'s bottom to the base\'s top (+ optional gap mm). Ideal for assemblies.',
    input_schema: {
      type: 'object',
      properties: { id: { type: 'string' }, base_id: { type: 'string' }, gap: num },
      required: ['id', 'base_id'],
    },
  },
  {
    name: 'simulate_physics',
    description:
      'Run a Newton rigid-body drop test on the current model and start playback for the user. Parts marked fixed are anchors; the rest fall under gravity. Returns, per part, its start/end height, whether it settled, and how far it moved — use this to answer "does it stand / stay?" and iterate.',
    input_schema: { type: 'object', properties: {} },
  },
  {
    name: 'select_feature',
    description: 'Select a part (or pass null to clear selection) so the user sees what you are working on.',
    input_schema: {
      type: 'object',
      properties: { id: { type: ['string', 'null'] } },
      required: ['id'],
    },
  },
  {
    name: 'undo',
    description: 'Undo the last document change.',
    input_schema: { type: 'object', properties: {} },
  },
];

// Tools gated behind user approval (hybrid autonomy): destructive/unreliable
// ops (delete, boolean) and paid external calls (generate_mesh). Reads and
// simple create/edit run automatically.
export const APPROVAL_GATED = new Set(['boolean_op', 'delete_feature', 'generate_mesh']);
export const requiresApproval = (name: string) => APPROVAL_GATED.has(name);

// --- executor ----------------------------------------------------------------

const AXIS: Record<string, number> = { x: 0, y: 1, z: 2 };

function bboxOf(id: string): { min: number[]; max: number[]; size: number[] } | null {
  const g: MeshGeometry | undefined = useDocumentStore.getState().meshes[id];
  if (!g || g.vertexCount === 0) return null;
  const p = g.positions;
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < p.length; i += 3) {
    for (let k = 0; k < 3; k++) {
      if (p[i + k] < min[k]) min[k] = p[i + k];
      if (p[i + k] > max[k]) max[k] = p[i + k];
    }
  }
  const round = (v: number) => Math.round(v * 100) / 100;
  return {
    min: min.map(round),
    max: max.map(round),
    size: [round(max[0] - min[0]), round(max[1] - min[1]), round(max[2] - min[2])],
  };
}

function featureSummary(f: Feature) {
  return {
    id: f.id,
    name: f.name,
    type: f.type,
    params: f.params,
    position: f.position ?? [0, 0, 0],
    rotation: f.rotation ?? [0, 0, 0],
    color: f.color,
    fixed: !!f.fixed,
    bbox: bboxOf(f.id),
  };
}

/** Compact document state to ground the model each turn. */
export function documentSnapshot() {
  const s = useDocumentStore.getState();
  return {
    units: 'millimetres, Y-up',
    documentName: s.documentName,
    selection: s.selectedFeatures,
    features: s.features.map(featureSummary),
  };
}

function toRgb(color: unknown): [number, number, number] {
  if (Array.isArray(color) && color.length === 3) {
    return color.map((v) => Math.max(0, Math.min(1, Number(v)))) as [number, number, number];
  }
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(String(color).trim());
  if (!m) return [0.6, 0.6, 0.7];
  return [parseInt(m[1], 16) / 255, parseInt(m[2], 16) / 255, parseInt(m[3], 16) / 255];
}

// De-index a server (indexed) mesh into the renderer's flat geometry.
function deindexServer(d: { positions: number[]; normals: number[]; indices: number[] }): MeshGeometry {
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

type Input = Record<string, unknown>;

/** Execute one tool call against the live document. Always resolves (errors are
 *  returned as { error } so the agent loop can feed them back to the model). */
export async function executeTool(name: string, input: Input): Promise<unknown> {
  const store = useDocumentStore.getState();
  try {
    switch (name) {
      case 'list_features':
        return { features: store.features.map(featureSummary) };

      case 'create_primitive': {
        const id = await cad.addPrimitive(String(input.shape), (input.params as number[]) ?? []);
        return { id, bbox: bboxOf(id) };
      }

      case 'move_feature': {
        const id = String(input.id);
        await cad.moveFeature(id, Number(input.dx) || 0, Number(input.dy) || 0, Number(input.dz) || 0);
        const f = useDocumentStore.getState().features.find((x) => x.id === id);
        return { id, position: f?.position ?? [0, 0, 0] };
      }

      case 'rotate_feature': {
        const id = String(input.id);
        const axis = AXIS[String(input.axis)] ?? 1;
        await cad.spinFeature(id, axis, ((Number(input.degrees) || 0) * Math.PI) / 180);
        const f = useDocumentStore.getState().features.find((x) => x.id === id);
        return { id, rotation: f?.rotation ?? [0, 0, 0] };
      }

      case 'resize_feature': {
        const id = String(input.id);
        const f = store.features.find((x) => x.id === id);
        if (!f) return { error: `no feature ${id}` };
        const prim = f.type.toLowerCase();
        if (!['box', 'cylinder', 'sphere', 'cone'].includes(prim))
          return { error: `resize only applies to primitives, not ${f.type}` };
        await cad.resizeFeature(id, prim, (input.params as number[]) ?? []);
        return { id, params: input.params, bbox: bboxOf(id) };
      }

      case 'set_material': {
        const id = String(input.id);
        const rgb = toRgb(input.color);
        await cad.setFeatureProps(id, { color: rgb });
        return { id, color: rgb };
      }

      case 'set_physics': {
        const id = String(input.id);
        const patch: Record<string, unknown> = {};
        if (typeof input.fixed === 'boolean') patch.fixed = input.fixed;
        if (typeof input.mass === 'number') patch.mass = Math.max(0, input.mass);
        await cad.setFeatureProps(id, patch);
        return { id, ...patch };
      }

      case 'generate_mesh': {
        const prompt = String(input.prompt ?? '').trim();
        if (!prompt) return { error: 'prompt is required' };
        const res = await fetch('/api/tripo/generate', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ prompt }),
        });
        const data = await res.json();
        if (!res.ok || data.error || !(data.indices && data.indices.length)) {
          return { error: data.error || data.message || `generation failed (${res.status})` };
        }
        const name = prompt.length > 32 ? prompt.slice(0, 32) + '…' : prompt;
        const color = Array.isArray(data.color) && data.color.length === 3 ? (data.color as [number, number, number]) : undefined;
        const id = await cad.importMesh(name, deindexServer(data), color);
        return { id, bbox: bboxOf(id), color };
      }

      case 'duplicate_feature': {
        const id = String(input.id);
        const f = store.features.find((x) => x.id === id);
        if (!f) return { error: `no feature ${id}` };
        const count = Math.max(1, Math.min(20, Number(input.count) || 1));
        const off = [Number(input.dx) || 0, Number(input.dy) || 0, Number(input.dz) || 0];
        const prim = f.type.toLowerCase();
        const isPrim = ['box', 'cylinder', 'sphere', 'cone'].includes(prim);
        const ids: string[] = [];
        for (let k = 1; k <= count; k++) {
          const t = [off[0] * k, off[1] * k, off[2] * k];
          let nid: string;
          if (isPrim) {
            nid = await cad.addPrimitive(prim, f.params ?? []);
            if (f.color) await cad.setFeatureProps(nid, { color: f.color });
            const pos = f.position ?? [0, 0, 0];
            const mv = [pos[0] + t[0], pos[1] + t[1], pos[2] + t[2]];
            if (mv.some((v) => v)) await cad.moveFeature(nid, mv[0], mv[1], mv[2]);
            const rot = f.rotation ?? [0, 0, 0];
            for (let a = 0; a < 3; a++) if (rot[a]) await cad.spinFeature(nid, a, (rot[a] * Math.PI) / 180);
          } else {
            // Clone the already-placed mesh, then apply just the array offset.
            nid = await cad.importMesh(`${f.name} copy`, store.meshes[id], f.color);
            if (t.some((v) => v)) await cad.moveFeature(nid, t[0], t[1], t[2]);
          }
          ids.push(nid);
        }
        return { ids };
      }

      case 'align': {
        const a = bboxOf(String(input.id));
        const b = bboxOf(String(input.reference_id));
        if (!a || !b) return { error: 'both features need geometry' };
        const axis = AXIS[String(input.axis)] ?? 0;
        const mode = String(input.mode ?? 'center');
        const edge = (bb: { min: number[]; max: number[] }) =>
          mode === 'min' ? bb.min[axis] : mode === 'max' ? bb.max[axis] : (bb.min[axis] + bb.max[axis]) / 2;
        const delta = edge(b) - edge(a);
        const d = [0, 0, 0];
        d[axis] = delta;
        await cad.moveFeature(String(input.id), d[0], d[1], d[2]);
        return { id: input.id, movedBy: Math.round(delta * 100) / 100 };
      }

      case 'stack_on': {
        const a = bboxOf(String(input.id));
        const b = bboxOf(String(input.base_id));
        if (!a || !b) return { error: 'both features need geometry' };
        const gap = Number(input.gap) || 0;
        const dx = (b.min[0] + b.max[0]) / 2 - (a.min[0] + a.max[0]) / 2;
        const dz = (b.min[2] + b.max[2]) / 2 - (a.min[2] + a.max[2]) / 2;
        const dy = b.max[1] + gap - a.min[1];
        await cad.moveFeature(String(input.id), dx, dy, dz);
        return { id: input.id, placedOn: input.base_id };
      }

      case 'simulate_physics': {
        const meshes = store.meshes;
        if (Object.keys(meshes).length === 0) return { error: 'no geometry to simulate' };
        const props = Object.fromEntries(
          store.features.map((f) => [
            f.id,
            { type: f.type, fixed: !!f.fixed, mass: f.mass ?? 0, rotated: !!f.rotation?.some((v) => v !== 0) },
          ])
        );
        let sim;
        try {
          sim = await runSimulation(meshes, props);
        } catch (e) {
          return { error: `${e instanceof Error ? e.message : 'simulation failed'} (is the Newton service on :8000?)` };
        }
        useDocumentStore.getState().startSimulation(sim);
        const last = sim.frames.length - 1;
        const prev = Math.max(0, last - 5);
        const parts = sim.bodyIds.map((id, b) => {
          const f = store.features.find((x) => x.id === id);
          const y0 = sim.frames[0][b][1];
          const yL = sim.frames[last][b][1];
          const p0 = sim.frames[0][b];
          const pL = sim.frames[last][b];
          return {
            id,
            name: f?.name,
            fixed: !!f?.fixed,
            startY: Math.round(y0 * 10) / 10,
            endY: Math.round(yL * 10) / 10,
            dropped: Math.round((y0 - yL) * 10) / 10,
            settled: Math.abs(yL - sim.frames[prev][b][1]) < 0.5,
            horizontalMove: Math.round(Math.hypot(pL[0] - p0[0], pL[2] - p0[2]) * 10) / 10,
          };
        });
        return { frames: sim.frames.length, parts };
      }

      case 'boolean_op': {
        const id = await cad.applyBoolean(
          String(input.op),
          String(input.target_id),
          String(input.tool_id)
        );
        return { id, bbox: bboxOf(id) };
      }

      case 'delete_feature': {
        const id = String(input.id);
        await cad.deleteFeature(id);
        return { deleted: id };
      }

      case 'select_feature': {
        const id = input.id == null ? null : String(input.id);
        store.selectFeature(id);
        return { selected: id };
      }

      case 'undo':
        await cad.undo();
        return { ok: true };

      default:
        return { error: `unknown tool: ${name}` };
    }
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
}

// Dev-only handle for headless verification (no effect in prod).
const isDev = (import.meta as unknown as { env?: { DEV?: boolean } }).env?.DEV;
if (typeof window !== 'undefined' && isDev) {
  (window as unknown as { __aiTools: unknown }).__aiTools = {
    AI_TOOLS,
    executeTool,
    documentSnapshot,
    requiresApproval,
  };
}
