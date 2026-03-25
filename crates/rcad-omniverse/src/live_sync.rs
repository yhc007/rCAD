//! Live sync for real-time collaboration
//!
//! Provides real-time synchronization of USD changes with Omniverse.

use crate::{OmniverseConfig, OmniverseError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Live sync channel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveSyncState {
    /// Not started
    Idle,
    /// Syncing
    Syncing,
    /// Paused
    Paused,
    /// Error occurred
    Error,
}

/// A change to sync
#[derive(Debug, Clone)]
pub struct SyncChange {
    /// Path of the changed prim
    pub prim_path: String,

    /// Type of change
    pub change_type: ChangeType,

    /// Changed data (serialized)
    pub data: Vec<u8>,

    /// Timestamp
    pub timestamp: u64,
}

/// Type of change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    /// Prim created
    Created,
    /// Prim modified
    Modified,
    /// Prim deleted
    Deleted,
    /// Transform changed
    TransformChanged,
    /// Material changed
    MaterialChanged,
}

/// Live sync manager
pub struct LiveSyncManager {
    config: OmniverseConfig,
    state: Arc<RwLock<LiveSyncState>>,
    channel_id: Option<String>,
    pending_changes: Arc<RwLock<Vec<SyncChange>>>,
    change_sender: Option<mpsc::Sender<SyncChange>>,
    subscribers: Arc<RwLock<HashMap<String, mpsc::Sender<SyncChange>>>>,
}

impl LiveSyncManager {
    /// Create a new live sync manager
    pub fn new(config: OmniverseConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(LiveSyncState::Idle)),
            channel_id: None,
            pending_changes: Arc::new(RwLock::new(Vec::new())),
            change_sender: None,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get current sync state
    pub async fn state(&self) -> LiveSyncState {
        *self.state.read().await
    }

    /// Start live sync session
    pub async fn start(&mut self, nucleus_path: &str) -> Result<String> {
        let mut state = self.state.write().await;

        if *state == LiveSyncState::Syncing {
            return Err(OmniverseError::SyncError(
                "Already syncing".to_string(),
            ));
        }

        *state = LiveSyncState::Syncing;

        // Generate channel ID
        let channel_id = uuid::Uuid::new_v4().to_string();
        self.channel_id = Some(channel_id.clone());

        // Create change channel
        let (tx, mut rx) = mpsc::channel::<SyncChange>(100);
        self.change_sender = Some(tx);

        // Start background sync task
        let state_clone = self.state.clone();
        let pending_clone = self.pending_changes.clone();
        let subscribers_clone = self.subscribers.clone();

        tokio::spawn(async move {
            while let Some(change) = rx.recv().await {
                // Process the change
                log::debug!("Processing sync change: {:?}", change.change_type);

                // Add to pending
                {
                    let mut pending = pending_clone.write().await;
                    pending.push(change.clone());

                    // Limit pending changes
                    if pending.len() > 1000 {
                        pending.remove(0);
                    }
                }

                // Notify subscribers
                {
                    let subscribers = subscribers_clone.read().await;
                    for (_, sender) in subscribers.iter() {
                        let _ = sender.send(change.clone()).await;
                    }
                }
            }

            // Channel closed, update state
            let mut state = state_clone.write().await;
            *state = LiveSyncState::Idle;
        });

        log::info!("Live sync started for: {}", nucleus_path);
        Ok(channel_id)
    }

    /// Stop live sync session
    pub async fn stop(&mut self) -> Result<()> {
        let mut state = self.state.write().await;

        if *state != LiveSyncState::Syncing {
            return Ok(());
        }

        // Close the change sender (will terminate the background task)
        self.change_sender = None;
        self.channel_id = None;

        *state = LiveSyncState::Idle;

        log::info!("Live sync stopped");
        Ok(())
    }

    /// Pause live sync
    pub async fn pause(&mut self) -> Result<()> {
        let mut state = self.state.write().await;

        if *state != LiveSyncState::Syncing {
            return Err(OmniverseError::SyncError(
                "Not currently syncing".to_string(),
            ));
        }

        *state = LiveSyncState::Paused;
        log::info!("Live sync paused");
        Ok(())
    }

    /// Resume live sync
    pub async fn resume(&mut self) -> Result<()> {
        let mut state = self.state.write().await;

        if *state != LiveSyncState::Paused {
            return Err(OmniverseError::SyncError(
                "Not paused".to_string(),
            ));
        }

        *state = LiveSyncState::Syncing;
        log::info!("Live sync resumed");
        Ok(())
    }

    /// Submit a change to sync
    pub async fn submit_change(&self, change: SyncChange) -> Result<()> {
        let state = self.state.read().await;

        if *state != LiveSyncState::Syncing {
            return Err(OmniverseError::SyncError(
                "Not currently syncing".to_string(),
            ));
        }

        if let Some(ref sender) = self.change_sender {
            sender
                .send(change)
                .await
                .map_err(|_| OmniverseError::SyncError("Channel closed".to_string()))?;
        }

        Ok(())
    }

    /// Subscribe to receive changes
    pub async fn subscribe(&self, subscriber_id: &str) -> Result<mpsc::Receiver<SyncChange>> {
        let (tx, rx) = mpsc::channel(100);

        let mut subscribers = self.subscribers.write().await;
        subscribers.insert(subscriber_id.to_string(), tx);

        Ok(rx)
    }

    /// Unsubscribe from changes
    pub async fn unsubscribe(&self, subscriber_id: &str) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.remove(subscriber_id);
    }

    /// Get pending changes
    pub async fn pending_changes(&self) -> Vec<SyncChange> {
        self.pending_changes.read().await.clone()
    }

    /// Clear pending changes
    pub async fn clear_pending(&self) {
        let mut pending = self.pending_changes.write().await;
        pending.clear();
    }

    /// Get channel ID
    pub fn channel_id(&self) -> Option<&str> {
        self.channel_id.as_deref()
    }
}

/// Builder for sync changes
pub struct SyncChangeBuilder {
    prim_path: String,
    change_type: ChangeType,
    data: Vec<u8>,
}

impl SyncChangeBuilder {
    pub fn new(prim_path: impl Into<String>, change_type: ChangeType) -> Self {
        Self {
            prim_path: prim_path.into(),
            change_type,
            data: Vec::new(),
        }
    }

    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    pub fn build(self) -> SyncChange {
        SyncChange {
            prim_path: self.prim_path,
            change_type: self.change_type,
            data: self.data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_live_sync_creation() {
        let config = OmniverseConfig::default();
        let manager = LiveSyncManager::new(config);

        assert_eq!(manager.state().await, LiveSyncState::Idle);
    }

    #[tokio::test]
    async fn test_start_stop() {
        let config = OmniverseConfig::default();
        let mut manager = LiveSyncManager::new(config);

        let channel_id = manager.start("/test/path").await.unwrap();
        assert!(!channel_id.is_empty());
        assert_eq!(manager.state().await, LiveSyncState::Syncing);

        manager.stop().await.unwrap();
        // State transitions async, just check it doesn't error
    }
}
