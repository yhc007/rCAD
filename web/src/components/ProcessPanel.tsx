import React from 'react';
import { Activity, X, Play, Pause, RotateCcw } from 'lucide-react';

interface Snapshot {
  time: number;
  flow: string;
  layout: { length: number; stations: number[] };
  tags: Record<string, number | boolean>;
}

// Live process digital-twin: streams tag telemetry from the mock conveyor flow
// (server /api/telemetry/ws), visualises the belt + flow state, and lets the
// operator start/stop/reset the line (supervisory control → /api/telemetry/
// control). Tag-keyed, so the source can later be a real MQTT / OPC UA gateway.
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
          /* ignore */
        }
      };
      ws.onclose = () => {
        setConnected(false);
        if (!closed) retry = setTimeout(connect, 1000);
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

  const control = (command: string) =>
    fetch('/api/telemetry/control', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ command }),
    }).catch(() => {});

  if (!open) return null;

  const layout = snap?.layout ?? { length: 300, stations: [100, 220] };
  const pos = Number(snap?.tags['conveyor.position'] ?? 0);
  const running = !!snap?.tags['conveyor.running'];
  const near = (i: number) => !!snap?.tags[`station${i + 1}.proximity`];
  const busy = (i: number) => !!snap?.tags[`station${i + 1}.busy`];
  const anyBusy = busy(0) || busy(1);

  return (
    <div className="absolute bottom-0 left-0 right-0 z-30 h-56 flex flex-col bg-cad-panel border-t border-cad-border shadow-2xl">
      <div className="flex items-center gap-2 h-9 px-4 border-b border-cad-border">
        <Activity size={15} className="text-cad-accent" />
        <span className="text-sm font-semibold text-cad-text">Process Twin</span>
        <span className="flex items-center gap-1 text-xs text-cad-text-muted ml-2">
          <span className={`w-2 h-2 rounded-full ${connected ? 'bg-green-500' : 'bg-red-500'}`} />
          {connected ? 'live' : 'connecting…'}
        </span>
        {/* flow state */}
        <span
          className={`ml-3 px-2 py-0.5 rounded text-xs font-mono ${
            anyBusy ? 'bg-yellow-500/20 text-yellow-400' : running ? 'bg-green-500/15 text-green-400' : 'bg-cad-bg text-cad-text-muted'
          }`}
        >
          {snap?.flow ?? '—'}
        </span>
        {/* controls */}
        <div className="flex items-center gap-1 ml-3">
          <CtrlBtn title="Start" onClick={() => control('start')}><Play size={13} /></CtrlBtn>
          <CtrlBtn title="Stop" onClick={() => control('stop')}><Pause size={13} /></CtrlBtn>
          <CtrlBtn title="Reset" onClick={() => control('reset')}><RotateCcw size={13} /></CtrlBtn>
        </div>
        <span className="text-xs text-cad-text-muted ml-3">t={snap?.time ?? 0}s</span>
        <button className="ml-auto text-cad-text-muted hover:text-cad-text" onClick={onClose} title="Close">
          <X size={16} />
        </button>
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Conveyor strip */}
        <div className="flex-1 p-4 flex flex-col justify-center">
          <div className="text-xs text-cad-text-muted mb-2">
            Conveyor (mm) · belt {running ? 'running' : 'stopped'}
          </div>
          <svg viewBox={`0 0 ${layout.length} 100`} preserveAspectRatio="none" className="w-full h-24">
            <rect x={0} y={38} width={layout.length} height={24} fill="#2a2a33" stroke="#444" strokeWidth={0.5} />
            {layout.stations.map((s, i) => (
              <g key={i}>
                <line x1={s} y1={20} x2={s} y2={80} stroke="#555" strokeWidth={1} strokeDasharray="3 3" />
                <circle
                  cx={s}
                  cy={18}
                  r={6}
                  fill={busy(i) ? '#eab308' : near(i) ? '#22c55e' : '#444'}
                  stroke="#666"
                  strokeWidth={0.5}
                />
                <text x={s} y={95} fontSize={7} fill="#888" textAnchor="middle">S{i + 1}</text>
              </g>
            ))}
            <rect x={pos - 9} y={34} width={18} height={32} rx={2} fill={anyBusy ? '#eab308' : '#e08a3c'} />
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

function CtrlBtn({ title, onClick, children }: { title: string; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      title={title}
      onClick={onClick}
      className="p-1.5 rounded bg-cad-bg border border-cad-border text-cad-text hover:border-cad-accent"
    >
      {children}
    </button>
  );
}
