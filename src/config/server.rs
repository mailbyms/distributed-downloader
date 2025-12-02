//! Server configuration

use serde::{Deserialize, Serialize};
use std::fs;
use crate::error::DistributedDownloaderError;

/// Server configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server ID
    pub id: u16,
    /// Manager address IPv4
    pub manager_addr_ipv4: String,

    /// Manager port
    pub manager_port: u16,

    /// Temporary directory for file segments
    pub tmp_dir: String,

    /// Number of threads for downloading
    pub threads_num: usize,

    /// Target directory for downloaded files
    pub target_dir: String,
}

impl ServerConfig {
    /// Load configuration from YAML file
    pub fn from_file(path: &str) -> Result<Self, DistributedDownloaderError> {
        let content = fs::read_to_string(path)?;
        let config: ServerConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to YAML file
    pub fn to_file(&self, path: &str) -> Result<(), DistributedDownloaderError> {
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}