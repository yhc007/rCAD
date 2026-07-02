import { create } from 'zustand';
import type { MeshGeometry } from '../lib/meshParsers';
import type { Simulation } from '../lib/physics';

export interface Feature {
  id: string;
  name: string;
  type: string;
  suppressed?: boolean;
  // Accumulated placement (the geometry is actually moved/rotated, not just a
  // render transform), so booleans/physics use the real pose. Rotation is the
  // running per-axis input in degrees (not a canonical Euler decomposition).
  position?: [number, number, number];
  rotation?: [number, number, number];
  // Primitive dimensions, in creation order (box [w,h,d], cylinder [r,h],
  // sphere [r], cone [br,tr,h]). Drives the editable Dimensions panel; absent
  // for imports/booleans (no parametric form).
  params?: number[];
  // Base material color as linear RGB in 0..1 (assigned from a palette on
  // creation; editable in the Material panel). Drives the renderer per mesh.
  color?: [number, number, number];
  // Physics properties (used by the Newton simulation)
  fixed?: boolean; // static anchor instead of a falling body
  mass?: number; // 0 = auto-compute from shape
}

interface DocumentState {
  // Document info
  documentName: string;
  documentId: string | null;
  isDirty: boolean;

  // Features
  features: Feature[];
  selectedFeature: string | null;
  // Ordered multi-selection (e.g. picking target + tool for a boolean op)
  selectedFeatures: string[];

  // Renderable geometry keyed by feature id (imported meshes, etc.)
  meshes: Record<string, MeshGeometry>;

  // History
  canUndo: boolean;
  canRedo: boolean;

  // Transient user-facing notice (e.g. a cancelled boolean)
  notice: string | null;

  // Physics simulation (precomputed frames played back in the viewport).
  // playNonce bumps to (re)start playback from the first frame.
  simulation: Simulation | null;
  playing: boolean;
  currentFrame: number;
  playNonce: number;

  // Actions
  setNotice: (notice: string | null) => void;
  startSimulation: (sim: Simulation) => void;
  replaySimulation: () => void;
  clearSimulation: () => void;
  setPlaying: (playing: boolean) => void;
  setCurrentFrame: (frame: number) => void;
  setDocument: (name: string, id: string) => void;
  addFeature: (feature: Feature) => void;
  addMeshFeature: (feature: Feature, geometry: MeshGeometry) => void;
  setFeatureMesh: (
    id: string,
    geometry: MeshGeometry,
    patch: Partial<Feature>
  ) => void;
  removeFeature: (id: string) => void;
  updateFeature: (id: string, updates: Partial<Feature>) => void;
  selectFeature: (id: string | null) => void;
  toggleSelectFeature: (id: string) => void;
  // History flags are owned by the command-history layer (useCAD); these just
  // mirror it into the UI.
  setHistoryFlags: (canUndo: boolean, canRedo: boolean) => void;
  // Wipe features/meshes/selection/sim without touching documentName/history —
  // used by the history layer before replaying commands from scratch.
  resetDocumentState: () => void;
  markDirty: () => void;
  markClean: () => void;
}

export const useDocumentStore = create<DocumentState>((set) => ({
  documentName: 'Untitled',
  documentId: null,
  isDirty: false,

  features: [],
  selectedFeature: null,
  selectedFeatures: [],

  meshes: {},

  canUndo: false,
  canRedo: false,

  notice: null,
  simulation: null,
  playing: false,
  currentFrame: 0,
  playNonce: 0,

  setNotice: (notice) => set({ notice }),

  startSimulation: (sim) =>
    set((state) => ({
      simulation: sim,
      currentFrame: 0,
      playing: true,
      playNonce: state.playNonce + 1,
    })),
  replaySimulation: () =>
    set((state) => ({ currentFrame: 0, playing: true, playNonce: state.playNonce + 1 })),
  clearSimulation: () => set({ simulation: null, playing: false, currentFrame: 0 }),
  setPlaying: (playing) => set({ playing }),
  setCurrentFrame: (frame) => set({ currentFrame: frame }),

  setDocument: (name, id) =>
    set({
      documentName: name,
      documentId: id,
      isDirty: false,
      features: [],
      selectedFeature: null,
      selectedFeatures: [],
      meshes: {},
      simulation: null,
    }),

  addFeature: (feature) =>
    set((state) => ({
      features: [...state.features, feature],
      selectedFeature: feature.id,
      selectedFeatures: [feature.id],
      isDirty: true,
    })),

  addMeshFeature: (feature, geometry) =>
    set((state) => ({
      features: [...state.features, feature],
      meshes: { ...state.meshes, [feature.id]: geometry },
      selectedFeature: feature.id,
      selectedFeatures: [feature.id],
      isDirty: true,
      simulation: null, // scene changed → drop stale sim
    })),

  setFeatureMesh: (id, geometry, patch) =>
    set((state) => ({
      meshes: { ...state.meshes, [id]: geometry },
      features: state.features.map((f) => (f.id === id ? { ...f, ...patch } : f)),
      isDirty: true,
      simulation: null,
    })),

  removeFeature: (id) =>
    set((state) => {
      const meshes = { ...state.meshes };
      delete meshes[id];
      return {
        features: state.features.filter((f) => f.id !== id),
        meshes,
        selectedFeature:
          state.selectedFeature === id ? null : state.selectedFeature,
        selectedFeatures: state.selectedFeatures.filter((f) => f !== id),
        isDirty: true,
        simulation: null,
      };
    }),

  updateFeature: (id, updates) =>
    set((state) => ({
      features: state.features.map((f) =>
        f.id === id ? { ...f, ...updates } : f
      ),
      isDirty: true,
      simulation: null, // properties changed → drop stale sim
    })),

  selectFeature: (id) =>
    set({ selectedFeature: id, selectedFeatures: id ? [id] : [] }),

  toggleSelectFeature: (id) =>
    set((state) => {
      const has = state.selectedFeatures.includes(id);
      const selectedFeatures = has
        ? state.selectedFeatures.filter((f) => f !== id)
        : [...state.selectedFeatures, id];
      return {
        selectedFeatures,
        selectedFeature: selectedFeatures[selectedFeatures.length - 1] ?? null,
      };
    }),

  setHistoryFlags: (canUndo, canRedo) => set({ canUndo, canRedo }),

  resetDocumentState: () =>
    set({
      features: [],
      meshes: {},
      selectedFeature: null,
      selectedFeatures: [],
      simulation: null,
      playing: false,
      currentFrame: 0,
    }),

  markDirty: () => set({ isDirty: true }),
  markClean: () => set({ isDirty: false }),
}));
