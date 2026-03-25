import React from 'react';
import {
  Box,
  Circle,
  Cylinder,
  Cone,
  Minus,
  Plus,
  Intersect,
  Undo,
  Redo,
  Save,
  FolderOpen,
  Download,
  Settings,
  Grid,
  Eye,
} from 'lucide-react';
import { useDocumentStore } from '../stores/documentStore';
import { useCAD } from '../hooks/useCAD';

export function Toolbar() {
  const { document, addFeature, canUndo, canRedo, undo, redo } = useDocumentStore();
  const { cad } = useCAD();

  const handleCreateBox = async () => {
    if (!cad) return;
    try {
      const id = await cad.createBox(50, 50, 50);
      addFeature({ id, name: 'Box', type: 'Box' });
    } catch (e) {
      console.error('Failed to create box:', e);
    }
  };

  const handleCreateCylinder = async () => {
    if (!cad) return;
    try {
      const id = await cad.createCylinder(25, 50);
      addFeature({ id, name: 'Cylinder', type: 'Cylinder' });
    } catch (e) {
      console.error('Failed to create cylinder:', e);
    }
  };

  const handleCreateSphere = async () => {
    if (!cad) return;
    try {
      const id = await cad.createSphere(25);
      addFeature({ id, name: 'Sphere', type: 'Sphere' });
    } catch (e) {
      console.error('Failed to create sphere:', e);
    }
  };

  const handleCreateCone = async () => {
    if (!cad) return;
    try {
      const id = await cad.createCone(25, 0, 50);
      addFeature({ id, name: 'Cone', type: 'Cone' });
    } catch (e) {
      console.error('Failed to create cone:', e);
    }
  };

  return (
    <div className="flex items-center h-12 px-4 border-b border-cad-border bg-cad-panel">
      {/* File operations */}
      <ToolbarGroup>
        <ToolbarButton icon={<FolderOpen size={18} />} label="Open" />
        <ToolbarButton icon={<Save size={18} />} label="Save" />
        <ToolbarButton icon={<Download size={18} />} label="Export" />
      </ToolbarGroup>

      <ToolbarDivider />

      {/* Edit operations */}
      <ToolbarGroup>
        <ToolbarButton
          icon={<Undo size={18} />}
          label="Undo"
          disabled={!canUndo}
          onClick={undo}
        />
        <ToolbarButton
          icon={<Redo size={18} />}
          label="Redo"
          disabled={!canRedo}
          onClick={redo}
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

      {/* Boolean operations */}
      <ToolbarGroup>
        <ToolbarButton icon={<Plus size={18} />} label="Union" />
        <ToolbarButton icon={<Minus size={18} />} label="Subtract" />
        <ToolbarButton icon={<Intersect size={18} />} label="Intersect" />
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
