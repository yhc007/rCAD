import React from 'react';
import { useDocumentStore } from '../stores/documentStore';
import * as Slider from '@radix-ui/react-slider';
import * as Tabs from '@radix-ui/react-tabs';

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
            <MaterialProperties />
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
}

function FeatureProperties({ feature }: { feature: Feature }) {
  return (
    <div className="space-y-4">
      <PropertySection title="General">
        <PropertyInput label="Name" value={feature.name} />
        <PropertyReadOnly label="Type" value={feature.type} />
        <PropertyReadOnly label="ID" value={feature.id.substring(0, 8)} />
      </PropertySection>

      {feature.type === 'Box' && (
        <PropertySection title="Dimensions">
          <PropertySlider label="Width" value={50} min={1} max={200} />
          <PropertySlider label="Height" value={50} min={1} max={200} />
          <PropertySlider label="Depth" value={50} min={1} max={200} />
        </PropertySection>
      )}

      {feature.type === 'Cylinder' && (
        <PropertySection title="Dimensions">
          <PropertySlider label="Radius" value={25} min={1} max={100} />
          <PropertySlider label="Height" value={50} min={1} max={200} />
        </PropertySection>
      )}

      {feature.type === 'Sphere' && (
        <PropertySection title="Dimensions">
          <PropertySlider label="Radius" value={25} min={1} max={100} />
        </PropertySection>
      )}

      {feature.type === 'Cone' && (
        <PropertySection title="Dimensions">
          <PropertySlider label="Bottom Radius" value={25} min={0} max={100} />
          <PropertySlider label="Top Radius" value={0} min={0} max={100} />
          <PropertySlider label="Height" value={50} min={1} max={200} />
        </PropertySection>
      )}

      <PropertySection title="Transform">
        <PropertyVector3 label="Position" x={0} y={0} z={0} />
        <PropertyVector3 label="Rotation" x={0} y={0} z={0} />
      </PropertySection>
    </div>
  );
}

function MaterialProperties() {
  return (
    <div className="space-y-4">
      <PropertySection title="Base Color">
        <div className="flex gap-2 items-center">
          <div
            className="w-8 h-8 rounded border border-cad-border"
            style={{ backgroundColor: '#cccccc' }}
          />
          <input
            type="text"
            value="#CCCCCC"
            className="flex-1 px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm"
          />
        </div>
      </PropertySection>

      <PropertySection title="PBR Properties">
        <PropertySlider label="Metallic" value={0} min={0} max={1} step={0.01} />
        <PropertySlider label="Roughness" value={0.5} min={0} max={1} step={0.01} />
        <PropertySlider label="Emissive" value={0} min={0} max={1} step={0.01} />
      </PropertySection>

      <PropertySection title="Presets">
        <div className="grid grid-cols-4 gap-2">
          <MaterialPreset name="Plastic" color="#888888" />
          <MaterialPreset name="Metal" color="#a0a0a0" />
          <MaterialPreset name="Gold" color="#ffd700" />
          <MaterialPreset name="Glass" color="#ffffff" />
        </div>
      </PropertySection>
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
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange?: (value: number) => void;
}) {
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
        className="w-16 px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm text-right"
      />
    </div>
  );
}

function PropertyVector3({
  label,
  x,
  y,
  z,
}: {
  label: string;
  x: number;
  y: number;
  z: number;
}) {
  return (
    <div className="flex items-center gap-2">
      <label className="w-20 text-sm text-cad-text-muted">{label}</label>
      <div className="flex gap-1 flex-1">
        <input
          type="number"
          value={x}
          className="w-full px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm"
          placeholder="X"
        />
        <input
          type="number"
          value={y}
          className="w-full px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm"
          placeholder="Y"
        />
        <input
          type="number"
          value={z}
          className="w-full px-2 py-1 bg-cad-bg border border-cad-border rounded text-sm"
          placeholder="Z"
        />
      </div>
    </div>
  );
}

function MaterialPreset({ name, color }: { name: string; color: string }) {
  return (
    <button
      className="flex flex-col items-center p-2 rounded hover:bg-cad-bg"
      title={name}
    >
      <div
        className="w-8 h-8 rounded-full border border-cad-border"
        style={{ backgroundColor: color }}
      />
      <span className="text-xs mt-1 text-cad-text-muted">{name}</span>
    </button>
  );
}
