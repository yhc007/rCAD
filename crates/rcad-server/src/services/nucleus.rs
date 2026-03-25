//! Nucleus service
//!
//! Manages Omniverse Nucleus connections for the server.

use rcad_omniverse::{OmniverseConfig, OmniverseConnector, OmniverseError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Nucleus service managing multiple connections
pub struct NucleusService {
    /// Active connections by session ID
    connections: Arc<RwLock<HashMap<String, OmniverseConnector>>>,

    /// Default configuration
    default_config: OmniverseConfig,
}

impl NucleusService {
    /// Create a new Nucleus service
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            default_config: OmniverseConfig::default(),
        }
    }

    /// Create a new connection
    pub async fn connect(&self, config: OmniverseConfig) -> Result<String, OmniverseError> {
        let mut connector = OmniverseConnector::new(config);
        connector.connect().await?;

        let session_id = connector
            .session_id()
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let mut connections = self.connections.write().await;
        connections.insert(session_id.clone(), connector);

        tracing::info!("Created Nucleus connection: {}", session_id);
        Ok(session_id)
    }

    /// Disconnect a session
    pub async fn disconnect(&self, session_id: &str) -> Result<(), OmniverseError> {
        let mut connections = self.connections.write().await;

        if let Some(mut connector) = connections.remove(session_id) {
            connector.disconnect().await?;
            tracing::info!("Disconnected Nucleus session: {}", session_id);
        }

        Ok(())
    }

    /// Get a connector by session ID
    pub async fn get_connector(&self, session_id: &str) -> Option<OmniverseConnector> {
        let connections = self.connections.read().await;
        // Note: In real implementation, would need to handle this differently
        // as OmniverseConnector isn't Clone
        None
    }

    /// Check if a session exists
    pub async fn session_exists(&self, session_id: &str) -> bool {
        let connections = self.connections.read().await;
        connections.contains_key(session_id)
    }

    /// Get active session count
    pub async fn session_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }

    /// Upload content to Nucleus
    pub async fn upload(
        &self,
        session_id: &str,
        content: &[u8],
        nucleus_path: &str,
    ) -> Result<String, OmniverseError> {
        let connections = self.connections.read().await;

        if let Some(connector) = connections.get(session_id) {
            connector.upload_usd_content(content, nucleus_path).await
        } else {
            Err(OmniverseError::ConnectionFailed(
                "Session not found".to_string(),
            ))
        }
    }

    /// Disconnect all sessions
    pub async fn disconnect_all(&self) -> Result<(), OmniverseError> {
        let mut connections = self.connections.write().await;

        for (session_id, mut connector) in connections.drain() {
            if let Err(e) = connector.disconnect().await {
                tracing::warn!("Error disconnecting session {}: {:?}", session_id, e);
            }
        }

        Ok(())
    }
}

impl Default for NucleusService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_creation() {
        let service = NucleusService::new();
        assert_eq!(service.session_count().await, 0);
    }
}
