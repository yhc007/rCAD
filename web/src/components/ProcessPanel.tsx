import React from 'react';
import { Activity, X } from 'lucide-react';

interface Snapshot {
  time: number;
  layout: { length: number; stations: number[] };
  tags: Record<string, number | boolean>;
}

// Live process digital-twin: streams tag telemetry from the mock conveyor sim
// (server /api/telemetry/ws) and shows a 2D belt + a tag inspector. The tag
// abstraction means this same panel will work once the source is a real MQTT /
// OPC UA gateway.
export function ProcessPanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [snap, setSnap] = React.useState<Snapshot | null>(null);
  const [connected, setConnected] = React.useState(false);

  React.useEffect(() => {
    if (!open) return;
    let ws: WebSocket | null = null;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let closed = false;

    const connect = () => {
      const proto = location.protocol === 'https:' ? 'wss' : 'ws';
      ws = new WebSocket(`${proto}://${location.host}/api/telemetry/ws`);
      ws.onopen = () => setConnected(true);
      ws.onmessage = (e) => {
        try {
          setSnap(JSON.parse(e.data));
        } catch {
          /* ignore malformed frame */
        }
      };
      ws.onclose = () => {
        setConnected(false);
        if (!closed) retry = setTimeout(connect, 1000); // reconnect
      };
      ws.onerror = () => ws?.close();
    };
    connect();
    return () => {
      closed = true;
      if (retry) clearTimeout(retry);
      ws?.close();
    };
  }, [open]);

  if (!open) return null;

  const layout = snap?.layout ?? { length: 300, stations: [100, 220] };
  const pos = Number(snap?.tags['conveyor.position'] ?? 0);
  const stationTrip = (i: number) => !!snap?.tags[`station${i + 1}.proximity`];

  return (
    <div className="absolute bottom-0 left-0 right-0 z-30 h-56 flex flex-col bg-cad-panel border-t border-cad-border shadow-2xl">
      <div className="flex items-center gap-2 h-9 px-4 border-b border-cad-border">
        <Activity size={15} className="text-cad-accent" />
        <span className="text-sm font-semibold text-cad-text">Process Twin</span>
        <span className="flex items-center gap-1 text-xs text-cad-text-muted ml-2">
          <span className={`w-2 h-2 rounded-full ${connected ? 'bg-green-500' : 'bg-red-500'}`} />
          {connected ? 'live' : 'connecting…'}
        </span>
        <span className="text-xs text-cad-text-muted ml-2">t={snap?.time ?? 0}s</span>
        <button className="ml-auto text-cad-text-muted hover:text-cad-text" onClick={onClose} title="Close">
          <X size={16} />
        </button>
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Conveyor strip */}
        <div className="flex-1 p-4 flex flex-col justify-center">
          <div className="text-xs text-cad-text-muted mb-2">Conveyor (mm)</div>
          <svg viewBox={`0 0 ${layout.length} 100`} preserveAspectRatio="none" className="w-full h-24">
            {/* belt */}
            <rect x={0} y={38} width={layout.length} height={24} fill="#2a2a33" stroke="#444" strokeWidth={0.5} />
            {/* stations + proximity sensors */}
            {layout.stations.map((s, i) => (
              <g key={i}>
                <line x1={s} y1={20} x2={s} y2={80} stroke="#555" strokeWidth={1} strokeDasharray="3 3" />
                <circle cx={s} cy={18} r={6} fill={stationTrip(i) ? '#22c55e' : '#444'} stroke="#666" strokeWidth={0.5} />
                <text x={s} y={95} fontSize={7} fill="#888" textAnchor="middle">S{i + 1}</text>
              </g>
            ))}
            {/* part */}
            <rect x={pos - 9} y={34} width={18} height={32} rx={2} fill="#e08a3c" />
          </svg>
        </div>

        {/* Tag inspector */}
        <div className="w-72 border-l border-cad-border overflow-auto p-3">
          <div className="text-xs text-cad-text-muted mb-2 uppercase">Tags</div>
          <table className="w-full text-xs font-mono">
            <tbody>
              {snap &&
                Object.entries(snap.tags).map(([k, v]) => (
                  <tr key={k} className="border-b border-cad-border/40">
                    <td className="py-1 text-cad-text-muted">{k}</td>
                    <td className="py-1 text-right text-cad-text">
                      {typeof v === 'boolean' ? (
                        <span className={v ? 'text-green-400' : 'text-cad-text-muted'}>{v ? '● TRUE' : '○ false'}</span>
                      ) : (
                        v
                      )}
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
