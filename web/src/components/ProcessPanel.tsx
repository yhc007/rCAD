import React from 'react';
import { Activity, X, Play, Pause, RotateCcw, Boxes } from 'lucide-react';
import { useTelemetryStore, type FlowNode, type FlowEdge, type FlowPart } from '../stores/telemetryStore';
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

  // Real camera feed (inspection camera): the browser webcam via getUserMedia.
  const videoRef = React.useRef<HTMLVideoElement>(null);
  const streamRef = React.useRef<MediaStream | null>(null);
  const [camOn, setCamOn] = React.useState(false);
  const [camErr, setCamErr] = React.useState<string | null>(null);
  const [vision, setVision] = React.useState<{ result: string; defect: number } | null>(null);
  const startCam = async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ video: true, audio: false });
      streamRef.current = stream;
      if (videoRef.current) {
        videoRef.current.srcObject = stream;
        await videoRef.current.play().catch(() => {});
      }
      setCamOn(true);
      setCamErr(null);
    } catch (e) {
      setCamErr(e instanceof Error ? e.message : 'camera unavailable');
    }
  };
  const stopCam = () => {
    streamRef.current?.getTracks().forEach((t) => t.stop());
    streamRef.current = null;
    setCamOn(false);
  };
  React.useEffect(() => () => stopCam(), []); // stop the camera on unmount

  // While the camera is on, grab a frame every ~1.5s, send it to the OpenCV
  // vision service for a real PASS/FAIL, and feed that verdict back into the
  // twin (overriding the mock inspection). Reverts to the mock when it stops.
  React.useEffect(() => {
    if (!camOn) return;
    let alive = true;
    const ingest = (tags: Record<string, unknown>) =>
      fetch('/api/telemetry/ingest', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tags }),
      }).catch(() => {});
    const inspect = async () => {
      const v = videoRef.current;
      if (!v || !v.videoWidth) return;
      const cvs = document.createElement('canvas');
      cvs.width = v.videoWidth;
      cvs.height = v.videoHeight;
      const ctx = cvs.getContext('2d');
      if (!ctx) return;
      ctx.drawImage(v, 0, 0);
      const image = cvs.toDataURL('image/jpeg', 0.7);
      try {
        const d = await (
          await fetch('/vision/inspect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ image }),
          })
        ).json();
        if (!alive || !d.ok) return;
        setVision({ result: d.result, defect: d.defect_ratio });
        ingest({ 'inspection.result': d.result, 'vision.defect': d.defect_ratio });
      } catch {
        /* vision service down → keep the mock */
      }
    };
    const id = setInterval(inspect, 1500);
    inspect();
    return () => {
      alive = false;
      clearInterval(id);
      setVision(null);
      ingest({ 'inspection.result': null, 'vision.defect': null }); // revert to mock
    };
  }, [camOn]);

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
      // Sensor markers bound to live tags → they light up in the 3D twin.
      const markers: [string, number][] = [
        ['station1.proximity', 100],
        ['station2.busy', 220],
      ];
      for (const [tag, x] of markers) {
        const st = await cad.addPrimitive('box', [8, 8, 8]);
        await cad.moveFeature(st, x, 32, 0); // sensor marker above the belt
        await cad.setFeatureProps(st, { color: [0.32, 0.34, 0.4], sensorTag: tag });
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
  const graph = snap?.graph;
  const parts = snap?.parts ?? [];
  const pos = Number(snap?.tags['conveyor.position'] ?? 0);
  const running = !!snap?.tags['conveyor.running'];
  const near = (i: number) => !!snap?.tags[`station${i + 1}.proximity`];
  const busy = (i: number) => !!snap?.tags[`station${i + 1}.busy`];
  const anyBusy = graph ? parts.some((p) => !!p.node) : busy(0) || busy(1);
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
        {/* tag source: mock sim vs an external gateway (MQTT/HTTP) */}
        {snap?.source && (
          <span
            className={`text-xs font-mono px-1.5 py-0.5 rounded ${
              snap.source === 'external' ? 'bg-cad-accent/20 text-cad-accent' : 'bg-cad-bg text-cad-text-muted'
            }`}
            title="Telemetry source"
          >
            src:{snap.source}
          </span>
        )}
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

      {/* Automation alarm (from a rule) */}
      {snap?.tags['alarm.active'] === true && (
        <div className="px-4 py-1.5 bg-red-600/25 border-b border-red-600/50 text-red-300 text-sm flex items-center gap-2">
          <span className="animate-pulse">🚨</span>
          ALARM — {String(snap.tags['alarm.message'] || 'condition met')}
        </div>
      )}

      <div className="flex flex-1 overflow-hidden">
        {/* Process flow graph (multi-step) — falls back to the single belt */}
        <div className="flex-1 p-4 flex flex-col justify-center min-w-0">
          <div className="text-xs text-cad-text-muted mb-2">
            {graph
              ? `Flow graph · ${graph.nodes.length} steps · ${parts.length} WIP`
              : 'Conveyor (mm)'}{' '}
            · belt {running ? 'running' : 'stopped'}
          </div>
          {graph ? (
            <FlowGraphView graph={graph} parts={parts} tags={snap!.tags} />
          ) : (
            <svg viewBox={`0 0 ${layout.length} 100`} preserveAspectRatio="none" className="w-full h-24">
              <rect x={0} y={38} width={layout.length} height={24} fill="#2a2a33" stroke="#444" strokeWidth={0.5} />
              {layout.stations.map((s, i) => (
                <g key={i}>
                  <line x1={s} y1={20} x2={s} y2={80} stroke="#555" strokeWidth={1} strokeDasharray="3 3" />
                  <circle cx={s} cy={18} r={6} fill={busy(i) ? '#eab308' : near(i) ? '#22c55e' : '#444'} stroke="#666" strokeWidth={0.5} />
                  <text x={s} y={95} fontSize={7} fill="#888" textAnchor="middle">S{i + 1}</text>
                </g>
              ))}
              <rect x={pos - 9} y={34} width={18} height={32} rx={2} fill={partFill} />
            </svg>
          )}
        </div>

        {/* Inspection camera (real webcam feed) */}
        <div className="w-52 border-l border-cad-border p-2 flex flex-col">
          <div className="text-xs text-cad-text-muted mb-1 flex items-center justify-between">
            <span>Inspection cam · S2</span>
            <button className="text-cad-accent hover:underline" onClick={camOn ? stopCam : startCam}>
              {camOn ? 'stop' : 'start'}
            </button>
          </div>
          <div className="relative flex-1 bg-black rounded overflow-hidden flex items-center justify-center">
            <video ref={videoRef} muted playsInline className="w-full h-full object-cover" />
            {!camOn && (
              <span className="absolute text-[11px] text-cad-text-muted px-2 text-center">
                {camErr ?? 'camera off'}
              </span>
            )}
            {camOn && vision && (
              <div className="absolute top-1 left-1 flex flex-col gap-0.5 items-start">
                <span
                  className={`px-1.5 py-0.5 rounded text-xs font-bold text-white ${
                    vision.result === 'PASS' ? 'bg-green-600/80' : 'bg-red-600/80'
                  }`}
                >
                  {vision.result}
                </span>
                <span className="text-[10px] text-white bg-black/50 px-1 rounded">
                  defect {Math.round(vision.defect * 100)}%
                </span>
              </div>
            )}
            {camOn && (
              <span className="absolute bottom-1 right-1 text-[10px] px-1 rounded bg-cad-accent/70 text-white">
                OpenCV
              </span>
            )}
          </div>
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

// Renders an arbitrary process flow graph: nodes (source/process/inspect/sink)
// laid out by their mm position + lane, transitions as arrows (green=pass,
// red=fail), and live parts as dots coloured by their inspection verdict.
const NODE_FILL: Record<string, string> = {
  source: '#3b82f6',
  process: '#475569',
  inspect: '#8b5cf6',
  sink: '#6b7280',
};
function FlowGraphView({
  graph,
  parts,
  tags,
}: {
  graph: { nodes: FlowNode[]; edges: FlowEdge[] };
  parts: FlowPart[];
  tags: Record<string, number | boolean | string>;
}) {
  const byId = React.useMemo(() => {
    const m: Record<string, FlowNode> = {};
    for (const n of graph.nodes) m[n.id] = n;
    return m;
  }, [graph]);
  const length = Math.max(60, ...graph.nodes.map((n) => n.pos));
  const laneY = (y?: number) => 44 - (y ?? 0) * 30; // y=0 belt line; negative = below
  const partFill = (v?: boolean | null) => (v === true ? '#22c55e' : v === false ? '#ef4444' : '#e0a83c');

  return (
    <svg viewBox={`-14 0 ${length + 40} 100`} preserveAspectRatio="none" className="w-full h-32">
      {/* main belt line */}
      <rect x={0} y={laneY(0) - 3} width={length} height={6} fill="#2a2a33" stroke="#444" strokeWidth={0.4} />
      {/* transitions */}
      {graph.edges.map((e, i) => {
        const a = byId[e.from];
        const b = byId[e.to];
        if (!a || !b) return null;
        const stroke = e.when === 'pass' ? '#22c55e' : e.when === 'fail' ? '#ef4444' : '#5b6472';
        const mx = (a.pos + b.pos) / 2;
        const my = (laneY(a.y) + laneY(b.y)) / 2;
        return (
          <g key={i}>
            <line x1={a.pos} y1={laneY(a.y)} x2={b.pos} y2={laneY(b.y)} stroke={stroke} strokeWidth={1} opacity={0.7} markerEnd="url(#arrow)" />
            {e.when && (
              <text x={mx} y={my - 2} fontSize={5.5} fill={stroke} textAnchor="middle">
                {e.when}
              </text>
            )}
          </g>
        );
      })}
      <defs>
        <marker id="arrow" markerWidth={6} markerHeight={6} refX={5} refY={2} orient="auto">
          <path d="M0,0 L4,2 L0,4 Z" fill="#6b7280" />
        </marker>
      </defs>
      {/* nodes */}
      {graph.nodes.map((n) => {
        const busy = tags[`node.${n.id}.busy`] === true;
        const count = tags[`node.${n.id}.count`];
        return (
          <g key={n.id}>
            <circle
              cx={n.pos}
              cy={laneY(n.y)}
              r={7}
              fill={NODE_FILL[n.kind] ?? '#475569'}
              stroke={busy ? '#eab308' : '#20242c'}
              strokeWidth={busy ? 2 : 1}
            />
            <text x={n.pos} y={laneY(n.y) - 10} fontSize={5.5} fill="#cbd5e1" textAnchor="middle">
              {n.label ?? n.id}
            </text>
            {count != null && (
              <text x={n.pos} y={laneY(n.y) + 1.8} fontSize={5} fill="#0b0e14" textAnchor="middle" fontWeight="bold">
                {String(count)}
              </text>
            )}
          </g>
        );
      })}
      {/* live parts */}
      {parts.map((p) => (
        <circle key={p.id} cx={p.x} cy={laneY(p.y) + 0} r={3.6} fill={partFill(p.verdict)} stroke="#0b0e14" strokeWidth={0.6} />
      ))}
    </svg>
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
