import React from 'react';
import { Activity, X, Play, Pause, RotateCcw, Boxes } from 'lucide-react';
import { useTelemetryStore } from '../stores/telemetryStore';
import { useCAD } from '../hooks/useCAD';

// Live process digital-twin: streams tag telemetry from the mock conveyor flow
// (server /api/telemetry/ws), visualises the belt + flow state, and lets the
// operator start/stop/reset the line (supervisory control → /api/telemetry/
// control). "Build cell" spawns a 3D conveyor+part+stations whose part rides the
// belt live. Tag-keyed, so the source can later be a real MQTT / OPC UA gateway.
export function ProcessPanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  const snap = useTelemetryStore((s) => s.snapshot);
  const connected = useTelemetryStore((s) => s.connected);
  const connect = useTelemetryStore((s) => s.connect);
  const disconnect = useTelemetryStore((s) => s.disconnect);
  const setTwinPart = useTelemetryStore((s) => s.setTwinPart);
  const { cad } = useCAD();
  const [building, setBuilding] = React.useState(false);

  React.useEffect(() => {
    if (!open) return;
    connect();
    return () => disconnect();
  }, [open, connect, disconnect]);

  // Spawn a demo cell: a belt (x 0..300), two sensor markers, and a part that
  // the Canvas binds to conveyor.position so it rides the belt live.
  const buildCell = async () => {
    if (building) return;
    setBuilding(true);
    try {
      await cad.newDoc();
      const conv = await cad.addPrimitive('box', [300, 10, 40]);
      await cad.moveFeature(conv, 150, -5, 0); // belt top at Y=0, spanning x 0..300
      await cad.setFeatureProps(conv, { color: [0.28, 0.3, 0.34] });
      for (const x of [100, 220]) {
        const st = await cad.addPrimitive('box', [8, 8, 8]);
        await cad.moveFeature(st, x, 32, 0); // sensor marker above the belt
        await cad.setFeatureProps(st, { color: [0.5, 0.5, 0.55] });
      }
      // Inspection camera marker over station 2 (the vision station).
      const cam = await cad.addPrimitive('box', [10, 6, 6]);
      await cad.moveFeature(cam, 220, 52, 24);
      await cad.setFeatureProps(cam, { color: [0.14, 0.14, 0.18] });
      const part = await cad.addPrimitive('box', [20, 20, 20]);
      await cad.moveFeature(part, 0, 10, 0); // sits on the belt at the start
      await cad.setFeatureProps(part, { color: [0.88, 0.55, 0.2] });
      setTwinPart(part);
    } finally {
      setBuilding(false);
    }
  };

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
  const result = snap?.tags['inspection.result'];
  const partFill = result === 'PASS' ? '#22c55e' : result === 'FAIL' ? '#ef4444' : anyBusy ? '#eab308' : '#e08a3c';

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
        {/* QC / inspection */}
        {snap && (
          <span className="ml-2 text-xs font-mono flex items-center gap-1">
            <span className="text-cad-text-muted">QC</span>
            <span className="text-green-400">{String(snap.tags['inspection.pass'] ?? 0)}✓</span>
            <span className="text-red-400">{String(snap.tags['inspection.fail'] ?? 0)}✗</span>
            <span className={result === 'PASS' ? 'text-green-400' : result === 'FAIL' ? 'text-red-400' : 'text-cad-text-muted'}>
              {String(result ?? '—')}
            </span>
          </span>
        )}
        {/* controls */}
        <div className="flex items-center gap-1 ml-3">
          <CtrlBtn title="Start" onClick={() => control('start')}><Play size={13} /></CtrlBtn>
          <CtrlBtn title="Stop" onClick={() => control('stop')}><Pause size={13} /></CtrlBtn>
          <CtrlBtn title="Reset" onClick={() => control('reset')}><RotateCcw size={13} /></CtrlBtn>
          <button
            title="Build 3D cell"
            onClick={buildCell}
            disabled={building}
            className="ml-1 flex items-center gap-1 px-2 py-1 rounded bg-cad-bg border border-cad-border text-cad-text text-xs hover:border-cad-accent disabled:opacity-50"
          >
            <Boxes size={13} /> Build cell
          </button>
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
            <rect x={pos - 9} y={34} width={18} height={32} rx={2} fill={partFill} />
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
