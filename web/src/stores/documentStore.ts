import { create } from 'zustand';

export interface Feature {
  id: string;
  name: string;
  type: string;
  suppressed?: boolean;
}

interface DocumentState {
  // Document info
  documentName: string;
  documentId: string | null;
  isDirty: boolean;

  // Features
  features: Feature[];
  selectedFeature: string | null;

  // History
  canUndo: boolean;
  canRedo: boolean;

  // Actions
  setDocument: (name: string, id: string) => void;
  addFeature: (feature: Feature) => void;
  removeFeature: (id: string) => void;
  updateFeature: (id: string, updates: Partial<Feature>) => void;
  selectFeature: (id: string | null) => void;
  undo: () => void;
  redo: () => void;
  markDirty: () => void;
  markClean: () => void;
}

export const useDocumentStore = create<DocumentState>((set, get) => ({
  documentName: 'Untitled',
  documentId: null,
  isDirty: false,

  features: [],
  selectedFeature: null,

  canUndo: false,
  canRedo: false,

  setDocument: (name, id) =>
    set({
      documentName: name,
      documentId: id,
      isDirty: false,
      features: [],
      selectedFeature: null,
    }),

  addFeature: (feature) =>
    set((state) => ({
      features: [...state.features, feature],
      selectedFeature: feature.id,
      isDirty: true,
      canUndo: true,
    })),

  removeFeature: (id) =>
    set((state) => ({
      features: state.features.filter((f) => f.id !== id),
      selectedFeature:
        state.selectedFeature === id ? null : state.selectedFeature,
      isDirty: true,
      canUndo: true,
    })),

  updateFeature: (id, updates) =>
    set((state) => ({
      features: state.features.map((f) =>
        f.id === id ? { ...f, ...updates } : f
      ),
      isDirty: true,
      canUndo: true,
    })),

  selectFeature: (id) => set({ selectedFeature: id }),

  undo: () => {
    // In a real implementation, would call into WASM to undo
    console.log('Undo');
    set((state) => ({ canRedo: true }));
  },

  redo: () => {
    // In a real implementation, would call into WASM to redo
    console.log('Redo');
  },

  markDirty: () => set({ isDirty: true }),
  markClean: () => set({ isDirty: false }),
}));
