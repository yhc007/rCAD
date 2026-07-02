import { useEffect } from 'react';
import { useDocumentStore } from '../stores/documentStore';

// Bottom-center transient notice driven by store.notice. Auto-dismisses.
export function Toast() {
  const notice = useDocumentStore((s) => s.notice);
  const setNotice = useDocumentStore((s) => s.setNotice);

  // Progress messages end with '…' and stay until the caller replaces/clears
  // them (a big STEP import can run well past a fixed timeout); everything else
  // auto-dismisses.
  const isProgress = !!notice && notice.endsWith('…');

  useEffect(() => {
    if (!notice || isProgress) return;
    const t = setTimeout(() => setNotice(null), 6000);
    return () => clearTimeout(t);
  }, [notice, isProgress, setNotice]);

  if (!notice) return null;

  return (
    <div className="absolute bottom-6 left-1/2 -translate-x-1/2 z-50 max-w-lg">
      <div className="flex items-start gap-3 px-4 py-3 bg-cad-panel border border-cad-border rounded-lg shadow-xl">
        <span className={isProgress ? 'text-cad-accent animate-pulse' : 'text-yellow-500'}>
          {isProgress ? '⏳' : '⚠️'}
        </span>
        <p className="text-sm text-cad-text flex-1">{notice}</p>
        <button
          className="text-cad-text-muted hover:text-cad-text"
          onClick={() => setNotice(null)}
          title="Dismiss"
        >
          ✕
        </button>
      </div>
    </div>
  );
}
