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
import { rotAxis } from './meshTransforms';

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
    name: 'make_bolt',
    description:
      'Create a bolt as a single part: a cylindrical shaft with a wider cylindrical head, standing along +Y with the shaft bottom at the origin. All dimensions in mm; head defaults ~1.8× diameter wide and ~0.8× tall.',
    input_schema: {
      type: 'object',
      properties: { diameter: num, length: num, head_diameter: num, head_height: num },
      required: ['diameter', 'length'],
    },
  },
  {
    name: 'make_revolve',
    description:
      'Create a solid of revolution (lathe) as a single part from a profile: a list of {y, radius} points in mm, increasing in y. Each consecutive pair becomes a frustum revolved around the vertical (Y) axis. Use for vases, bottles, pillars, knobs, wheels, nozzles, cups. A radius of 0 makes a pointed end.',
    input_schema: {
      type: 'object',
      properties: {
        profile: {
          type: 'array',
          description: 'ordered [{y, radius}, …], at least 2 points',
          items: {
            type: 'object',
            properties: { y: num, radius: num },
            required: ['y', 'radius'],
          },
        },
      },
      required: ['profile'],
    },
  },
  {
    name: 'make_gear',
    description:
      'Create a spur gear as a single part (a disc with straight teeth around it), standing along +Y and centred at the origin. `teeth` is the tooth count; `module` (mm) sets tooth size — pitch diameter = module × teeth; `thickness` is the face width. (No centre bore — the kernel can\'t cut holes.)',
    input_schema: {
      type: 'object',
      properties: { teeth: num, module: num, thickness: num },
      required: ['teeth'],
    },
  },
  {
    name: 'simulate_physics',
    description:
      'Run a Newton rigid-body drop test on the current model and start playback for the user. Parts marked fixed are anchors; the rest fall under gravity. Returns, per part, its start/end height, whether it settled, and how far it moved — use this to answer "does it stand / stay?" and iterate.',
    input_schema: { type: 'object', properties: {} },
  },
  {
    name: 'line_status',
    description:
      'Read the live process line: flow state, whether the belt is running, the part position, belt speed, station dwell, throughput count, and which station is busy.',
    input_schema: { type: 'object', properties: {} },
  },
  {
    name: 'line_control',
    description: 'Supervisory control of the process line: start, stop, or reset the conveyor.',
    input_schema: {
      type: 'object',
      properties: { command: { type: 'string', enum: ['start', 'stop', 'reset'] } },
      required: ['command'],
    },
  },
  {
    name: 'set_dwell',
    description: 'Set how long each station processes a part, in seconds.',
    input_schema: { type: 'object', properties: { seconds: num }, required: ['seconds'] },
  },
  {
    name: 'set_speed',
    description: 'Set the conveyor belt speed in mm/s.',
    input_schema: { type: 'object', properties: { mm_per_s: num }, required: ['mm_per_s'] },
  },
  {
    name: 'set_fail_rate',
    description:
      'Set the mock vision inspection defect rate at station 2, in percent (0–100). Parts failing inspection are counted as rejects.',
    input_schema: { type: 'object', properties: { percent: num }, required: ['percent'] },
  },
  {
    name: 'add_rule',
    description:
      'Add an automation rule: when a live tag crosses a value, run an action. Control actions (stop/start/set_speed/set_dwell/set_fail_rate) fire once per crossing; alarm is shown while the condition holds. Useful tags: reject.count, throughput.count, conveyor.speed, conveyor.running, inspection.fail. Example: reject.count > 5 → stop with an alarm.',
    input_schema: {
      type: 'object',
      properties: {
        when_tag: { type: 'string', description: 'tag name, e.g. reject.count' },
        op: { type: 'string', enum: ['>', '<', '>=', '<=', '==', '!='] },
        value: num,
        action: { type: 'string', enum: ['stop', 'start', 'set_speed', 'set_dwell', 'set_fail_rate', 'alarm'] },
        arg: { type: 'number', description: 'argument for set_* actions' },
        message: { type: 'string', description: 'alarm message' },
      },
      required: ['when_tag', 'op', 'value', 'action'],
    },
  },
  {
    name: 'list_rules',
    description: 'List the current automation rules.',
    input_schema: { type: 'object', properties: {} },
  },
  {
    name: 'clear_rules',
    description: 'Remove all automation rules.',
    input_schema: { type: 'object', properties: {} },
  },
  {
    name: 'get_flow_graph',
    description: 'Read the current process line as a flow graph (its steps and transitions).',
    input_schema: { type: 'object', properties: {} },
  },
  {
    name: 'set_flow_graph',
    description:
      'Design the process line as a multi-step flow graph and restart it. Nodes are steps of kind: source (parts enter), process (a machining/assembly station), inspect (a QC check that yields pass/fail), or sink (parts exit, e.g. pack or reject). Edges connect steps; an edge leaving an inspect step may set when to "pass" or "fail" to route that branch (else it is unconditional). Cycles are allowed — send failed parts back to an earlier step for rework. pos is mm along the belt (left→right); y is the lane (0 = main belt, -1 = a branch below). Needs at least one source and one sink. Example: load(source)→mill(process)→qc(inspect), qc pass→pack(sink), qc fail→rework(process)→back to mill.',
    input_schema: {
      type: 'object',
      properties: {
        nodes: {
          type: 'array',
          description: 'the process steps',
          items: {
            type: 'object',
            properties: {
              id: { type: 'string', description: 'unique step id' },
              kind: { type: 'string', enum: ['source', 'process', 'inspect', 'sink'] },
              pos: { type: 'number', description: 'mm along the belt (left→right)' },
              y: { type: 'number', description: 'lane: 0 main belt, -1 branch below' },
              dwell: { type: 'number', description: 'processing seconds (process/inspect)' },
              fail_rate: { type: 'number', description: 'inspect defect percent 0–100' },
              verdict_tag: {
                type: 'string',
                description:
                  'inspect only: route on this live external tag when present (e.g. "inspection.result" from the real vision camera), else the mock',
              },
              label: { type: 'string' },
            },
            required: ['id', 'kind'],
          },
        },
        edges: {
          type: 'array',
          description: 'the transitions between steps',
          items: {
            type: 'object',
            properties: {
              from: { type: 'string' },
              to: { type: 'string' },
              when: { type: 'string', enum: ['pass', 'fail', 'always'] },
            },
            required: ['from', 'to'],
          },
        },
        spawn_interval: { type: 'number', description: 'seconds between new parts (default 3)' },
      },
      required: ['nodes', 'edges'],
    },
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

// Rotate a mesh about a world axis (through the origin) then translate — used to
// orient/position the primitive pieces of a composite macro before merging.
function placedMesh(
  g: MeshGeometry,
  axisIdx: number,
  angle: number,
  t: [number, number, number]
): MeshGeometry {
  const n = g.vertexCount;
  const positions = new Float32Array(n * 3);
  const normals = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    const [px, py, pz] = angle
      ? rotAxis(axisIdx, angle, g.positions[i * 3], g.positions[i * 3 + 1], g.positions[i * 3 + 2])
      : [g.positions[i * 3], g.positions[i * 3 + 1], g.positions[i * 3 + 2]];
    positions[i * 3] = px + t[0];
    positions[i * 3 + 1] = py + t[1];
    positions[i * 3 + 2] = pz + t[2];
    const [nx, ny, nz] = angle
      ? rotAxis(axisIdx, angle, g.normals[i * 3], g.normals[i * 3 + 1], g.normals[i * 3 + 2])
      : [g.normals[i * 3], g.normals[i * 3 + 1], g.normals[i * 3 + 2]];
    normals[i * 3] = nx;
    normals[i * 3 + 1] = ny;
    normals[i * 3 + 2] = nz;
  }
  return { positions, normals, vertexCount: n };
}

// Copy a mesh translated out to `radius` along +X then swung to `theta` about Y
// — places gear teeth evenly around the axis.
function revolvedCopy(g: MeshGeometry, radius: number, theta: number): MeshGeometry {
  const n = g.vertexCount;
  const positions = new Float32Array(n * 3);
  const normals = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    const [x, y, z] = rotAxis(1, theta, g.positions[i * 3] + radius, g.positions[i * 3 + 1], g.positions[i * 3 + 2]);
    positions[i * 3] = x;
    positions[i * 3 + 1] = y;
    positions[i * 3 + 2] = z;
    const [nx, ny, nz] = rotAxis(1, theta, g.normals[i * 3], g.normals[i * 3 + 1], g.normals[i * 3 + 2]);
    normals[i * 3] = nx;
    normals[i * 3 + 1] = ny;
    normals[i * 3 + 2] = nz;
  }
  return { positions, normals, vertexCount: n };
}

// Concatenate de-indexed meshes into one (no index remap — each is flat).
function mergeGeoms(list: MeshGeometry[]): MeshGeometry {
  const total = list.reduce((s, g) => s + g.vertexCount, 0);
  const positions = new Float32Array(total * 3);
  const normals = new Float32Array(total * 3);
  let o = 0;
  for (const g of list) {
    positions.set(g.positions, o);
    normals.set(g.normals, o);
    o += g.vertexCount * 3;
  }
  return { positions, normals, vertexCount: total };
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

      case 'make_bolt': {
        const d = Number(input.diameter) || 0;
        const L = Number(input.length) || 0;
        if (d <= 0 || L <= 0) return { error: 'diameter and length must be > 0' };
        const hd = Number(input.head_diameter) || d * 1.8;
        const hh = Number(input.head_height) || d * 0.8;
        const shaft0 = await cad.buildPrimitiveMesh('cylinder', [d / 2, L]);
        const head0 = await cad.buildPrimitiveMesh('cylinder', [hd / 2, hh]);
        // Cylinders build along Z; rotate +90° about X to stand along Y.
        const shaft = placedMesh(shaft0, 0, Math.PI / 2, [0, L / 2, 0]); // bottom at 0
        const head = placedMesh(head0, 0, Math.PI / 2, [0, L + hh / 2, 0]); // on top
        const id = await cad.importMesh(`Bolt M${d}`, mergeGeoms([shaft, head]), [0.62, 0.63, 0.66]);
        return { id, bbox: bboxOf(id) };
      }

      case 'make_revolve': {
        const raw = Array.isArray(input.profile) ? (input.profile as Array<{ y: unknown; radius: unknown }>) : [];
        const pts = raw
          .map((p) => ({ y: Number(p.y), radius: Math.max(0, Number(p.radius)) }))
          .filter((p) => Number.isFinite(p.y) && Number.isFinite(p.radius));
        if (pts.length < 2) return { error: 'profile needs at least 2 {y,radius} points' };
        pts.sort((a, b) => a.y - b.y);
        const parts: MeshGeometry[] = [];
        for (let i = 0; i < pts.length - 1; i++) {
          const a = pts[i];
          const b = pts[i + 1];
          const h = b.y - a.y;
          if (h <= 1e-6 || (a.radius <= 1e-6 && b.radius <= 1e-6)) continue;
          const frustum = await cad.buildPrimitiveMesh('cone', [a.radius, b.radius, h]);
          // Cone builds along Z (bottomR at -h/2); rotate -90° about X so the
          // bottom radius ends up lower, then place its base at a.y.
          parts.push(placedMesh(frustum, 0, -Math.PI / 2, [0, a.y + h / 2, 0]));
        }
        if (!parts.length) return { error: 'profile produced no solid sections' };
        const id = await cad.importMesh('Revolve', mergeGeoms(parts));
        return { id, bbox: bboxOf(id) };
      }

      case 'make_gear': {
        const N = Math.max(4, Math.min(200, Math.round(Number(input.teeth) || 0)));
        if (!N) return { error: 'teeth is required' };
        const m = Math.max(0.2, Number(input.module) || 2);
        const t = Math.max(1, Number(input.thickness) || m * 3);
        const rp = (m * N) / 2; // pitch radius
        const ro = rp + m; // outer (addendum) radius
        const bodyR = rp - 0.25 * m; // disc radius, just below pitch
        const overlap = 0.6 * m;
        const radialLen = ro - (bodyR - overlap); // tooth reaches into the disc
        const R = ro - radialLen / 2; // tooth centre radius
        const toothW = ((Math.PI * m) / 2) * 0.9; // ~half the circular pitch
        // Disc: cylinder (built along Z) stood up along Y.
        const disc = placedMesh(await cad.buildPrimitiveMesh('cylinder', [bodyR, t]), 0, Math.PI / 2, [0, 0, 0]);
        // One tooth box, reused around the circle.
        const tooth = await cad.buildPrimitiveMesh('box', [radialLen, t, toothW]);
        const parts: MeshGeometry[] = [disc];
        for (let i = 0; i < N; i++) parts.push(revolvedCopy(tooth, R, (i * 2 * Math.PI) / N));
        const id = await cad.importMesh(`Gear ${N}T`, mergeGeoms(parts), [0.6, 0.62, 0.66]);
        return {
          id,
          bbox: bboxOf(id),
          pitch_diameter: Math.round(2 * rp * 100) / 100,
          outer_diameter: Math.round(2 * ro * 100) / 100,
        };
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

      case 'line_status': {
        const res = await fetch('/api/telemetry/status');
        const d = await res.json();
        const t = d.tags ?? {};
        return {
          flow: d.flow,
          enabled: t['conveyor.enabled'], // operator on/off
          belt_moving: t['conveyor.running'],
          position: t['conveyor.position'],
          speed: t['conveyor.speed'],
          dwell: t['station.dwell'],
          throughput_passed: t['throughput.count'],
          rejected: t['reject.count'],
          inspection_last: t['inspection.result'],
          inspection_fail_rate: t['inspection.fail_rate'],
          station1_busy: t['station1.busy'],
          station2_busy: t['station2.busy'],
        };
      }

      case 'line_control': {
        const res = await fetch('/api/telemetry/control', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ command: String(input.command) }),
        });
        return await res.json();
      }

      case 'set_dwell': {
        const res = await fetch('/api/telemetry/control', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ command: 'config', dwell: Math.max(0, Number(input.seconds) || 0) }),
        });
        return await res.json();
      }

      case 'set_speed': {
        const res = await fetch('/api/telemetry/control', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ command: 'config', speed: Math.max(0, Number(input.mm_per_s) || 0) }),
        });
        return await res.json();
      }

      case 'set_fail_rate': {
        const res = await fetch('/api/telemetry/control', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ command: 'config', fail_rate: Math.max(0, Math.min(100, Number(input.percent) || 0)) }),
        });
        return await res.json();
      }

      case 'add_rule': {
        const rule = {
          tag: String(input.when_tag),
          op: String(input.op),
          value: Number(input.value) || 0,
          action: String(input.action),
          arg: Number(input.arg) || 0,
          message: String(input.message ?? ''),
        };
        const res = await fetch('/api/telemetry/rules', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ rules: [rule], mode: 'append' }),
        });
        return { added: rule, ...(await res.json()) };
      }

      case 'list_rules': {
        return await (await fetch('/api/telemetry/rules')).json();
      }

      case 'clear_rules': {
        const res = await fetch('/api/telemetry/rules', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ rules: [], mode: 'replace' }),
        });
        return await res.json();
      }

      case 'get_flow_graph': {
        return await (await fetch('/api/telemetry/graph')).json();
      }

      case 'set_flow_graph': {
        const rawNodes = Array.isArray(input.nodes) ? input.nodes : [];
        const rawEdges = Array.isArray(input.edges) ? input.edges : [];
        // Build clean nodes; auto-space along the belt when pos is omitted so a
        // graph still lays out sensibly (routing is by edges, not position).
        const nodes = rawNodes.map((n, i) => {
          const o = (n ?? {}) as Record<string, unknown>;
          return {
            id: String(o.id),
            kind: String(o.kind),
            pos: o.pos != null ? Number(o.pos) : i * 90,
            y: o.y != null ? Number(o.y) : 0,
            ...(o.dwell != null ? { dwell: Number(o.dwell) } : {}),
            ...(o.fail_rate != null ? { fail_rate: Number(o.fail_rate) } : {}),
            ...(o.verdict_tag != null ? { verdict_tag: String(o.verdict_tag) } : {}),
            ...(o.label != null ? { label: String(o.label) } : {}),
          };
        });
        const edges = rawEdges.map((e) => {
          const o = (e ?? {}) as Record<string, unknown>;
          const when = o.when == null || o.when === 'always' ? null : String(o.when);
          return { from: String(o.from), to: String(o.to), when };
        });
        if (!nodes.some((n) => n.kind === 'source') || !nodes.some((n) => n.kind === 'sink')) {
          return { ok: false, error: 'graph needs at least one source and one sink node' };
        }
        const graph = {
          nodes,
          edges,
          spawn_interval: input.spawn_interval != null ? Number(input.spawn_interval) : 3,
        };
        const res = await fetch('/api/telemetry/graph', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ graph }),
        });
        return { steps: nodes.length, transitions: edges.length, ...(await res.json()) };
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
