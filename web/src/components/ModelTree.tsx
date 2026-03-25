import React from 'react';
import {
  Box,
  Circle,
  Cylinder,
  Cone,
  Donut,
  Plus,
  Minus,
  ChevronRight,
  ChevronDown,
  Eye,
  EyeOff,
} from 'lucide-react';
import { useDocumentStore } from '../stores/documentStore';
import * as ScrollArea from '@radix-ui/react-scroll-area';

export function ModelTree() {
  const { features, selectedFeature, selectFeature } = useDocumentStore();

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between px-4 py-2 border-b border-cad-border">
        <h2 className="text-sm font-semibold">Model Tree</h2>
        <button className="p-1 hover:bg-cad-bg rounded" title="Add Feature">
          <Plus size={16} />
        </button>
      </div>

      <ScrollArea.Root className="flex-1 overflow-hidden">
        <ScrollArea.Viewport className="h-full w-full">
          <div className="p-2">
            {/* Document Root */}
            <TreeNode
              icon={<FolderIcon />}
              label="Untitled Document"
              level={0}
              expanded
            >
              {/* Features */}
              {features.map((feature) => (
                <TreeNode
                  key={feature.id}
                  icon={getFeatureIcon(feature.type)}
                  label={feature.name}
                  level={1}
                  selected={selectedFeature === feature.id}
                  onClick={() => selectFeature(feature.id)}
                />
              ))}

              {features.length === 0 && (
                <div className="px-6 py-2 text-sm text-cad-text-muted">
                  No features yet
                </div>
              )}
            </TreeNode>
          </div>
        </ScrollArea.Viewport>
        <ScrollArea.Scrollbar
          orientation="vertical"
          className="w-2 bg-transparent"
        >
          <ScrollArea.Thumb className="bg-cad-border rounded" />
        </ScrollArea.Scrollbar>
      </ScrollArea.Root>
    </div>
  );
}

interface TreeNodeProps {
  icon: React.ReactNode;
  label: string;
  level: number;
  expanded?: boolean;
  selected?: boolean;
  suppressed?: boolean;
  children?: React.ReactNode;
  onClick?: () => void;
}

function TreeNode({
  icon,
  label,
  level,
  expanded: initialExpanded = false,
  selected,
  suppressed,
  children,
  onClick,
}: TreeNodeProps) {
  const [expanded, setExpanded] = React.useState(initialExpanded);
  const hasChildren = React.Children.count(children) > 0;

  return (
    <div>
      <div
        className={`
          flex items-center gap-1 px-2 py-1 rounded cursor-pointer
          ${selected ? 'bg-cad-accent/20 text-cad-accent' : 'hover:bg-cad-bg'}
          ${suppressed ? 'opacity-50' : ''}
        `}
        style={{ paddingLeft: `${level * 16 + 8}px` }}
        onClick={onClick}
      >
        {hasChildren ? (
          <button
            className="p-0.5 hover:bg-cad-border rounded"
            onClick={(e) => {
              e.stopPropagation();
              setExpanded(!expanded);
            }}
          >
            {expanded ? (
              <ChevronDown size={14} />
            ) : (
              <ChevronRight size={14} />
            )}
          </button>
        ) : (
          <div className="w-5" />
        )}

        <span className="text-cad-text-muted">{icon}</span>
        <span className="text-sm flex-1 truncate">{label}</span>

        <button
          className="p-0.5 opacity-0 group-hover:opacity-100 hover:bg-cad-border rounded"
          title={suppressed ? 'Show' : 'Hide'}
        >
          {suppressed ? <EyeOff size={14} /> : <Eye size={14} />}
        </button>
      </div>

      {hasChildren && expanded && <div>{children}</div>}
    </div>
  );
}

function FolderIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      className="text-yellow-500"
    >
      <path
        d="M2 4a1 1 0 011-1h4l1 1h5a1 1 0 011 1v7a1 1 0 01-1 1H3a1 1 0 01-1-1V4z"
        fill="currentColor"
      />
    </svg>
  );
}

function getFeatureIcon(type: string) {
  const iconProps = { size: 16 };

  switch (type) {
    case 'Box':
      return <Box {...iconProps} className="text-blue-400" />;
    case 'Cylinder':
      return <Cylinder {...iconProps} className="text-green-400" />;
    case 'Sphere':
      return <Circle {...iconProps} className="text-purple-400" />;
    case 'Cone':
      return <Cone {...iconProps} className="text-orange-400" />;
    case 'Torus':
      return <Donut {...iconProps} className="text-pink-400" />;
    case 'Union':
      return <Plus {...iconProps} className="text-emerald-400" />;
    case 'Subtract':
      return <Minus {...iconProps} className="text-red-400" />;
    default:
      return <Box {...iconProps} />;
  }
}
