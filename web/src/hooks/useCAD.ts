import { useState, useCallback } from 'react';
import type { MeshGeometry } from './../lib/meshParsers';
import { useDocumentStore } from '../stores/documentStore';
import { offsetMesh, rotateMeshAboutCenter } from '../lib/meshTransforms';

// Timeouts (ms). Booleans use truck-shapeops, which can hang on curved solids;
// the short boolean timeout is what keeps a hang from wedging the app.
const CREATE_TIMEOUT = 30_000;
const MESH_TIMEOUT = 30_000;
const BOOLEAN_TIMEOUT = 8_000;
const INIT_TIMEOUT = 60_000;

// Indexed mesh as returned by the worker (one entry per unique vertex).
interface IndexedMesh {
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
}

type WorkerResponse = { reqId: number; result?: unknown; error?: string };

// Low-level worker ops, replayed verbatim to rebuild solids after a worker crash
// (kept in sync with the command history below — every full exec appends here).
type WorkerOp =
  | { method: 'create'; args: { id: string; kind: string; params: number[] } }
  | { method: 'recreate'; args: { id: string; kind: string; params: number[] } }
  | { method: 'boolean'; args: { id: string; op: string; targetId: string; toolId: string } }
  | { method: 'translate'; args: { id: string; dx: number; dy: number; dz: number } }
  | { method: 'rotate'; args: { id: string; ax: number; ay: number; az: number; angle: number } };

// ---------------------------------------------------------------------------
// Command history — the document's single source of truth.
//
// Every user mutation is a Command. Applying one mutates BOTH the WASM worker
// and the document store; the store is therefore a pure projection of the
// command list up to `pointer`. Undo/redo move the pointer and rebuild from
// scratch; save/load serialise/replay the list. This generalises the old
// op-log (which only ever drove worker-crash recovery).
// ---------------------------------------------------------------------------

interface SerializedMesh {
  positions: number[];
  normals: number[];
  vertexCount: number;
}

type RGB = [number, number, number];

type Command =
  | { kind: 'create'; id: string; prim: string; params: number[]; name: string; type: string; color: RGB }
  | { kind: 'boolean'; id: string; op: string; targetId: string; toolId: string; name: string; type: string; color: RGB }
  | { kind: 'import'; id: string; name: string; geometry: MeshGeometry; color: RGB }
  | { kind: 'translate'; id: string; d: [number, number, number] }
  | { kind: 'rotate'; id: string; axis: number; angle: number } // angle in radians
  | { kind: 'resize'; id: string; prim: string; params: number[] }
  | { kind: 'setProps'; id: string; patch: Record<string, unknown> }
  | { kind: 'delete'; id: string };

interface Pending {
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
  timer?: ReturnType<typeof setTimeout>;
}

// A pleasant CAD-ish palette; each new solid/import cycles to the next colour so
// assemblies are visually distinguishable. The chosen colour is baked into the
// command, so undo/redo/load reproduce it deterministically.
const PALETTE: RGB[] = [
  [0.55, 0.62, 0.72], // steel blue-grey
  [0.82, 0.52, 0.30], // copper
  [0.45, 0.62, 0.82], // sky blue
  [0.55, 0.72, 0.48], // sage green
  [0.80, 0.46, 0.52], // rose
  [0.74, 0.68, 0.40], // brass
  [0.58, 0.52, 0.74], // violet
  [0.40, 0.70, 0.68], // teal
];
let colorCounter = 0;
const nextColor = (): RGB => PALETTE[colorCounter++ % PALETTE.length];

// --- module-level CAD engine (single worker shared by every useCAD caller) ---
//
// useCAD is called from multiple components (App, Toolbar, ...). A per-hook
// worker would give each component its own empty document, so the engine lives
// at module scope and all hooks share it.

let worker: Worker | null = null;
const pending = new Map<number, Pending>();
let reqCounter = 0;
const opLog: WorkerOp[] = [];
let initPromise: Promise<unknown> | null = null;

// History state.
let history: Command[] = [];
let pointer = 0; // commands[0..pointer) are applied; [pointer..] are redoable.

// Serialise every mutation: dispatch/undo/redo/load each rebuild shared worker +
// store state across many awaits, so overlapping calls (rapid clicks, a gizmo
// commit landing mid-rebuild) would corrupt the pointer. The chain runs them
// strictly in order.
let chain: Promise<unknown> = Promise.resolve();
function enqueue<T>(fn: () => Promise<T>): Promise<T> {
  const run = chain.then(fn, fn);
  chain = run.then(
    () => {},
    () => {}
  );
  return run;
}

function syncFlags() {
  useDocumentStore.getState().setHistoryFlags(pointer > 0, pointer < history.length);
}

function attachHandlers(w: Worker) {
  w.onmessage = (e: MessageEvent<WorkerResponse>) => {
    const { reqId, result, error } = e.data;
    const p = pending.get(reqId);
    if (!p) return;
    if (p.timer) clearTimeout(p.timer);
    pending.delete(reqId);
    if (error) p.reject(new Error(error));
    else p.resolve(result);
  };
  w.onerror = (e) => console.error('CAD worker error:', e.message);
}

function makeWorker(): Worker {
  const w = new Worker(new URL('../wasm/cadWorker.ts', import.meta.url), {
    type: 'module',
  });
  attachHandlers(w);
  return w;
}

function ensureWorker(): Worker {
  if (!worker) worker = makeWorker();
  return worker;
}

// Kill a hung worker and rebuild solids by replaying the worker op-log. The hung
// op was never logged (we log only on success), so it is not replayed. The store
// is untouched — only the worker died, the document model is intact.
function restartWorker() {
  worker?.terminate();
  for (const [, p] of pending) {
    if (p.timer) clearTimeout(p.timer);
    p.reject(new Error('worker restarted'));
  }
  pending.clear();

  worker = makeWorker();
  for (const op of opLog) {
    const reqId = ++reqCounter;
    pending.set(reqId, { resolve: () => {}, reject: () => {} });
    worker.postMessage({ reqId, method: op.method, args: op.args });
  }
}

function call<T>(method: string, args: unknown, timeoutMs?: number): Promise<T> {
  const w = ensureWorker();
  const reqId = ++reqCounter;
  return new Promise<T>((resolve, reject) => {
    const timer = timeoutMs
      ? setTimeout(() => {
          pending.delete(reqId);
          reject(new Error('timeout'));
          restartWorker();
        }, timeoutMs)
      : undefined;
    pending.set(reqId, { resolve: resolve as (v: unknown) => void, reject, timer });
    w.postMessage({ reqId, method, args });
  });
}

function init(): Promise<unknown> {
  if (!initPromise) {
    ensureWorker();
    initPromise = call('init', undefined, INIT_TIMEOUT);
  }
  return initPromise;
}

// --- de-index + low-level worker ops (each appends to opLog on success) -------

function deindex(md: IndexedMesh): MeshGeometry {
  const idx = md.indices;
  const n = idx.length;
  const positions = new Float32Array(n * 3);
  const normals = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    const v = idx[i];
    positions[i * 3] = md.positions[v * 3];
    positions[i * 3 + 1] = md.positions[v * 3 + 1];
    positions[i * 3 + 2] = md.positions[v * 3 + 2];
    normals[i * 3] = md.normals[v * 3];
    normals[i * 3 + 1] = md.normals[v * 3 + 1];
    normals[i * 3 + 2] = md.normals[v * 3 + 2];
  }
  return { positions, normals, vertexCount: n };
}

async function getMesh(featureId: string): Promise<MeshGeometry> {
  return deindex(await call<IndexedMesh>('mesh', { id: featureId }, MESH_TIMEOUT));
}

async function wCreate(id: string, kind: string, params: number[]) {
  await call('create', { id, kind, params }, CREATE_TIMEOUT);
  opLog.push({ method: 'create', args: { id, kind, params } });
}

async function wRecreate(id: string, kind: string, params: number[]) {
  await call('recreate', { id, kind, params }, CREATE_TIMEOUT);
  opLog.push({ method: 'recreate', args: { id, kind, params } });
}

async function wBoolean(id: string, op: string, targetId: string, toolId: string) {
  await call('boolean', { id, op, targetId, toolId }, BOOLEAN_TIMEOUT);
  opLog.push({ method: 'boolean', args: { id, op, targetId, toolId } });
}

async function wTranslate(id: string, dx: number, dy: number, dz: number): Promise<MeshGeometry> {
  const md = await call<IndexedMesh>('translate', { id, dx, dy, dz }, MESH_TIMEOUT);
  opLog.push({ method: 'translate', args: { id, dx, dy, dz } });
  return deindex(md);
}

async function wRotate(
  id: string, ax: number, ay: number, az: number, angle: number
): Promise<MeshGeometry> {
  const md = await call<IndexedMesh>('rotate', { id, ax, ay, az, angle }, MESH_TIMEOUT);
  opLog.push({ method: 'rotate', args: { id, ax, ay, az, angle } });
  return deindex(md);
}

const axisVec = (axis: number): [number, number, number] => [
  axis === 0 ? 1 : 0,
  axis === 1 ? 1 : 0,
  axis === 2 ? 1 : 0,
];

// --- command execution (the only thing that mutates worker + store) ----------

async function execCommand(cmd: Command): Promise<void> {
  const store = useDocumentStore.getState();
  switch (cmd.kind) {
    case 'create': {
      await wCreate(cmd.id, cmd.prim, cmd.params);
      const geom = await getMesh(cmd.id);
      store.addMeshFeature(
        { id: cmd.id, name: cmd.name, type: cmd.type, params: cmd.params, color: cmd.color },
        geom
      );
      break;
    }
    case 'boolean': {
      await wBoolean(cmd.id, cmd.op, cmd.targetId, cmd.toolId);
      const geom = await getMesh(cmd.id);
      store.removeFeature(cmd.targetId);
      store.removeFeature(cmd.toolId);
      store.addMeshFeature({ id: cmd.id, name: cmd.name, type: cmd.type, color: cmd.color }, geom);
      break;
    }
    case 'import': {
      store.addMeshFeature(
        { id: cmd.id, name: cmd.name, type: 'Import', color: cmd.color },
        cmd.geometry
      );
      break;
    }
    case 'translate': {
      const f = store.features.find((x) => x.id === cmd.id);
      if (!f) break;
      const cur = f.position ?? [0, 0, 0];
      const next: [number, number, number] = [
        cur[0] + cmd.d[0], cur[1] + cmd.d[1], cur[2] + cmd.d[2],
      ];
      const geom =
        f.type === 'Import'
          ? offsetMesh(store.meshes[cmd.id], cmd.d)
          : await wTranslate(cmd.id, cmd.d[0], cmd.d[1], cmd.d[2]);
      store.setFeatureMesh(cmd.id, geom, { position: next });
      break;
    }
    case 'rotate': {
      const f = store.features.find((x) => x.id === cmd.id);
      if (!f) break;
      const cur = f.rotation ?? [0, 0, 0];
      const next: [number, number, number] = [cur[0], cur[1], cur[2]];
      next[cmd.axis] += (cmd.angle * 180) / Math.PI;
      const av = axisVec(cmd.axis);
      const geom =
        f.type === 'Import'
          ? rotateMeshAboutCenter(store.meshes[cmd.id], cmd.axis, cmd.angle)
          : await wRotate(cmd.id, av[0], av[1], av[2], cmd.angle);
      store.setFeatureMesh(cmd.id, geom, { rotation: next });
      break;
    }
    case 'resize': {
      // Rebuild the solid at its new size, then re-apply the current placement
      // so resizing keeps the part where it sits.
      const f = store.features.find((x) => x.id === cmd.id);
      if (!f) break;
      await wRecreate(cmd.id, cmd.prim, cmd.params);
      const pos = f.position ?? [0, 0, 0];
      const rot = f.rotation ?? [0, 0, 0];
      if (pos[0] || pos[1] || pos[2]) await wTranslate(cmd.id, pos[0], pos[1], pos[2]);
      for (let a = 0; a < 3; a++) {
        if (rot[a]) {
          const av = axisVec(a);
          await wRotate(cmd.id, av[0], av[1], av[2], (rot[a] * Math.PI) / 180);
        }
      }
      const geom = await getMesh(cmd.id);
      store.setFeatureMesh(cmd.id, geom, { params: cmd.params });
      break;
    }
    case 'setProps': {
      store.updateFeature(cmd.id, cmd.patch);
      break;
    }
    case 'delete': {
      store.removeFeature(cmd.id);
      break;
    }
  }
}

// Apply a brand-new command: drop any redo tail, execute, record.
function dispatch(cmd: Command): Promise<void> {
  return enqueue(async () => {
    history = history.slice(0, pointer);
    await execCommand(cmd);
    history.push(cmd);
    pointer = history.length;
    syncFlags();
  });
}

// Rebuild worker + store from scratch by replaying commands[0..pointer).
async function rebuild(): Promise<void> {
  await call('reset', undefined, INIT_TIMEOUT);
  opLog.length = 0;
  useDocumentStore.getState().resetDocumentState();
  for (let i = 0; i < pointer; i++) await execCommand(history[i]);
  syncFlags();
}

function undo(): Promise<void> {
  return enqueue(async () => {
    if (pointer === 0) return;
    pointer -= 1;
    await rebuild();
  });
}

function redo(): Promise<void> {
  return enqueue(async () => {
    if (pointer >= history.length) return;
    pointer += 1;
    await rebuild();
  });
}

// --- serialisation -----------------------------------------------------------

function serializeCommand(cmd: Command): unknown {
  if (cmd.kind === 'import') {
    const g = cmd.geometry;
    const mesh: SerializedMesh = {
      positions: Array.from(g.positions),
      normals: Array.from(g.normals),
      vertexCount: g.vertexCount,
    };
    return { ...cmd, geometry: mesh };
  }
  return cmd;
}

function deserializeCommand(raw: Record<string, unknown>): Command {
  if (raw.kind === 'import') {
    const m = raw.geometry as SerializedMesh;
    const geometry: MeshGeometry = {
      positions: new Float32Array(m.positions),
      normals: new Float32Array(m.normals),
      vertexCount: m.vertexCount,
    };
    return { ...(raw as object), geometry } as Command;
  }
  return raw as unknown as Command;
}

function serialize(): string {
  const { documentName } = useDocumentStore.getState();
  return JSON.stringify({
    format: 'rcad',
    version: 1,
    name: documentName,
    commands: history.slice(0, pointer).map(serializeCommand),
  });
}

function loadDoc(json: string): Promise<void> {
  return enqueue(async () => {
    const data = JSON.parse(json);
    if (data.format !== 'rcad') throw new Error('Not an rCAD document');
    const cmds: Command[] = (data.commands as Record<string, unknown>[]).map(deserializeCommand);
    await call('reset', undefined, INIT_TIMEOUT);
    opLog.length = 0;
    useDocumentStore.getState().resetDocumentState();
    history = cmds;
    pointer = cmds.length;
    for (let i = 0; i < pointer; i++) await execCommand(history[i]);
    useDocumentStore.setState({ documentName: data.name || 'Untitled', isDirty: false });
    syncFlags();
  });
}

function newDoc(): Promise<void> {
  return enqueue(async () => {
    await call('reset', undefined, INIT_TIMEOUT);
    opLog.length = 0;
    history = [];
    pointer = 0;
    useDocumentStore.getState().resetDocumentState();
    useDocumentStore.setState({ documentName: 'Untitled', isDirty: false });
    syncFlags();
  });
}

// --- public CAD operations (stable, shared object) ---------------------------

const PRIM_LABEL: Record<string, string> = {
  box: 'Box',
  cylinder: 'Cylinder',
  sphere: 'Sphere',
  cone: 'Cone',
};
const BOOL_LABEL: Record<string, string> = {
  union: 'Union',
  subtract: 'Subtract',
  intersect: 'Intersect',
};

export const cad = {
  // Creation
  addPrimitive: async (prim: string, params: number[]) => {
    const id = crypto.randomUUID();
    const label = PRIM_LABEL[prim] ?? prim;
    await dispatch({ kind: 'create', id, prim, params, name: label, type: label, color: nextColor() });
    return id;
  },
  applyBoolean: async (op: string, targetId: string, toolId: string) => {
    const id = crypto.randomUUID();
    const label = BOOL_LABEL[op] ?? op;
    await dispatch({ kind: 'boolean', id, op, targetId, toolId, name: label, type: label, color: nextColor() });
    return id;
  },
  importMesh: async (name: string, geometry: MeshGeometry) => {
    const id = crypto.randomUUID();
    await dispatch({ kind: 'import', id, name, geometry, color: nextColor() });
    return id;
  },

  // Editing
  moveFeature: (id: string, dx: number, dy: number, dz: number) =>
    dispatch({ kind: 'translate', id, d: [dx, dy, dz] }),
  spinFeature: (id: string, axis: number, angle: number) =>
    dispatch({ kind: 'rotate', id, axis, angle }),
  resizeFeature: (id: string, prim: string, params: number[]) =>
    dispatch({ kind: 'resize', id, prim, params }),
  setFeatureProps: (id: string, patch: Record<string, unknown>) =>
    dispatch({ kind: 'setProps', id, patch }),
  deleteFeature: (id: string) => dispatch({ kind: 'delete', id }),

  // History + document
  undo,
  redo,
  serialize,
  loadDoc,
  newDoc,

  // Queries / export
  getMesh,
  exportStl: (featureId: string, binary = true) =>
    call<Uint8Array>('export', { id: featureId, format: 'stl', binary }, MESH_TIMEOUT),
  exportObj: (featureId: string) =>
    call<string>('export', { id: featureId, format: 'obj', binary: false }, MESH_TIMEOUT),
};

export type Cad = typeof cad;

// Dev-only handle for debugging and headless verification (no effect in prod).
const isDev = (import.meta as unknown as { env?: { DEV?: boolean } }).env?.DEV;
if (typeof window !== 'undefined' && isDev) {
  (window as unknown as { __rcad: unknown }).__rcad = { cad, store: useDocumentStore };
}

export function useCAD() {
  const [isInitialized, setIsInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const initialize = useCallback(async () => {
    try {
      await init();
      setIsInitialized(true);
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to initialize CAD';
      setError(message);
      console.error('CAD initialization failed:', e);
    }
  }, []);

  // The cad object is module-level and always usable (the worker lazily inits),
  // so every component shares one document regardless of init ordering.
  return { initialize, isInitialized, error, cad };
}
