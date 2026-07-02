import React from 'react';
import { Sparkles, X, Send, Check, Ban } from 'lucide-react';
import { useDocumentStore } from '../stores/documentStore';
import { runTurn, type Msg, type ToolCall } from '../lib/aiAgent';

type Item =
  | { kind: 'user'; text: string }
  | { kind: 'assistant'; text: string }
  | { kind: 'error'; text: string }
  | { kind: 'action'; id: string; name: string; input: Record<string, unknown>; status: 'running' | 'done'; result?: unknown };

const EXAMPLES = [
  'Make a red 60mm cube',
  'Add a 20mm-radius, 80mm-tall cylinder next to it',
  'Build a fixed base plate with a smaller box resting on top',
];

export function AiPanel({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [items, setItems] = React.useState<Item[]>([]);
  const [input, setInput] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  const [pending, setPending] = React.useState<{ call: ToolCall; resolve: (ok: boolean) => void } | null>(null);
  const historyRef = React.useRef<Msg[]>([]);
  const scrollRef = React.useRef<HTMLDivElement>(null);
  const selectFeature = useDocumentStore((s) => s.selectFeature);

  React.useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [items, pending]);

  const send = async (text: string) => {
    const q = text.trim();
    if (!q || busy) return;
    setInput('');
    setItems((prev) => [...prev, { kind: 'user', text: q }]);
    setBusy(true);
    try {
      historyRef.current = await runTurn(historyRef.current, q, {
        onAssistantText: (t) => setItems((p) => [...p, { kind: 'assistant', text: t }]),
        onToolCall: (c) =>
          setItems((p) => [...p, { kind: 'action', id: c.id, name: c.name, input: c.input, status: 'running' }]),
        onToolResult: (id, result) =>
          setItems((p) =>
            p.map((it) => (it.kind === 'action' && it.id === id ? { ...it, status: 'done', result } : it))
          ),
        requestApproval: (call) => new Promise<boolean>((resolve) => setPending({ call, resolve })),
        onError: (m) => setItems((p) => [...p, { kind: 'error', text: m }]),
      });
    } finally {
      setBusy(false);
      setPending(null);
    }
  };

  const resolveApproval = (ok: boolean) => {
    pending?.resolve(ok);
    setPending(null);
  };

  if (!open) return null;

  return (
    <div className="absolute top-0 right-0 h-full w-[360px] z-40 flex flex-col bg-cad-panel border-l border-cad-border shadow-2xl">
      <div className="flex items-center gap-2 h-12 px-4 border-b border-cad-border">
        <Sparkles size={16} className="text-cad-accent" />
        <span className="text-sm font-semibold text-cad-text flex-1">AI Copilot</span>
        <button className="text-cad-text-muted hover:text-cad-text" onClick={onClose} title="Close">
          <X size={18} />
        </button>
      </div>

      <div ref={scrollRef} className="flex-1 overflow-auto p-3 space-y-2">
        {items.length === 0 && (
          <div className="text-sm text-cad-text-muted space-y-2">
            <p>Describe what to build or change. Every action is undoable.</p>
            {EXAMPLES.map((ex) => (
              <button
                key={ex}
                onClick={() => send(ex)}
                className="block w-full text-left px-3 py-2 rounded bg-cad-bg border border-cad-border hover:border-cad-accent text-cad-text"
              >
                {ex}
              </button>
            ))}
          </div>
        )}

        {items.map((it, i) => (
          <MessageItem key={i} item={it} onSelect={selectFeature} />
        ))}

        {busy && !pending && (
          <div className="text-xs text-cad-text-muted animate-pulse px-1">thinking…</div>
        )}

        {pending && (
          <div className="rounded border border-cad-accent/60 bg-cad-accent/10 p-3 space-y-2">
            <p className="text-sm text-cad-text">
              Approve <span className="font-mono text-cad-accent">{pending.call.name}</span>?
            </p>
            <pre className="text-xs text-cad-text-muted whitespace-pre-wrap break-all">
              {JSON.stringify(pending.call.input)}
            </pre>
            <div className="flex gap-2">
              <button
                onClick={() => resolveApproval(true)}
                className="flex items-center gap-1 px-3 py-1 rounded bg-cad-accent text-white text-sm"
              >
                <Check size={14} /> Approve
              </button>
              <button
                onClick={() => resolveApproval(false)}
                className="flex items-center gap-1 px-3 py-1 rounded bg-cad-bg border border-cad-border text-cad-text text-sm"
              >
                <Ban size={14} /> Deny
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="p-3 border-t border-cad-border">
        <div className="flex items-end gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                send(input);
              }
            }}
            rows={2}
            placeholder="Ask the copilot…"
            disabled={busy}
            className="flex-1 resize-none px-3 py-2 bg-cad-bg border border-cad-border rounded text-sm text-cad-text placeholder:text-cad-text-muted focus:outline-none focus:border-cad-accent disabled:opacity-50"
          />
          <button
            onClick={() => send(input)}
            disabled={busy || !input.trim()}
            className="p-2 rounded bg-cad-accent text-white disabled:opacity-40"
            title="Send (Enter)"
          >
            <Send size={16} />
          </button>
        </div>
      </div>
    </div>
  );
}

function MessageItem({ item, onSelect }: { item: Item; onSelect: (id: string) => void }) {
  if (item.kind === 'user') {
    return (
      <div className="ml-6 px-3 py-2 rounded bg-cad-accent/15 border border-cad-accent/30 text-sm text-cad-text">
        {item.text}
      </div>
    );
  }
  if (item.kind === 'assistant') {
    return <div className="px-1 text-sm text-cad-text whitespace-pre-wrap">{item.text}</div>;
  }
  if (item.kind === 'error') {
    return (
      <div className="px-3 py-2 rounded bg-red-500/10 border border-red-500/40 text-sm text-red-300">
        ⚠ {item.text}
      </div>
    );
  }

  // action chip
  const res = item.result as { error?: string; id?: string; bbox?: { size?: number[] } } | undefined;
  const err = res?.error;
  const id = res?.id;
  const size = res?.bbox?.size;
  return (
    <button
      disabled={!id}
      onClick={() => id && onSelect(id)}
      className={`w-full text-left flex items-center gap-2 px-2 py-1 rounded text-xs font-mono ${
        id ? 'hover:bg-cad-bg cursor-pointer' : 'cursor-default'
      }`}
    >
      <span className={item.status === 'running' ? 'text-cad-text-muted animate-pulse' : err ? 'text-red-400' : 'text-green-400'}>
        {item.status === 'running' ? '…' : err ? '✕' : '✓'}
      </span>
      <span className="text-cad-text">{item.name}</span>
      <span className="text-cad-text-muted truncate flex-1">
        {err ? err : size ? `[${size.join(' × ')}]` : id ? id.slice(0, 8) : JSON.stringify(item.input)}
      </span>
    </button>
  );
}
