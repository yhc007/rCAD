import React, { useState, useEffect, useCallback } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { Search, Box, Cylinder, Circle, Cone, Plus, Minus, Save, FolderOpen } from 'lucide-react';

interface Command {
  id: string;
  label: string;
  shortcut?: string;
  icon: React.ReactNode;
  action: () => void;
  category: string;
}

export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');

  // Listen for keyboard shortcut
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setOpen(true);
      }
      if (e.key === 'Escape') {
        setOpen(false);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const commands: Command[] = [
    // File commands
    {
      id: 'file-new',
      label: 'New Document',
      shortcut: '⌘N',
      icon: <FolderOpen size={16} />,
      action: () => console.log('New document'),
      category: 'File',
    },
    {
      id: 'file-open',
      label: 'Open Document',
      shortcut: '⌘O',
      icon: <FolderOpen size={16} />,
      action: () => console.log('Open document'),
      category: 'File',
    },
    {
      id: 'file-save',
      label: 'Save Document',
      shortcut: '⌘S',
      icon: <Save size={16} />,
      action: () => console.log('Save document'),
      category: 'File',
    },

    // Create commands
    {
      id: 'create-box',
      label: 'Create Box',
      icon: <Box size={16} />,
      action: () => console.log('Create box'),
      category: 'Create',
    },
    {
      id: 'create-cylinder',
      label: 'Create Cylinder',
      icon: <Cylinder size={16} />,
      action: () => console.log('Create cylinder'),
      category: 'Create',
    },
    {
      id: 'create-sphere',
      label: 'Create Sphere',
      icon: <Circle size={16} />,
      action: () => console.log('Create sphere'),
      category: 'Create',
    },
    {
      id: 'create-cone',
      label: 'Create Cone',
      icon: <Cone size={16} />,
      action: () => console.log('Create cone'),
      category: 'Create',
    },

    // Boolean commands
    {
      id: 'boolean-union',
      label: 'Boolean Union',
      icon: <Plus size={16} />,
      action: () => console.log('Boolean union'),
      category: 'Boolean',
    },
    {
      id: 'boolean-subtract',
      label: 'Boolean Subtract',
      icon: <Minus size={16} />,
      action: () => console.log('Boolean subtract'),
      category: 'Boolean',
    },
  ];

  const filteredCommands = commands.filter((cmd) =>
    cmd.label.toLowerCase().includes(query.toLowerCase())
  );

  const groupedCommands = filteredCommands.reduce((acc, cmd) => {
    if (!acc[cmd.category]) {
      acc[cmd.category] = [];
    }
    acc[cmd.category].push(cmd);
    return acc;
  }, {} as Record<string, Command[]>);

  const handleSelect = (command: Command) => {
    command.action();
    setOpen(false);
    setQuery('');
  };

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50" />
        <Dialog.Content className="fixed top-1/4 left-1/2 -translate-x-1/2 w-full max-w-lg bg-cad-panel border border-cad-border rounded-lg shadow-xl">
          {/* Search input */}
          <div className="flex items-center gap-3 px-4 py-3 border-b border-cad-border">
            <Search size={20} className="text-cad-text-muted" />
            <input
              type="text"
              placeholder="Search commands..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="flex-1 bg-transparent text-cad-text outline-none placeholder:text-cad-text-muted"
              autoFocus
            />
            <kbd className="px-2 py-1 text-xs bg-cad-bg rounded border border-cad-border">
              ESC
            </kbd>
          </div>

          {/* Command list */}
          <div className="max-h-80 overflow-auto py-2">
            {Object.entries(groupedCommands).map(([category, cmds]) => (
              <div key={category}>
                <div className="px-4 py-1 text-xs font-semibold text-cad-text-muted uppercase">
                  {category}
                </div>
                {cmds.map((cmd) => (
                  <button
                    key={cmd.id}
                    className="w-full flex items-center gap-3 px-4 py-2 hover:bg-cad-bg text-left"
                    onClick={() => handleSelect(cmd)}
                  >
                    <span className="text-cad-text-muted">{cmd.icon}</span>
                    <span className="flex-1 text-cad-text">{cmd.label}</span>
                    {cmd.shortcut && (
                      <kbd className="px-2 py-0.5 text-xs bg-cad-bg rounded border border-cad-border text-cad-text-muted">
                        {cmd.shortcut}
                      </kbd>
                    )}
                  </button>
                ))}
              </div>
            ))}

            {filteredCommands.length === 0 && (
              <div className="px-4 py-8 text-center text-cad-text-muted">
                No commands found
              </div>
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
