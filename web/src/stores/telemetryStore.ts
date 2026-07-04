import { create } from 'zustand';

// Shared process-twin telemetry: a single WebSocket to /api/telemetry/ws whose
// latest snapshot is read by both the Process Twin panel (2D + control) and the
// 3D Canvas (which binds the part's position to the conveyor tag).

export interface FlowNode {
  id: string;
  kind: string; // source | process | inspect | sink
  pos: number;
  y?: number;
  verdict_tag?: string | null; // inspect: routes on this external tag (e.g. vision)
  label?: string | null;
}
export interface FlowEdge {
  from: string;
  to: string;
  when?: string | null; // 'pass' | 'fail' | null
}
export interface FlowPart {
  id: number;
  x: number;
  y?: number;
  node?: string | null;
  verdict?: boolean | null;
}

export interface Snapshot {
  time: number;
  flow: string;
  source?: string; // 'mock' | 'external' — the tag source (mock sim vs real gateway)
  layout: { length: number; stations: number[] };
  graph?: { nodes: FlowNode[]; edges: FlowEdge[] }; // authorable multi-step process
  parts?: FlowPart[]; // live work-pieces flowing the graph
  tags: Record<string, number | boolean | string>;
}

interface TelemetryState {
  connected: boolean;
  snapshot: Snapshot | null;
  // The feature whose 3D position tracks conveyor.position (the twin "part").
  twinPartId: string | null;
  connect: () => void;
  disconnect: () => void;
  setTwinPart: (id: string | null) => void;
}

let ws: WebSocket | null = null;
let retry: ReturnType<typeof setTimeout> | undefined;

export const useTelemetryStore = create<TelemetryState>((set) => ({
  connected: false,
  snapshot: null,
  twinPartId: null,

  connect: () => {
    if (ws) return; // one shared socket
    const open = () => {
      const proto = location.protocol === 'https:' ? 'wss' : 'ws';
      ws = new WebSocket(`${proto}://${location.host}/api/telemetry/ws`);
      ws.onopen = () => set({ connected: true });
      ws.onmessage = (e) => {
        try {
          set({ snapshot: JSON.parse(e.data) });
        } catch {
          /* ignore malformed frame */
        }
      };
      ws.onclose = () => {
        set({ connected: false });
        ws = null;
        retry = setTimeout(open, 1000); // reconnect while still mounted
      };
      ws.onerror = () => ws?.close();
    };
    open();
  },

  disconnect: () => {
    if (retry) clearTimeout(retry);
    retry = undefined;
    const sock = ws;
    ws = null;
    if (sock) {
      sock.onclose = null; // don't trigger reconnect
      sock.close();
    }
    set({ connected: false });
  },

  setTwinPart: (id) => set({ twinPartId: id }),
}));
