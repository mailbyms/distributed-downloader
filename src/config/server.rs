//! 服务器配置

use serde::{Deserialize, Serialize};
use std::fs;
use crate::error::DistributedDownloaderError;

/// 服务器配置结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 服务器ID
    pub id: u16,
    /// 管理器地址IPv4
    pub manager_addr_ipv4: String,

    /// 管理器端口
    pub manager_port: u16,

    /// 文件段的临时目录
    pub tmp_dir: String,

    /// 下载线程数
    pub threads_num: usize,

    /// 下载文件的目标目录
    pub target_dir: String,
}

impl ServerConfig {
    /// 从YAML文件加载配置
    pub fn from_file(path: &str) -> Result<Self, DistributedDownloaderError> {
        let content = fs::read_to_string(path)?;
        let config: ServerConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// 保存配置到YAML文件
    pub fn to_file(&self, path: &str) -> Result<(), DistributedDownloaderError> {
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}