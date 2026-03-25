import React, { useRef, useEffect, useCallback } from 'react';
import { useWebGPU } from '../hooks/useWebGPU';
import { useDocumentStore } from '../stores/documentStore';

export function Canvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const { initialize, render, resize, orbit, pan, zoom, isReady, error, rendererType } = useWebGPU();
  const { selectedFeature } = useDocumentStore();

  // Track mouse state
  const isDragging = useRef(false);
  const isPanning = useRef(false);
  const lastPos = useRef({ x: 0, y: 0 });

  // Initialize WebGPU
  useEffect(() => {
    if (canvasRef.current) {
      initialize(canvasRef.current);
    }
  }, [initialize]);

  // Handle resize
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry && canvasRef.current) {
        const { width, height } = entry.contentRect;
        canvasRef.current.width = width * window.devicePixelRatio;
        canvasRef.current.height = height * window.devicePixelRatio;
        canvasRef.current.style.width = `${width}px`;
        canvasRef.current.style.height = `${height}px`;
        resize(Math.floor(width), Math.floor(height));
      }
    });

    observer.observe(container);
    return () => observer.disconnect();
  }, [resize]);

  // Render loop
  useEffect(() => {
    if (!isReady) return;

    let animationId: number;

    const renderLoop = () => {
      render();
      animationId = requestAnimationFrame(renderLoop);
    };

    animationId = requestAnimationFrame(renderLoop);
    return () => cancelAnimationFrame(animationId);
  }, [isReady, render]);

  // Mouse event handlers
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 0) {
      // Left button - orbit
      isDragging.current = true;
    } else if (e.button === 1 || (e.button === 0 && e.shiftKey)) {
      // Middle button or shift+left - pan
      isPanning.current = true;
    }
    lastPos.current = { x: e.clientX, y: e.clientY };
  }, []);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const dx = e.clientX - lastPos.current.x;
    const dy = e.clientY - lastPos.current.y;
    lastPos.current = { x: e.clientX, y: e.clientY };

    if (isDragging.current) {
      orbit(dx * 0.01, dy * 0.01);
    } else if (isPanning.current) {
      pan(dx, -dy);
    }
  }, [orbit, pan]);

  const handleMouseUp = useCallback(() => {
    isDragging.current = false;
    isPanning.current = false;
  }, []);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    zoom(e.deltaY * 0.001);
  }, [zoom]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return;

      switch (e.key) {
        case '1':
          // Front view
          break;
        case '2':
          // Back view
          break;
        case '3':
          // Right view
          break;
        case '4':
          // Left view
          break;
        case '5':
          // Top view
          break;
        case '6':
          // Bottom view
          break;
        case '7':
          // Isometric view
          break;
        case 'f':
        case 'F':
          // Fit to view
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <div
      ref={containerRef}
      className="w-full h-full relative"
      onContextMenu={(e) => e.preventDefault()}
    >
      <canvas
        ref={canvasRef}
        className="w-full h-full"
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onWheel={handleWheel}
      />

      {/* WebGPU Error Fallback */}
      {error && (
        <div className="absolute inset-0 flex items-center justify-center bg-cad-bg/90">
          <div className="max-w-md p-6 bg-cad-panel border border-cad-border rounded-lg text-center">
            <div className="text-yellow-500 text-4xl mb-4">⚠️</div>
            <h3 className="text-lg font-semibold text-cad-text mb-2">WebGPU Not Available</h3>
            <p className="text-cad-text-muted text-sm mb-4">{error}</p>
            <p className="text-cad-text-muted text-xs">
              The CAD modeling engine (WASM) is still functional.
              3D rendering requires WebGPU support.
            </p>
          </div>
        </div>
      )}

      {/* View Controls Overlay */}
      <div className="absolute bottom-4 left-4 flex gap-2">
        <ViewButton label="Front" shortcut="1" />
        <ViewButton label="Top" shortcut="5" />
        <ViewButton label="Iso" shortcut="7" />
        <ViewButton label="Fit" shortcut="F" />
      </div>

      {/* Coordinate System Indicator */}
      <div className="absolute bottom-4 right-4 w-16 h-16">
        <CoordinateGizmo />
      </div>

      {/* Renderer indicator */}
      {rendererType && (
        <div className="absolute top-4 right-4 px-2 py-1 bg-cad-panel/80 border border-cad-border rounded text-xs text-cad-text-muted">
          {rendererType === 'webgpu' ? '🚀 WebGPU' : '🔷 WebGL'}
        </div>
      )}
    </div>
  );
}

function ViewButton({ label, shortcut }: { label: string; shortcut: string }) {
  return (
    <button
      className="px-3 py-1 bg-cad-panel/80 hover:bg-cad-panel border border-cad-border rounded text-sm text-cad-text"
      title={`${label} (${shortcut})`}
    >
      {label}
    </button>
  );
}

function CoordinateGizmo() {
  return (
    <svg viewBox="0 0 100 100" className="w-full h-full">
      {/* X axis - Red */}
      <line x1="50" y1="50" x2="90" y2="50" stroke="#ef4444" strokeWidth="3" />
      <text x="95" y="55" fill="#ef4444" fontSize="14">X</text>

      {/* Y axis - Green */}
      <line x1="50" y1="50" x2="50" y2="10" stroke="#22c55e" strokeWidth="3" />
      <text x="45" y="8" fill="#22c55e" fontSize="14">Y</text>

      {/* Z axis - Blue */}
      <line x1="50" y1="50" x2="25" y2="75" stroke="#3b82f6" strokeWidth="3" />
      <text x="15" y="85" fill="#3b82f6" fontSize="14">Z</text>
    </svg>
  );
}
