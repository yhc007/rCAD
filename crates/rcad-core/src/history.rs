//! Undo/redo history system for rCAD
//!
//! Provides transaction-based undo/redo with command pattern implementation.

use crate::{EntityId, Error, FeatureId, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Maximum number of undo states to keep
const MAX_HISTORY_SIZE: usize = 100;

/// History manager for undo/redo operations
#[derive(Debug, Clone, Default)]
pub struct History {
    /// Stack of undo states
    undo_stack: VecDeque<HistoryEntry>,

    /// Stack of redo states
    redo_stack: Vec<HistoryEntry>,

    /// Whether we're currently in a transaction
    in_transaction: bool,

    /// Current transaction being built
    current_transaction: Option<Transaction>,

    /// History entry counter for IDs
    entry_counter: u64,
}

impl History {
    /// Create a new history manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a new transaction
    pub fn begin_transaction(&mut self, name: impl Into<String>) {
        if self.in_transaction {
            // Nested transactions are merged into the parent
            return;
        }

        self.in_transaction = true;
        self.current_transaction = Some(Transaction {
            name: name.into(),
            commands: Vec::new(),
        });
    }

    /// Add a command to the current transaction
    pub fn add_command(&mut self, command: Command) {
        if let Some(ref mut transaction) = self.current_transaction {
            transaction.commands.push(command);
        } else {
            // Auto-create transaction for single commands
            self.begin_transaction("Auto");
            if let Some(ref mut transaction) = self.current_transaction {
                transaction.commands.push(command);
            }
            self.commit_transaction();
        }
    }

    /// Commit the current transaction
    pub fn commit_transaction(&mut self) {
        if let Some(transaction) = self.current_transaction.take() {
            if !transaction.commands.is_empty() {
                self.entry_counter += 1;
                let entry = HistoryEntry {
                    id: self.entry_counter,
                    transaction,
                };

                self.undo_stack.push_back(entry);

                // Clear redo stack on new action
                self.redo_stack.clear();

                // Trim history if too large
                while self.undo_stack.len() > MAX_HISTORY_SIZE {
                    self.undo_stack.pop_front();
                }
            }
        }
        self.in_transaction = false;
    }

    /// Cancel the current transaction
    pub fn cancel_transaction(&mut self) {
        self.current_transaction = None;
        self.in_transaction = false;
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get the name of the next undo action
    pub fn undo_name(&self) -> Option<&str> {
        self.undo_stack.back().map(|e| e.transaction.name.as_str())
    }

    /// Get the name of the next redo action
    pub fn redo_name(&self) -> Option<&str> {
        self.redo_stack.last().map(|e| e.transaction.name.as_str())
    }

    /// Pop the next undo entry (returns commands to execute for undo)
    pub fn pop_undo(&mut self) -> Option<Transaction> {
        self.undo_stack.pop_back().map(|entry| {
            self.redo_stack.push(entry.clone());
            entry.transaction
        })
    }

    /// Pop the next redo entry (returns commands to execute for redo)
    pub fn pop_redo(&mut self) -> Option<Transaction> {
        self.redo_stack.pop().map(|entry| {
            self.undo_stack.push_back(entry.clone());
            entry.transaction
        })
    }

    /// Get the number of undo steps available
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the number of redo steps available
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.current_transaction = None;
        self.in_transaction = false;
    }

    /// Get all undo entries (for display)
    pub fn undo_entries(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.undo_stack.iter().rev()
    }

    /// Get all redo entries (for display)
    pub fn redo_entries(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.redo_stack.iter().rev()
    }
}

/// A single history entry
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Unique ID for this entry
    pub id: u64,

    /// The transaction
    pub transaction: Transaction,
}

/// A transaction containing multiple commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Human-readable name of the transaction
    pub name: String,

    /// Commands in this transaction
    pub commands: Vec<Command>,
}

impl Transaction {
    /// Create a new transaction
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            commands: Vec::new(),
        }
    }

    /// Add a command to the transaction
    pub fn add_command(&mut self, command: Command) {
        self.commands.push(command);
    }

    /// Check if the transaction is empty
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// A reversible command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// Forward action
    pub action: Action,

    /// Reverse action (for undo)
    pub reverse: Action,
}

impl Command {
    /// Create a new command with its reverse
    pub fn new(action: Action, reverse: Action) -> Self {
        Self { action, reverse }
    }

    /// Create a feature add command
    pub fn add_feature(feature_id: FeatureId, feature_data: Vec<u8>) -> Self {
        Self {
            action: Action::AddFeature {
                id: feature_id,
                data: feature_data.clone(),
            },
            reverse: Action::RemoveFeature { id: feature_id },
        }
    }

    /// Create a feature remove command
    pub fn remove_feature(feature_id: FeatureId, feature_data: Vec<u8>) -> Self {
        Self {
            action: Action::RemoveFeature { id: feature_id },
            reverse: Action::AddFeature {
                id: feature_id,
                data: feature_data,
            },
        }
    }

    /// Create a feature modify command
    pub fn modify_feature(feature_id: FeatureId, old_data: Vec<u8>, new_data: Vec<u8>) -> Self {
        Self {
            action: Action::ModifyFeature {
                id: feature_id,
                data: new_data,
            },
            reverse: Action::ModifyFeature {
                id: feature_id,
                data: old_data,
            },
        }
    }

    /// Create a parameter change command
    pub fn set_parameter(name: String, old_value: Option<Vec<u8>>, new_value: Vec<u8>) -> Self {
        Self {
            action: Action::SetParameter {
                name: name.clone(),
                value: new_value,
            },
            reverse: match old_value {
                Some(data) => Action::SetParameter { name, value: data },
                None => Action::RemoveParameter { name },
            },
        }
    }
}

/// An action that can be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// Add a feature
    AddFeature {
        id: FeatureId,
        /// Serialized feature data
        data: Vec<u8>,
    },

    /// Remove a feature
    RemoveFeature { id: FeatureId },

    /// Modify a feature
    ModifyFeature {
        id: FeatureId,
        /// New serialized feature data
        data: Vec<u8>,
    },

    /// Move a feature in the tree
    MoveFeature {
        id: FeatureId,
        new_parent: Option<FeatureId>,
        new_index: usize,
    },

    /// Suppress/unsuppress a feature
    SetSuppressed { id: FeatureId, suppressed: bool },

    /// Set a document parameter
    SetParameter { name: String, value: Vec<u8> },

    /// Remove a document parameter
    RemoveParameter { name: String },

    /// Rename a feature
    RenameFeature { id: FeatureId, name: String },

    /// Set document metadata
    SetMetadata { key: String, value: String },

    /// Batch action (for complex operations)
    Batch { actions: Vec<Action> },
}

/// Memento for storing complete document state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMemento {
    /// Serialized document state
    pub data: Vec<u8>,

    /// Timestamp of creation
    pub timestamp: u64,

    /// Description of the state
    pub description: String,
}

impl DocumentMemento {
    /// Create a new memento
    pub fn new(data: Vec<u8>, description: impl Into<String>) -> Self {
        Self {
            data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            description: description.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_basic() {
        let mut history = History::new();

        assert!(!history.can_undo());
        assert!(!history.can_redo());

        // Add a transaction
        history.begin_transaction("Test Action");
        history.add_command(Command::new(
            Action::SetParameter {
                name: "test".to_string(),
                value: vec![1, 2, 3],
            },
            Action::RemoveParameter {
                name: "test".to_string(),
            },
        ));
        history.commit_transaction();

        assert!(history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.undo_name(), Some("Test Action"));
    }

    #[test]
    fn test_undo_redo() {
        let mut history = History::new();

        // Add first transaction
        history.begin_transaction("Action 1");
        history.add_command(Command::add_feature(FeatureId::new(), vec![1]));
        history.commit_transaction();

        // Add second transaction
        history.begin_transaction("Action 2");
        history.add_command(Command::add_feature(FeatureId::new(), vec![2]));
        history.commit_transaction();

        assert_eq!(history.undo_count(), 2);
        assert_eq!(history.redo_count(), 0);

        // Undo
        let undone = history.pop_undo();
        assert!(undone.is_some());
        assert_eq!(undone.unwrap().name, "Action 2");
        assert_eq!(history.undo_count(), 1);
        assert_eq!(history.redo_count(), 1);

        // Redo
        let redone = history.pop_redo();
        assert!(redone.is_some());
        assert_eq!(redone.unwrap().name, "Action 2");
        assert_eq!(history.undo_count(), 2);
        assert_eq!(history.redo_count(), 0);
    }

    #[test]
    fn test_clear_redo_on_new_action() {
        let mut history = History::new();

        history.begin_transaction("Action 1");
        history.add_command(Command::add_feature(FeatureId::new(), vec![1]));
        history.commit_transaction();

        history.pop_undo(); // Now we have 1 redo

        // New action should clear redo
        history.begin_transaction("Action 2");
        history.add_command(Command::add_feature(FeatureId::new(), vec![2]));
        history.commit_transaction();

        assert_eq!(history.redo_count(), 0);
    }
}
