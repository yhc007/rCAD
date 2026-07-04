import React, { useRef, useEffect, useCallback, useState } from 'react';
import { useWebGPU } from '../hooks/useWebGPU';
import { useCAD } from '../hooks/useCAD';
import { useDocumentStore } from '../stores/documentStore';
import { useTelemetryStore } from '../stores/telemetryStore';
import { importMeshFile } from '../lib/importFile';
import { transformGeometryByPose } from '../lib/physics';
import type { MeshGeometry } from '../lib/meshParsers';

// Transform-gizmo axes (X red, Y green, Z blue).
const AXES: [number, number, number][] = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
const AXIS_COLORS = ['#ef4444', '#22c55e', '#3b82f6'];

function aabbInfo(p: Float32Array) {
  const mn = [Infinity, Infinity, Infinity];
  const mx = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < p.length; i += 3) {
    for (let k = 0; k < 3; k++) {
      if (p[i + k] < mn[k]) mn[k] = p[i + k];
      if (p[i + k] > mx[k]) mx[k] = p[i + k];
    }
  }
  return {
    center: [(mn[0] + mx[0]) / 2, (mn[1] + mx[1]) / 2, (mn[2] + mx[2]) / 2] as [number, number, number],
    maxDim: Math.max(mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]),
  };
}

function offsetGeom(g: MeshGeometry, d: [number, number, number]): MeshGeometry {
  const positions = new Float32Array(g.positions.length);
  for (let i = 0; i < g.positions.length; i += 3) {
    positions[i] = g.positions[i] + d[0];
    positions[i + 1] = g.positions[i + 1] + d[1];
    positions[i + 2] = g.positions[i + 2] + d[2];
  }
  return { positions, normals: g.normals, vertexCount: g.vertexCount };
}

// Orthonormal in-plane basis (u, v) for each rotation axis's ring.
const RING_BASIS: { u: [number, number, number]; v: [number, number, number] }[] = [
  { u: [0, 1, 0], v: [0, 0, 1] }, // X axis → YZ plane
  { u: [0, 0, 1], v: [1, 0, 0] }, // Y axis → ZX plane
  { u: [1, 0, 0], v: [0, 1, 0] }, // Z axis → XY plane
];

function rotateAxisVec(axisIdx: number, a: number, x: number, y: number, z: number): [number, number, number] {
  const c = Math.cos(a), s = Math.sin(a);
  if (axisIdx === 0) return [x, y * c - z * s, y * s + z * c];
  if (axisIdx === 1) return [x * c + z * s, y, -x * s + z * c];
  return [x * c - y * s, x * s + y * c, z];
}

// Rotate a mesh in place (about its AABB centre) — mirrors the WASM behaviour.
function rotateGeom(g: MeshGeometry, axisIdx: number, a: number): MeshGeometry {
  const { center } = aabbInfo(g.positions);
  const positions = new Float32Array(g.positions.length);
  const normals = new Float32Array(g.normals.length);
  const p = g.positions, n = g.normals;
  for (let i = 0; i < p.length; i += 3) {
    const [rx, ry, rz] = rotateAxisVec(axisIdx, a, p[i] - center[0], p[i + 1] - center[1], p[i + 2] - center[2]);
    positions[i] = rx + center[0]; positions[i + 1] = ry + center[1]; positions[i + 2] = rz + center[2];
    const [nx, ny, nz] = rotateAxisVec(axisIdx, a, n[i], n[i + 1], n[i + 2]);
    normals[i] = nx; normals[i + 1] = ny; normals[i + 2] = nz;
  }
  return { positions, normals, vertexCount: g.vertexCount };
}

const dot3 = (a: number[], b: readonly number[]) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

// Angle of the ray∩(plane through C, normal A) in the (U, V) basis, or null.
function ringAngle(
  ray: { origin: number[]; dir: number[] },
  C: readonly number[],
  A: readonly number[],
  U: readonly number[],
  V: readonly number[]
): number | null {
  const denom = dot3(ray.dir, A);
  if (Math.abs(denom) < 1e-6) return null;
  const tp = dot3([C[0] - ray.origin[0], C[1] - ray.origin[1], C[2] - ray.origin[2]], A) / denom;
  const rel = [
    ray.origin[0] + tp * ray.dir[0] - C[0],
    ray.origin[1] + tp * ray.dir[1] - C[1],
    ray.origin[2] + tp * ray.dir[2] - C[2],
  ];
  return Math.atan2(dot3(rel, V), dot3(rel, U));
}

interface GizmoDrag {
  mode: 'translate' | 'rotate';
  id: string;
  axisVec: [number, number, number];
  axisIdx: number;
  rest: MeshGeometry;
  isImport: boolean;
  // translate
  axisScreen: [number, number];
  downPx: [number, number];
  length: number;
  worldDelta: [number, number, number];
  // rotate
  center: [number, number, number];
  u: readonly [number, number, number];
  v: readonly [number, number, number];
  lastAngle: number;
  totalAngle: number;
}

export function Canvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const { initialize, render, resize, orbit, pan, zoom, setMeshes, setView, fitToContent, pick, setSelection, project, unprojectRay, isReady, error, rendererType } = useWebGPU();
  const { cad } = useCAD();
  const meshes = useDocumentStore((s) => s.meshes);
  const features = useDocumentStore((s) => s.features);
  const selectedFeatures = useDocumentStore((s) => s.selectedFeatures);
  const selectFeature = useDocumentStore((s) => s.selectFeature);
  const toggleSelectFeature = useDocumentStore((s) => s.toggleSelectFeature);
  const setNotice = useDocumentStore((s) => s.setNotice);
  const simulation = useDocumentStore((s) => s.simulation);
  const playNonce = useDocumentStore((s) => s.playNonce);
  const playing = useDocumentStore((s) => s.playing);
  const currentFrame = useDocumentStore((s) => s.currentFrame);
  const [isDragOver, setIsDragOver] = useState(false);

  // Track mouse state
  const isDragging = useRef(false);
  const isPanning = useRef(false);
  const lastPos = useRef({ x: 0, y: 0 });
  const downPos = useRef({ x: 0, y: 0 });
  const moved = useRef(false);

  // Transform gizmo (SVG overlay) drag state.
  const gizmoRef = useRef<SVGSVGElement>(null);
  const drag = useRef<GizmoDrag | null>(null);

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

  // Push rest geometry to the renderer when it changes — unless a simulation is
  // active, in which case the playback loop owns the geometry. Re-fit the camera
  // only when the set of features changes (add/import/remove), not when an
  // existing feature is just moved/edited.
  const prevMeshKeys = useRef('');
  useEffect(() => {
    if (simulation) return;
    const ids = Object.keys(meshes);
    const keys = ids.join(',');
    const fit = keys !== prevMeshKeys.current;
    prevMeshKeys.current = keys;
    const colorOf = (id: string): [number, number, number] =>
      features.find((f) => f.id === id)?.color ?? [0.6, 0.6, 0.7];
    setMeshes(ids.map((id) => meshes[id]), fit, ids.map(colorOf));
  }, [meshes, features, simulation, setMeshes]);

  // Process-twin binding: while telemetry is live, drive the bound "part"
  // feature's rendered position from conveyor.position AND colour every
  // sensor-tagged feature (a sensor/station attached to the model) from its
  // live tag value — a renderer override, not a document command.
  const twinPartId = useTelemetryStore((s) => s.twinPartId);
  const twinSnapshot = useTelemetryStore((s) => s.snapshot);
  useEffect(() => {
    if (simulation || !twinSnapshot) return;
    const st = useDocumentStore.getState();
    const tags = twinSnapshot.tags;
    const pos = Number(tags['conveyor.position'] ?? 0);
    const result = tags['inspection.result'];
    const ids = Object.keys(st.meshes);
    const featOf = (id: string) => st.features.find((f) => f.id === id);
    // A tag value → an indicator colour.
    const tagColor = (v: unknown): [number, number, number] => {
      if (v === 'PASS' || v === true) return [0.2, 0.78, 0.32];
      if (v === 'FAIL') return [0.85, 0.2, 0.2];
      if (typeof v === 'number' && v > 0) return [0.9, 0.7, 0.2];
      return [0.32, 0.34, 0.4]; // idle / false
    };
    const colorOf = (id: string): [number, number, number] => {
      const f = featOf(id);
      if (id === twinPartId) {
        return result === 'PASS' ? [0.2, 0.78, 0.32] : result === 'FAIL' ? [0.85, 0.2, 0.2] : f?.color ?? [0.6, 0.6, 0.7];
      }
      if (f?.sensorTag && f.sensorTag in tags) return tagColor(tags[f.sensorTag]);
      return f?.color ?? [0.6, 0.6, 0.7];
    };
    const havePart = twinPartId && st.meshes[twinPartId];
    const list = ids.map((id) => (havePart && id === twinPartId ? offsetGeom(st.meshes[id], [pos, 0, 0]) : st.meshes[id]));
    setMeshes(list, false, ids.map(colorOf));
  }, [twinSnapshot, twinPartId, simulation, setMeshes]);

  // Apply a single simulation frame: transform each rest mesh by its body pose.
  const applyFrame = useCallback(
    (frame: number) => {
      const { simulation: sim, meshes: m } = useDocumentStore.getState();
      if (!sim) return;
      const f = Math.max(0, Math.min(frame, sim.frames.length - 1));
      const transformed = sim.bodyIds.map((id, b) =>
        transformGeometryByPose(m[id], sim.centers[id], sim.frames[f][b])
      );
      setMeshes(transformed, false); // no camera re-fit during playback
    },
    [setMeshes]
  );

  // Playback loop: runs while `playing`, starting from the current frame.
  // Re-armed by playNonce (start/replay) and by the play/pause toggle.
  useEffect(() => {
    const s0 = useDocumentStore.getState();
    if (!s0.simulation || !s0.playing) return;
    const fps = s0.simulation.fps;
    const startFrame = s0.currentFrame;
    const startT = performance.now();
    let raf = 0;
    const loop = () => {
      const st = useDocumentStore.getState();
      if (!st.simulation || !st.playing) return;
      const last = st.simulation.frames.length - 1;
      const f = startFrame + Math.floor(((performance.now() - startT) / 1000) * fps);
      if (f >= last) {
        applyFrame(last);
        st.setCurrentFrame(last);
        st.setPlaying(false); // stop at the end
        return;
      }
      applyFrame(f);
      st.setCurrentFrame(f);
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [playNonce, playing, applyFrame]);

  // While paused (or scrubbing), show the selected frame.
  useEffect(() => {
    if (!simulation || playing) return;
    applyFrame(currentFrame);
  }, [currentFrame, playing, simulation, applyFrame]);

  // Tell the renderer which mesh indices are selected (for highlighting).
  useEffect(() => {
    const ids = Object.keys(meshes);
    setSelection(selectedFeatures.map((id) => ids.indexOf(id)).filter((i) => i >= 0));
  }, [selectedFeatures, meshes, setSelection]);

  // --- transform gizmo ---------------------------------------------------

  // Reposition the gizmo's SVG axes each frame (called from the render loop).
  const updateGizmo = useCallback(() => {
    const svg = gizmoRef.current;
    if (!svg) return;
    const st = useDocumentStore.getState();
    const fid = st.selectedFeature;
    const mesh = fid ? st.meshes[fid] : null;
    if (!mesh) {
      svg.style.display = 'none';
      return;
    }
    const info = aabbInfo(mesh.positions);
    let center = info.center;
    let L = Math.max(info.maxDim * 0.8, 1);
    if (drag.current && drag.current.id === fid && drag.current.mode === 'translate') {
      const wd = drag.current.worldDelta;
      center = [center[0] + wd[0], center[1] + wd[1], center[2] + wd[2]];
      L = drag.current.length;
    }
    const pc = project(center);
    if (!pc) {
      svg.style.display = 'none';
      return;
    }
    svg.style.display = '';

    // Translate handles (axis line + tip circle).
    const lines = svg.querySelectorAll('line');
    const circles = svg.querySelectorAll('circle');
    for (let i = 0; i < 3; i++) {
      const pe = project([center[0] + AXES[i][0] * L, center[1] + AXES[i][1] * L, center[2] + AXES[i][2] * L]);
      const line = lines[i] as SVGLineElement;
      const c = circles[i] as SVGCircleElement;
      if (!pe) {
        line.style.opacity = '0';
        c.style.opacity = '0';
        continue;
      }
      line.style.opacity = '1';
      c.style.opacity = '1';
      line.setAttribute('x1', `${pc[0]}`);
      line.setAttribute('y1', `${pc[1]}`);
      line.setAttribute('x2', `${pe[0]}`);
      line.setAttribute('y2', `${pe[1]}`);
      c.setAttribute('cx', `${pe[0]}`);
      c.setAttribute('cy', `${pe[1]}`);
    }

    // Rotation rings (projected circle of radius L in each axis plane).
    const polylines = svg.querySelectorAll('polyline'); // [vis0,hit0, vis1,hit1, vis2,hit2]
    const N = 40;
    for (let i = 0; i < 3; i++) {
      const { u, v } = RING_BASIS[i];
      let pts = '';
      let ok = true;
      for (let k = 0; k <= N; k++) {
        const t = (k / N) * 2 * Math.PI;
        const ct = Math.cos(t) * L, st = Math.sin(t) * L;
        const pp = project([
          center[0] + ct * u[0] + st * v[0],
          center[1] + ct * u[1] + st * v[1],
          center[2] + ct * u[2] + st * v[2],
        ]);
        if (!pp) { ok = false; break; }
        pts += `${pp[0]},${pp[1]} `;
      }
      const vis = polylines[i * 2] as SVGPolylineElement;
      const hit = polylines[i * 2 + 1] as SVGPolylineElement;
      vis.style.opacity = ok ? '0.85' : '0';
      vis.setAttribute('points', ok ? pts : '');
      hit.setAttribute('points', ok ? pts : '');
    }
  }, [project]);

  const startAxisDrag = useCallback(
    (axis: number, e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const canvas = canvasRef.current;
      if (!canvas) return;
      const st = useDocumentStore.getState();
      const fid = st.selectedFeature;
      const mesh = fid ? st.meshes[fid] : null;
      if (!fid || !mesh) return;
      const { center, maxDim } = aabbInfo(mesh.positions);
      const L = Math.max(maxDim * 0.8, 1);
      const axisVec = AXES[axis];
      const pc = project(center);
      const pe = project([center[0] + axisVec[0] * L, center[1] + axisVec[1] * L, center[2] + axisVec[2] * L]);
      if (!pc || !pe) return;
      const rect = canvas.getBoundingClientRect();
      const feat = st.features.find((f) => f.id === fid);
      drag.current = {
        mode: 'translate',
        id: fid,
        axisVec,
        axisIdx: axis,
        rest: mesh,
        isImport: feat?.type === 'Import',
        axisScreen: [pe[0] - pc[0], pe[1] - pc[1]],
        downPx: [e.clientX - rect.left, e.clientY - rect.top],
        length: L,
        worldDelta: [0, 0, 0],
        center,
        u: RING_BASIS[axis].u,
        v: RING_BASIS[axis].v,
        lastAngle: 0,
        totalAngle: 0,
      };
    },
    [project]
  );

  const startRingDrag = useCallback(
    (axis: number, e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const canvas = canvasRef.current;
      if (!canvas) return;
      const st = useDocumentStore.getState();
      const fid = st.selectedFeature;
      const mesh = fid ? st.meshes[fid] : null;
      if (!fid || !mesh) return;
      const { center } = aabbInfo(mesh.positions);
      const rect = canvas.getBoundingClientRect();
      const ray = unprojectRay(e.clientX - rect.left, e.clientY - rect.top);
      if (!ray) return;
      const A = AXES[axis];
      const { u, v } = RING_BASIS[axis];
      const ang = ringAngle(ray, center, A, u, v);
      if (ang === null) return;
      const feat = st.features.find((f) => f.id === fid);
      drag.current = {
        mode: 'rotate',
        id: fid,
        axisVec: A,
        axisIdx: axis,
        rest: mesh,
        isImport: feat?.type === 'Import',
        axisScreen: [0, 0],
        downPx: [0, 0],
        length: 0,
        worldDelta: [0, 0, 0],
        center,
        u,
        v,
        lastAngle: ang,
        totalAngle: 0,
      };
    },
    [unprojectRay]
  );

  // Window-level drag handling: live JS preview while dragging, commit on release.
  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      const d = drag.current;
      const canvas = canvasRef.current;
      if (!d || !canvas) return;
      const rect = canvas.getBoundingClientRect();
      const m = useDocumentStore.getState().meshes;

      if (d.mode === 'rotate') {
        const ray = unprojectRay(e.clientX - rect.left, e.clientY - rect.top);
        if (!ray) return;
        const ang = ringAngle(ray, d.center, d.axisVec, d.u, d.v);
        if (ang === null) return;
        let delta = ang - d.lastAngle;
        if (delta > Math.PI) delta -= 2 * Math.PI;
        if (delta < -Math.PI) delta += 2 * Math.PI;
        d.totalAngle += delta;
        d.lastAngle = ang;
        setMeshes(
          Object.entries(m).map(([id, g]) => (id === d.id ? rotateGeom(g, d.axisIdx, d.totalAngle) : g)),
          false
        );
        return;
      }

      const dx = e.clientX - rect.left - d.downPx[0];
      const dy = e.clientY - rect.top - d.downPx[1];
      const denom = d.axisScreen[0] ** 2 + d.axisScreen[1] ** 2 || 1;
      const t = ((dx * d.axisScreen[0] + dy * d.axisScreen[1]) / denom) * d.length;
      d.worldDelta = [d.axisVec[0] * t, d.axisVec[1] * t, d.axisVec[2] * t];
      setMeshes(
        Object.entries(m).map(([id, g]) => (id === d.id ? offsetGeom(g, d.worldDelta) : g)),
        false
      );
    };
    const onUp = () => {
      const d = drag.current;
      if (!d) return;
      drag.current = null;
      const restore = () => setMeshes(Object.values(useDocumentStore.getState().meshes), false);

      if (d.mode === 'rotate') {
        if (Math.abs(d.totalAngle) < 1e-4) return restore();
        void (async () => {
          try {
            // The rotate command handles import vs solid + history; the live
            // preview holds until the store update re-syncs the meshes.
            await cad.spinFeature(d.id, d.axisIdx, d.totalAngle);
          } catch (err) {
            useDocumentStore.getState().setNotice(err instanceof Error ? err.message : 'Rotate failed');
            restore();
          }
        })();
        return;
      }

      const wd = d.worldDelta;
      if (Math.abs(wd[0]) + Math.abs(wd[1]) + Math.abs(wd[2]) < 1e-6) return restore();
      void (async () => {
        try {
          await cad.moveFeature(d.id, wd[0], wd[1], wd[2]);
        } catch (err) {
          useDocumentStore.getState().setNotice(err instanceof Error ? err.message : 'Move failed');
          restore();
        }
      })();
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
    return () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
  }, [cad, setMeshes, unprojectRay]);

  // Render loop
  useEffect(() => {
    if (!isReady) return;

    let animationId: number;

    const renderLoop = () => {
      render();
      updateGizmo();
      animationId = requestAnimationFrame(renderLoop);
    };

    animationId = requestAnimationFrame(renderLoop);
    return () => cancelAnimationFrame(animationId);
  }, [isReady, render, updateGizmo]);

  // Click (no drag) selects the mesh under the cursor.
  const handlePick = useCallback(
    (e: React.MouseEvent) => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const ndcX = ((e.clientX - rect.left) / rect.width) * 2 - 1;
      const ndcY = 1 - ((e.clientY - rect.top) / rect.height) * 2;

      const idx = pick(ndcX, ndcY);
      const ids = Object.keys(meshes);
      const additive = e.metaKey || e.ctrlKey;

      if (idx === null || idx >= ids.length) {
        if (!additive) selectFeature(null); // click empty space clears
        return;
      }
      if (additive) toggleSelectFeature(ids[idx]);
      else selectFeature(ids[idx]);
    },
    [pick, meshes, selectFeature, toggleSelectFeature]
  );

  // Mouse event handlers
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 0) {
      // Left button - orbit
      isDragging.current = true;
    } else if (e.button === 1) {
      // Middle button - pan
      isPanning.current = true;
    }
    lastPos.current = { x: e.clientX, y: e.clientY };
    downPos.current = { x: e.clientX, y: e.clientY };
    moved.current = false;
  }, []);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const dx = e.clientX - lastPos.current.x;
    const dy = e.clientY - lastPos.current.y;
    lastPos.current = { x: e.clientX, y: e.clientY };

    if (
      Math.abs(e.clientX - downPos.current.x) +
        Math.abs(e.clientY - downPos.current.y) >
      4
    ) {
      moved.current = true;
    }

    if (isDragging.current) {
      orbit(dx * 0.01, dy * 0.01);
    } else if (isPanning.current) {
      pan(dx, -dy);
    }
  }, [orbit, pan]);

  const handleMouseUp = useCallback(
    (e: React.MouseEvent) => {
      const wasClick = e.button === 0 && !moved.current;
      isDragging.current = false;
      isPanning.current = false;
      if (wasClick) handlePick(e);
    },
    [handlePick]
  );

  const handleMouseLeave = useCallback(() => {
    isDragging.current = false;
    isPanning.current = false;
  }, []);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    zoom(e.deltaY * 0.001);
  }, [zoom]);

  // Drag & drop import (.stl / .obj)
  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      setIsDragOver(false);
      const files = Array.from(e.dataTransfer.files);
      for (const file of files) {
        const mb = (file.size / 1048576).toFixed(1);
        try {
          if (/\.rcad$/i.test(file.name)) {
            setNotice(`Opening ${file.name}…`);
            await cad.loadDoc(await file.text());
            setNotice(`Opened ${file.name}.`);
            continue;
          }
          const isServer = /\.(step|stp|gltf|glb)$/i.test(file.name);
          setNotice(
            `Importing ${file.name} (${mb} MB)${isServer ? ' — this can take a while' : ''}…`
          );
          const parts = await importMeshFile(file);
          let verts = 0;
          for (const p of parts) {
            await cad.importMesh(p.name, p.geometry, p.color);
            verts += p.geometry.vertexCount;
          }
          setNotice(
            `Imported ${file.name}${parts.length > 1 ? ` — ${parts.length} parts` : ''} — ${verts.toLocaleString()} verts.`
          );
        } catch (err) {
          console.error('Import failed:', err);
          setNotice(err instanceof Error ? err.message : 'Import failed');
        }
      }
    },
    [cad, setNotice]
  );

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    if (e.currentTarget === e.target) setIsDragOver(false);
  }, []);

  // Keyboard shortcuts: 1-7 standard views, F to fit.
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;

      switch (e.key) {
        case '1': setView('front'); break;
        case '2': setView('back'); break;
        case '3': setView('right'); break;
        case '4': setView('left'); break;
        case '5': setView('top'); break;
        case '6': setView('bottom'); break;
        case '7': setView('iso'); break;
        case 'f':
        case 'F':
          fitToContent();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [setView, fitToContent]);

  return (
    <div
      ref={containerRef}
      className="w-full h-full relative"
      onContextMenu={(e) => e.preventDefault()}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
    >
      <canvas
        ref={canvasRef}
        className="w-full h-full"
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseLeave}
        onWheel={handleWheel}
      />

      {/* Transform gizmo: drag an axis handle to move the selected feature */}
      <svg
        ref={gizmoRef}
        className="absolute inset-0 w-full h-full pointer-events-none"
        style={{ display: 'none' }}
      >
        {/* Rotation rings first (so the translate handles draw on top) */}
        {AXES.map((_, i) => (
          <g key={`ring-${i}`}>
            <polyline fill="none" stroke={AXIS_COLORS[i]} strokeWidth={2} pointerEvents="none" />
            <polyline
              fill="none"
              stroke="transparent"
              strokeWidth={14}
              className="pointer-events-auto cursor-grab"
              style={{ pointerEvents: 'stroke' }}
              onMouseDown={(e) => startRingDrag(i, e)}
            />
          </g>
        ))}
        {/* Translate handles */}
        {AXES.map((_, i) => (
          <g key={`axis-${i}`}>
            <line stroke={AXIS_COLORS[i]} strokeWidth={2.5} strokeOpacity={0.95} />
            <circle
              r={7}
              fill={AXIS_COLORS[i]}
              className="pointer-events-auto cursor-grab"
              onMouseDown={(e) => startAxisDrag(i, e)}
            />
          </g>
        ))}
      </svg>

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

      {/* Physics playback controls (only while a simulation exists) */}
      {simulation && (
        <div className="absolute top-4 left-1/2 -translate-x-1/2 flex items-center gap-3 px-3 py-2 bg-cad-panel/90 border border-cad-border rounded-lg">
          <button
            className="w-7 h-7 flex items-center justify-center text-sm bg-cad-bg hover:bg-cad-border rounded text-cad-text"
            title={playing ? 'Pause' : 'Play'}
            onClick={() => {
              const st = useDocumentStore.getState();
              const last = (st.simulation?.frames.length ?? 1) - 1;
              if (!st.playing && st.currentFrame >= last) st.replaySimulation();
              else st.setPlaying(!st.playing);
            }}
          >
            {playing ? '⏸' : '▶'}
          </button>
          <input
            type="range"
            min={0}
            max={simulation.frames.length - 1}
            value={currentFrame}
            title="Scrub"
            onChange={(e) => {
              const st = useDocumentStore.getState();
              st.setPlaying(false);
              st.setCurrentFrame(Number(e.target.value));
            }}
            className="w-48 accent-cad-accent"
          />
          <span className="text-xs text-cad-text-muted tabular-nums w-14 text-right">
            {currentFrame + 1}/{simulation.frames.length}
          </span>
          <button
            className="w-7 h-7 flex items-center justify-center text-sm bg-cad-bg hover:bg-cad-border rounded text-cad-text"
            title="Replay"
            onClick={() => useDocumentStore.getState().replaySimulation()}
          >
            ⟲
          </button>
          <button
            className="w-7 h-7 flex items-center justify-center text-sm bg-cad-bg hover:bg-cad-border rounded text-cad-text"
            title="Stop"
            onClick={() => useDocumentStore.getState().clearSimulation()}
          >
            ■
          </button>
        </div>
      )}

      {/* View Controls Overlay */}
      <div className="absolute bottom-4 left-4 flex gap-2">
        <ViewButton label="Front" shortcut="1" onClick={() => setView('front')} />
        <ViewButton label="Top" shortcut="5" onClick={() => setView('top')} />
        <ViewButton label="Iso" shortcut="7" onClick={() => setView('iso')} />
        <ViewButton label="Fit" shortcut="F" onClick={fitToContent} />
      </div>

      {/* Coordinate System Indicator */}
      <div className="absolute bottom-4 right-4 w-16 h-16">
        <CoordinateGizmo />
      </div>

      {/* Drag & drop hint */}
      {isDragOver && (
        <div className="absolute inset-0 flex items-center justify-center bg-cad-accent/10 border-2 border-dashed border-cad-accent pointer-events-none">
          <div className="px-4 py-2 bg-cad-panel border border-cad-border rounded text-cad-text">
            Drop .stl or .obj to import
          </div>
        </div>
      )}

      {/* Renderer indicator */}
      {rendererType && (
        <div className="absolute top-4 right-4 px-2 py-1 bg-cad-panel/80 border border-cad-border rounded text-xs text-cad-text-muted">
          {rendererType === 'webgpu' ? '🚀 WebGPU' : '🔷 WebGL'}
        </div>
      )}
    </div>
  );
}

function ViewButton({
  label,
  shortcut,
  onClick,
}: {
  label: string;
  shortcut: string;
  onClick?: () => void;
}) {
  return (
    <button
      className="px-3 py-1 bg-cad-panel/80 hover:bg-cad-panel border border-cad-border rounded text-sm text-cad-text"
      title={`${label} (${shortcut})`}
      onClick={onClick}
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
