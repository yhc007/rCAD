//! # rcad-omniverse
//!
//! NVIDIA Omniverse connector for rCAD.
//! Provides USD export and Nucleus server synchronization.

pub mod connector;
pub mod live_sync;
pub mod nucleus;

pub use connector::*;
pub use live_sync::*;
pub use nucleus::*;

use thiserror::Error;

/// Result type for Omniverse operations
pub type Result<T> = std::result::Result<T, OmniverseError>;

/// Omniverse-related errors
#[derive(Debug, Error)]
pub enum OmniverseError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Nucleus error: {0}")]
    NucleusError(String),

    #[error("USD error: {0}")]
    UsdError(String),

    #[error("Sync error: {0}")]
    SyncError(String),

    #[error("Timeout")]
    Timeout,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(String),
}

/// Omniverse configuration
#[derive(Debug, Clone)]
pub struct OmniverseConfig {
    /// Nucleus server URL (e.g., "omniverse://localhost/")
    pub nucleus_url: String,

    /// Username for authentication
    pub username: String,

    /// API key or token
    pub api_key: String,

    /// Connection timeout in seconds
    pub timeout_secs: u64,

    /// Retry count for failed operations
    pub retry_count: u32,

    /// Enable live sync
    pub enable_live_sync: bool,
}

impl Default for OmniverseConfig {
    fn default() -> Self {
        Self {
            nucleus_url: "omniverse://localhost/".to_string(),
            username: String::new(),
            api_key: String::new(),
            timeout_secs: 30,
            retry_count: 3,
            enable_live_sync: true,
        }
    }
}

impl OmniverseConfig {
    /// Create a new configuration with Nucleus URL
    pub fn new(nucleus_url: impl Into<String>) -> Self {
        Self {
            nucleus_url: nucleus_url.into(),
            ..Default::default()
        }
    }

    /// Set authentication credentials
    pub fn with_auth(mut self, username: impl Into<String>, api_key: impl Into<String>) -> Self {
        self.username = username.into();
        self.api_key = api_key.into();
        self
    }
}
