//! 管理器配置

use serde::{Deserialize, Serialize};
use std::fs;
use crate::error::DistributedDownloaderError;

/// 管理器配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerConfig {
    /// 管理器IPv4地址
    pub manager_addr_ipv4: String,

    /// 管理器监听端口
    pub manager_port: u16,
}

impl ManagerConfig {
    /// 从YAML文件加载配置
    pub fn from_file(path: &str) -> Result<Self, DistributedDownloaderError> {
        let content = fs::read_to_string(path)?;
        let config: ManagerConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// 保存配置到YAML文件
    pub fn to_file(&self, path: &str) -> Result<(), DistributedDownloaderError> {
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}