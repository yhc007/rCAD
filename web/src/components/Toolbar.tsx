import React from 'react';
import {
  Box,
  Circle,
  Cylinder,
  Cone,
  Minus,
  Plus,
  Layers,
  Undo,
  Redo,
  Save,
  FolderOpen,
  Download,
  Settings,
  Grid,
  Eye,
  Play,
} from 'lucide-react';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { useDocumentStore } from '../stores/documentStore';
import { useCAD } from '../hooks/useCAD';
import { importMeshFile } from '../lib/importFile';
import { geometryToSTL, geometryToOBJ, downloadBlob } from '../lib/meshExporters';
import { runSimulation } from '../lib/physics';

export function Toolbar() {
  const {
    features,
    meshes,
    selectedFeature,
    selectedFeatures,
    setNotice,
    canUndo,
    canRedo,
  } = useDocumentStore();
  const { cad } = useCAD();
  const fileInputRef = React.useRef<HTMLInputElement>(null);
  const [boolBusy, setBoolBusy] = React.useState(false);
  const [simBusy, setSimBusy] = React.useState(false);

  const handleSimulate = async () => {
    const { meshes, features: feats, startSimulation } = useDocumentStore.getState();
    if (Object.keys(meshes).length === 0) {
      setNotice('Add or import geometry before simulating.');
      return;
    }
    const props = Object.fromEntries(
      feats.map((f) => [
        f.id,
        {
          type: f.type,
          fixed: !!f.fixed,
          mass: f.mass ?? 0,
          rotated: !!f.rotation?.some((v) => v !== 0),
        },
      ])
    );
    setSimBusy(true);
    setNotice('Running physics simulation…');
    try {
      const sim = await runSimulation(meshes, props);
      startSimulation(sim);
      setNotice(null);
    } catch (e) {
      console.error('Simulation failed:', e);
      setNotice(
        e instanceof Error
          ? `Simulation failed: ${e.message} (is the physics service running on :8000?)`
          : 'Simulation failed'
      );
    } finally {
      setSimBusy(false);
    }
  };

  // A boolean needs exactly two WASM-backed solids selected (imports are
  // mesh-only and have no B-Rep to operate on).
  const boolReady =
    selectedFeatures.length === 2 &&
    selectedFeatures.every((id) => {
      const f = features.find((x) => x.id === id);
      return !!f && f.type !== 'Import';
    });

  const handleBoolean = async (op: 'union' | 'subtract' | 'intersect') => {
    if (!cad || selectedFeatures.length !== 2 || boolBusy) return;
    const [targetId, toolId] = selectedFeatures;
    setBoolBusy(true);
    setNotice(null);
    try {
      // The boolean command replaces the two operands with the single result.
      await cad.applyBoolean(op, targetId, toolId);
    } catch (e) {
      console.error(`Boolean ${op} failed:`, e);
      const msg = e instanceof Error ? e.message : String(e);
      setNotice(
        msg === 'timeout'
          ? 'Boolean operation timed out and was cancelled — the geometry kernel struggles with curved solids. Your model is intact.'
          : `Boolean ${op} failed: ${msg}`
      );
    } finally {
      setBoolBusy(false);
    }
  };

  const handleExport = (format: 'stl' | 'obj') => {
    const geometry = selectedFeature ? meshes[selectedFeature] : undefined;
    if (!geometry || geometry.vertexCount === 0) {
      setNotice('Select a feature with geometry in the Model Tree to export.');
      return;
    }
    const feat = features.find((f) => f.id === selectedFeature);
    const base = (feat?.name || 'model').replace(/[^\w.-]+/g, '_');
    if (format === 'stl') {
      downloadBlob(geometryToSTL(geometry), `${base}.stl`);
    } else {
      downloadBlob(geometryToOBJ(geometry), `${base}.obj`);
    }
  };

  const handleSave = () => {
    const name = (useDocumentStore.getState().documentName || 'model').replace(
      /[^\w.-]+/g,
      '_'
    );
    const blob = new Blob([cad.serialize()], { type: 'application/json' });
    downloadBlob(blob, `${name}.rcad`);
    useDocumentStore.getState().markClean();
  };

  const handleOpenClick = () => fileInputRef.current?.click();

  const handleFilesSelected = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files ?? []);
    for (const file of files) {
      const mb = (file.size / 1048576).toFixed(1);
      try {
        if (/\.rcad$/i.test(file.name)) {
          setNotice(`Opening ${file.name}…`);
          await cad.loadDoc(await file.text());
          setNotice(`Opened ${file.name}.`);
          continue;
        }
        // A persistent '…' notice (the Toast keeps it until replaced) — large
        // STEP files upload + tessellate slowly with no other feedback.
        const isStep = /\.(step|stp)$/i.test(file.name);
        setNotice(
          `Importing ${file.name} (${mb} MB)${isStep ? ' — STEP parsing can take a while' : ''}…`
        );
        const { name, geometry } = await importMeshFile(file);
        await cad.importMesh(name, geometry);
        setNotice(`Imported ${name} — ${geometry.vertexCount.toLocaleString()} verts.`);
      } catch (err) {
        console.error('Import failed:', err);
        setNotice(err instanceof Error ? err.message : 'Import failed');
      }
    }
    e.target.value = ''; // allow re-importing the same file
  };

  const handleCreate = (prim: string, params: number[]) => async () => {
    try {
      await cad.addPrimitive(prim, params);
    } catch (e) {
      console.error(`Failed to create ${prim}:`, e);
    }
  };
  const handleCreateBox = handleCreate('box', [50, 50, 50]);
  const handleCreateCylinder = handleCreate('cylinder', [25, 50]);
  const handleCreateSphere = handleCreate('sphere', [25]);
  const handleCreateCone = handleCreate('cone', [25, 0, 50]);

  return (
    <div className="flex items-center h-12 px-4 border-b border-cad-border bg-cad-panel">
      {/* Hidden file input for STL/OBJ import */}
      <input
        ref={fileInputRef}
        type="file"
        accept=".stl,.obj,.step,.stp,.gltf,.glb,.iges,.igs,.rcad"
        multiple
        className="hidden"
        onChange={handleFilesSelected}
      />

      {/* File operations */}
      <ToolbarGroup>
        <ToolbarButton icon={<FolderOpen size={18} />} label="Open" onClick={handleOpenClick} />
        <ToolbarButton icon={<Save size={18} />} label="Save (.rcad)" onClick={handleSave} />
        <DropdownMenu.Root>
          <DropdownMenu.Trigger asChild>
            <button
              className="p-2 rounded transition-colors hover:bg-cad-bg text-cad-text"
              title="Export selected"
            >
              <Download size={18} />
            </button>
          </DropdownMenu.Trigger>
          <DropdownMenu.Portal>
            <DropdownMenu.Content
              sideOffset={4}
              className="min-w-[10rem] bg-cad-panel border border-cad-border rounded-lg shadow-xl p-1 z-50"
            >
              <DropdownMenu.Item
                className="px-3 py-1.5 text-sm text-cad-text rounded outline-none cursor-pointer hover:bg-cad-bg data-[highlighted]:bg-cad-bg"
                onSelect={() => handleExport('stl')}
              >
                Export as STL
              </DropdownMenu.Item>
              <DropdownMenu.Item
                className="px-3 py-1.5 text-sm text-cad-text rounded outline-none cursor-pointer hover:bg-cad-bg data-[highlighted]:bg-cad-bg"
                onSelect={() => handleExport('obj')}
              >
                Export as OBJ
              </DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Portal>
        </DropdownMenu.Root>
      </ToolbarGroup>

      <ToolbarDivider />

      {/* Edit operations */}
      <ToolbarGroup>
        <ToolbarButton
          icon={<Undo size={18} />}
          label="Undo"
          disabled={!canUndo}
          onClick={() => void cad.undo()}
        />
        <ToolbarButton
          icon={<Redo size={18} />}
          label="Redo"
          disabled={!canRedo}
          onClick={() => void cad.redo()}
        />
      </ToolbarGroup>

      <ToolbarDivider />

      {/* Primitives */}
      <ToolbarGroup>
        <ToolbarButton
          icon={<Box size={18} />}
          label="Box"
          onClick={handleCreateBox}
        />
        <ToolbarButton
          icon={<Cylinder size={18} />}
          label="Cylinder"
          onClick={handleCreateCylinder}
        />
        <ToolbarButton
          icon={<Circle size={18} />}
          label="Sphere"
          onClick={handleCreateSphere}
        />
        <ToolbarButton
          icon={<Cone size={18} />}
          label="Cone"
          onClick={handleCreateCone}
        />
      </ToolbarGroup>

      <ToolbarDivider />

      {/* Boolean operations (select exactly 2 solids) */}
      <ToolbarGroup>
        <ToolbarButton
          icon={<Plus size={18} />}
          label="Union (select 2)"
          disabled={!boolReady || boolBusy}
          onClick={() => handleBoolean('union')}
        />
        <ToolbarButton
          icon={<Minus size={18} />}
          label="Subtract (1st − 2nd)"
          disabled={!boolReady || boolBusy}
          onClick={() => handleBoolean('subtract')}
        />
        <ToolbarButton
          icon={<Layers size={18} />}
          label="Intersect (select 2)"
          disabled={!boolReady || boolBusy}
          onClick={() => handleBoolean('intersect')}
        />
      </ToolbarGroup>

      <ToolbarDivider />

      {/* Physics simulation */}
      <ToolbarGroup>
        <ToolbarButton
          icon={<Play size={18} />}
          label="Simulate physics (drop test)"
          disabled={simBusy}
          onClick={handleSimulate}
        />
      </ToolbarGroup>

      <div className="flex-1" />

      {/* View options */}
      <ToolbarGroup>
        <ToolbarButton icon={<Grid size={18} />} label="Grid" toggle />
        <ToolbarButton icon={<Eye size={18} />} label="Wireframe" toggle />
        <ToolbarButton icon={<Settings size={18} />} label="Settings" />
      </ToolbarGroup>
    </div>
  );
}

function ToolbarGroup({ children }: { children: React.ReactNode }) {
  return <div className="flex items-center gap-1">{children}</div>;
}

function ToolbarDivider() {
  return <div className="w-px h-6 mx-2 bg-cad-border" />;
}

interface ToolbarButtonProps {
  icon: React.ReactNode;
  label: string;
  disabled?: boolean;
  toggle?: boolean;
  onClick?: () => void;
}

function ToolbarButton({
  icon,
  label,
  disabled,
  toggle,
  onClick,
}: ToolbarButtonProps) {
  const [active, setActive] = React.useState(false);

  const handleClick = () => {
    if (toggle) {
      setActive(!active);
    }
    onClick?.();
  };

  return (
    <button
      className={`
        p-2 rounded transition-colors
        ${disabled ? 'opacity-50 cursor-not-allowed' : 'hover:bg-cad-bg'}
        ${active ? 'bg-cad-accent/20 text-cad-accent' : 'text-cad-text'}
      `}
      title={label}
      disabled={disabled}
      onClick={handleClick}
    >
      {icon}
    </button>
  );
}
