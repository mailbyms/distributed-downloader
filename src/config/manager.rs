//! Manager configuration

use serde::{Deserialize, Serialize};
use std::fs;
use crate::error::DistributedDownloaderError;

/// Manager configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerConfig {
    /// Manager IPv4 address
    pub manager_addr_ipv4: String,

    /// Manager listening port
    pub manager_port: u16,
}

impl ManagerConfig {
    /// Load configuration from YAML file
    pub fn from_file(path: &str) -> Result<Self, DistributedDownloaderError> {
        let content = fs::read_to_string(path)?;
        let config: ManagerConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to YAML file
    pub fn to_file(&self, path: &str) -> Result<(), DistributedDownloaderError> {
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}