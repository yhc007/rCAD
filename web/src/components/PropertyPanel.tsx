import React from 'react';
import { useDocumentStore } from '../stores/documentStore';
import { useCAD } from '../hooks/useCAD';
import * as Slider from '@radix-ui/react-slider';
import * as Tabs from '@radix-ui/react-tabs';

// Material colour helpers — features store linear RGB in 0..1; the <input
// type=color> works in #rrggbb.
const clamp01 = (v: number) => Math.max(0, Math.min(1, v));
function rgbToHex(c: [number, number, number]): string {
  const h = (v: number) => Math.round(clamp01(v) * 255).toString(16).padStart(2, '0');
  return `#${h(c[0])}${h(c[1])}${h(c[2])}`;
}
function hexToRgb(hex: string): [number, number, number] {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex.trim());
  if (!m) return [0.6, 0.6, 0.7];
  return [parseInt(m[1], 16) / 255, parseInt(m[2], 16) / 255, parseInt(m[3], 16) / 255];
}

export function PropertyPanel() {
  const { selectedFeature, features } = useDocumentStore();

  const feature = selectedFeature
    ? features.find((f) => f.id === selectedFeature)
    : null;

  return (
    <div className="h-full flex flex-col">
      <Tabs.Root defaultValue="properties" className="flex flex-col h-full">
        <Tabs.List className="flex border-b border-cad-border">
          <Tabs.Trigger
            value="properties"
            className="flex-1 px-4 py-2 text-sm text-cad-text-muted data-[state=active]:text-cad-text data-[state=active]:border-b-2 data-[state=active]:border-cad-accent"
          >
            Properties
          </Tabs.Trigger>
          <Tabs.Trigger
            value="material"
            className="flex-1 px-4 py-2 text-sm text-cad-text-muted data-[state=active]:text-cad-text data-[state=active]:border-b-2 data-[state=active]:border-cad-accent"
          >
            Material
          </Tabs.Trigger>
        </Tabs.List>

        <Tabs.Content value="properties" className="flex-1 overflow-auto p-4">
          {feature ? (
            <FeatureProperties feature={feature} />
          ) : (
            <div className="text-cad-text-muted text-sm">
              Select a feature to view properties
            </div>
          )}
        </Tabs.Content>

        <Tabs.Content value="material" className="flex-1 overflow-auto p-4">
          {feature ? (
            <MaterialProperties feature={feature} />
          ) : (
            <div className="text-cad-text-muted text-sm">
              Select a feature to edit material
            </div>
          )}
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
}

interface Feature {
  id: string;
  name: string;
  type: string;
  position?: [number, number, number];
  rotation?: [number, number, number];
  params?: number[];
  color?: [number, number, number];
  sensorTag?: string;
  fixed?: boolean;
  mass?: number;
}

function FeatureProperties({ feature }: { feature: Feature }) {
  const { cad } = useCAD();
  const pos: [number, number, number] = feature.position ?? [0, 0, 0];

  const fail = (e: unknown, what: string) =>
    useDocumentStore.getState().setNotice(e instanceof Error ? e.message : `${what} failed`);

  // Moving a feature actually translates its geometry (WASM solid for
  // primitives/booleans, mesh offset for imports) so booleans/physics follow.
  const commitPosition = async (axis: number, value: number) => {
    const d: [number, number, number] = [0, 0, 0];
    d[axis] = value - pos[axis];
    if (!d[0] && !d[1] && !d[2]) return;
    try {
      await cad.moveFeature(feature.id, d[0], d[1], d[2]);
    } catch (e) {
      fail(e, 'Move');
    }
  };

  // Rotation is per-axis input in degrees; each commit applies the delta about
  // that world axis, rotating the part in place (about its centre).
  const rot: [number, number, number] = feature.rotation ?? [0, 0, 0];
  const commitRotation = async (axis: number, valueDeg: number) => {
    const deltaDeg = valueDeg - rot[axis];
    if (deltaDeg === 0) return;
    try {
      await cad.spinFeature(feature.id, axis, (deltaDeg * Math.PI) / 180);
    } catch (e) {
      fail(e, 'Rotate');
    }
  };

  // Editing a primitive dimension rebuilds the solid at the new size, preserving
  // its current placement. `prim` maps the display type back to the kernel kind.
  const prim = feature.type.toLowerCase();
  const dims = feature.params ?? [];
  const commitDimension = async (index: number, value: number) => {
    if (dims[index] === value || !(value >= 0)) return;
    const params = dims.slice();
    params[index] = value;
    try {
      await cad.resizeFeature(feature.id, prim, params);
    } catch (e) {
      fail(e, 'Resize');
    }
  };

  return (
    <div className="space-y-4">
      <PropertySection title="General">
        <PropertyInput label="Name" value={feature.name} />
        <PropertyReadOnly label="Type" value={feature.type} />
        <PropertyReadOnly label="ID" value={feature.id.substring(0, 8)} />
      </PropertySection>

      <PropertySection title="Physics">
        <label className="flex items-center justify-between text-sm text-cad-text">
          <span>Fixed (static anchor)</span>
          <input
            type="checkbox"
            checked={!!feature.fixed}
            onChange={(e) => cad.setFeatureProps(feature.id, { fixed: e.target.checked })}
          />
        </label>
        <label className="flex items-center justify-between text-sm text-cad-text">
          <span>Mass (0 = auto)</span>
          <input
            type="number"
            min={0}
            step={0.1}
            value={feature.mass ?? 0}
            disabled={!!feature.fixed}
            onChange={(e) =>
              cad.setFeatureProps(feature.id, { mass: Math.max(0, Number(e.target.value) || 0) })
            }
            className="w-24 px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm text-cad-text disabled:opacity-50"
          />
        </label>
      </PropertySection>

      {feature.type === 'Box' && dims.length === 3 && (
        <PropertySection title="Dimensions">
          <PropertySlider key={`${feature.id}-w-${dims[0]}`} label="Width" value={dims[0]} min={1} max={200} onCommit={(v) => commitDimension(0, v)} />
          <PropertySlider key={`${feature.id}-h-${dims[1]}`} label="Height" value={dims[1]} min={1} max={200} onCommit={(v) => commitDimension(1, v)} />
          <PropertySlider key={`${feature.id}-d-${dims[2]}`} label="Depth" value={dims[2]} min={1} max={200} onCommit={(v) => commitDimension(2, v)} />
        </PropertySection>
      )}

      {feature.type === 'Cylinder' && dims.length === 2 && (
        <PropertySection title="Dimensions">
          <PropertySlider key={`${feature.id}-r-${dims[0]}`} label="Radius" value={dims[0]} min={1} max={100} onCommit={(v) => commitDimension(0, v)} />
          <PropertySlider key={`${feature.id}-h-${dims[1]}`} label="Height" value={dims[1]} min={1} max={200} onCommit={(v) => commitDimension(1, v)} />
        </PropertySection>
      )}

      {feature.type === 'Sphere' && dims.length === 1 && (
        <PropertySection title="Dimensions">
          <PropertySlider key={`${feature.id}-r-${dims[0]}`} label="Radius" value={dims[0]} min={1} max={100} onCommit={(v) => commitDimension(0, v)} />
        </PropertySection>
      )}

      {feature.type === 'Cone' && dims.length === 3 && (
        <PropertySection title="Dimensions">
          <PropertySlider key={`${feature.id}-br-${dims[0]}`} label="Bottom Radius" value={dims[0]} min={0} max={100} onCommit={(v) => commitDimension(0, v)} />
          <PropertySlider key={`${feature.id}-tr-${dims[1]}`} label="Top Radius" value={dims[1]} min={0} max={100} onCommit={(v) => commitDimension(1, v)} />
          <PropertySlider key={`${feature.id}-ht-${dims[2]}`} label="Height" value={dims[2]} min={1} max={200} onCommit={(v) => commitDimension(2, v)} />
        </PropertySection>
      )}

      <PropertySection title="Transform">
        <AxisEditor label="Position (Enter / blur)" value={pos} onCommit={commitPosition} step={5} />
        <AxisEditor label="Rotation° (Enter / blur)" value={rot} onCommit={commitRotation} step={15} />
      </PropertySection>

      <PropertySection title="Telemetry (Twin)">
        <div>
          <label className="text-xs text-cad-text-muted">Sensor tag binding</label>
          <input
            key={`${feature.id}-tag`}
            type="text"
            defaultValue={feature.sensorTag ?? ''}
            placeholder="e.g. station1.proximity"
            title="Bind this part's colour to a live telemetry tag"
            onBlur={(e) =>
              cad.setFeatureProps(feature.id, { sensorTag: e.target.value.trim() || undefined })
            }
            onKeyDown={(e) => {
              if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
            }}
            className="w-full mt-1 px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm text-cad-text font-mono"
          />
        </div>
      </PropertySection>
    </div>
  );
}

function AxisEditor({
  label,
  value,
  onCommit,
  step,
}: {
  label: string;
  value: number[];
  onCommit: (axis: number, v: number) => void;
  step: number;
}) {
  return (
    <div>
      <label className="text-xs text-cad-text-muted">{label}</label>
      <div className="grid grid-cols-3 gap-1 mt-1">
        {['X', 'Y', 'Z'].map((ax, i) => (
          <input
            // Remount when the committed value changes so defaultValue updates.
            key={`${label}-${ax}-${value[i]}`}
            type="number"
            step={step}
            defaultValue={value[i]}
            title={`${label}-${ax}`}
            onBlur={(e) => onCommit(i, Number(e.target.value) || 0)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
            }}
            className="w-full px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm text-cad-text"
          />
        ))}
      </div>
    </div>
  );
}

function MaterialProperties({ feature }: { feature: Feature }) {
  const { cad } = useCAD();
  const color = feature.color ?? [0.6, 0.6, 0.7];
  const hex = rgbToHex(color);
  const setColor = (rgb: [number, number, number]) =>
    cad.setFeatureProps(feature.id, { color: rgb });

  return (
    <div className="space-y-4">
      <PropertySection title="Base Color">
        <div className="flex gap-2 items-center">
          <input
            type="color"
            value={hex}
            onChange={(e) => setColor(hexToRgb(e.target.value))}
            title="Base color"
            className="w-8 h-8 rounded border border-cad-border bg-transparent cursor-pointer"
          />
          <input
            type="text"
            value={hex}
            onChange={(e) => setColor(hexToRgb(e.target.value))}
            className="flex-1 px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm uppercase"
          />
        </div>
      </PropertySection>

      <PropertySection title="Presets">
        <div className="grid grid-cols-4 gap-2">
          {[
            ['Steel', '#8c9ab8'],
            ['Copper', '#d1873f'],
            ['Brass', '#bfae66'],
            ['Slate', '#6b7280'],
            ['Blue', '#5b9bd5'],
            ['Green', '#7fbf6b'],
            ['Rose', '#cc7484'],
            ['Teal', '#5fb3ab'],
          ].map(([name, c]) => (
            <MaterialPreset key={name} name={name} color={c} onClick={() => setColor(hexToRgb(c))} />
          ))}
        </div>
      </PropertySection>

      <p className="text-xs text-cad-text-muted">
        Color is per-part and saved with the document; Undo reverts it.
      </p>
    </div>
  );
}

function PropertySection({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <h3 className="text-xs font-semibold text-cad-text-muted uppercase mb-2">
        {title}
      </h3>
      <div className="space-y-2">{children}</div>
    </div>
  );
}

function PropertyInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange?: (value: string) => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <label className="w-20 text-sm text-cad-text-muted">{label}</label>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange?.(e.target.value)}
        className="flex-1 px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm"
      />
    </div>
  );
}

function PropertyReadOnly({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-2">
      <label className="w-20 text-sm text-cad-text-muted">{label}</label>
      <span className="text-sm text-cad-text">{value}</span>
    </div>
  );
}

function PropertySlider({
  label,
  value,
  min,
  max,
  step = 1,
  onChange,
  onCommit,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange?: (value: number) => void;
  // Fired once when the user releases the slider or blurs the input — a
  // dimension edit rebuilds the solid, so we don't want one per drag tick.
  onCommit?: (value: number) => void;
}) {
  // Remount when the committed value changes (e.g. via undo/resize) so the
  // local drag state re-seeds from the new value.
  const [localValue, setLocalValue] = React.useState(value);

  return (
    <div className="flex items-center gap-2">
      <label className="w-20 text-sm text-cad-text-muted">{label}</label>
      <Slider.Root
        className="relative flex items-center flex-1 h-5"
        value={[localValue]}
        min={min}
        max={max}
        step={step}
        onValueChange={([v]) => {
          setLocalValue(v);
          onChange?.(v);
        }}
        onValueCommit={([v]) => onCommit?.(v)}
      >
        <Slider.Track className="relative h-1 flex-1 bg-cad-bg rounded">
          <Slider.Range className="absolute h-full bg-cad-accent rounded" />
        </Slider.Track>
        <Slider.Thumb className="block w-4 h-4 bg-cad-text rounded-full focus:outline-none focus:ring-2 focus:ring-cad-accent" />
      </Slider.Root>
      <input
        type="number"
        value={localValue}
        onChange={(e) => {
          const v = parseFloat(e.target.value);
          setLocalValue(v);
          onChange?.(v);
        }}
        onBlur={(e) => onCommit?.(parseFloat(e.target.value))}
        onKeyDown={(e) => {
          if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur();
        }}
        className="w-16 px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm text-right"
      />
    </div>
  );
}

function MaterialPreset({
  name,
  color,
  onClick,
}: {
  name: string;
  color: string;
  onClick?: () => void;
}) {
  return (
    <button
      className="flex flex-col items-center p-2 rounded hover:bg-cad-bg"
      title={name}
      onClick={onClick}
    >
      <div
        className="w-8 h-8 rounded-full border border-cad-border"
        style={{ backgroundColor: color }}
      />
      <span className="text-xs mt-1 text-cad-text-muted">{name}</span>
    </button>
  );
}
