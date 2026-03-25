import React, { useEffect } from 'react';
import { Toolbar } from './components/Toolbar';
import { PropertyPanel } from './components/PropertyPanel';
import { ModelTree } from './components/ModelTree';
import { Canvas } from './components/Canvas';
import { CommandPalette } from './components/CommandPalette';
import { useDocumentStore } from './stores/documentStore';
import { useCAD } from './hooks/useCAD';

function App() {
  const { initialize, isInitialized, error } = useCAD();
  const { setDocument } = useDocumentStore();

  useEffect(() => {
    initialize().then(() => {
      console.log('rCAD initialized');
    });
  }, [initialize]);

  if (error) {
    return (
      <div className="flex items-center justify-center h-screen bg-cad-bg text-cad-text">
        <div className="text-center">
          <h1 className="text-2xl font-bold mb-4">Initialization Error</h1>
          <p className="text-cad-text-muted">{error}</p>
        </div>
      </div>
    );
  }

  if (!isInitialized) {
    return (
      <div className="flex items-center justify-center h-screen bg-cad-bg text-cad-text">
        <div className="text-center">
          <h1 className="text-2xl font-bold mb-4">Loading rCAD...</h1>
          <div className="animate-spin w-8 h-8 border-4 border-cad-accent border-t-transparent rounded-full mx-auto"></div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen bg-cad-bg">
      {/* Top Toolbar */}
      <Toolbar />

      {/* Main Content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left Panel - Model Tree */}
        <div className="w-64 border-r border-cad-border bg-cad-panel">
          <ModelTree />
        </div>

        {/* Center - 3D Canvas */}
        <div className="flex-1 relative">
          <Canvas />
        </div>

        {/* Right Panel - Properties */}
        <div className="w-72 border-l border-cad-border bg-cad-panel">
          <PropertyPanel />
        </div>
      </div>

      {/* Command Palette (modal) */}
      <CommandPalette />
    </div>
  );
}

export default App;
