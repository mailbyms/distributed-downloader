//! Unit tests for configuration modules

#[cfg(test)]
mod tests {
    use distributed_downloader::config::{ManagerConfig, ServerConfig, ClientConfig};
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_manager_config_serialization() {
        let config = ManagerConfig {
            manager_addr_ipv4: "127.0.0.1".to_string(),
            manager_port: 5000,
        };

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("manager.yml");

        // Test saving config
        assert!(config.to_file(config_path.to_str().unwrap()).is_ok());

        // Test loading config
        let loaded_config = ManagerConfig::from_file(config_path.to_str().unwrap()).unwrap();
        assert_eq!(config.manager_addr_ipv4, loaded_config.manager_addr_ipv4);
        assert_eq!(config.manager_port, loaded_config.manager_port);
    }

    #[test]
    fn test_server_config_serialization() {

        let config = ServerConfig {
            manager_addr_ipv4: "127.0.0.1".to_string(),
            manager_port: 5000,
            tmp_dir: "./tmp".to_string(),
            threads_num: 4,
            target_dir: "./target".to_string(),
        };

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("server.yml");

        // Test saving config
        assert!(config.to_file(config_path.to_str().unwrap()).is_ok());

        // Test loading config
        let loaded_config = ServerConfig::from_file(config_path.to_str().unwrap()).unwrap();
        assert_eq!(config.to_client_port, loaded_config.to_client_port);
        assert_eq!(config.threads_num, loaded_config.threads_num);
    }

    #[test]
    fn test_client_config_serialization() {

        let config = ClientConfig {
            client_addr_ipv4: "127.0.0.1".to_string(),
            to_server_port: 9000,
            to_manager_port: 6000,
            manager_addr_ipv4: "127.0.0.1".to_string(),
            manager_port: 5000,
            tmp_dir: "./tmp".to_string(),
            target_dir: "./target".to_string(),
        };

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("client.yml");

        // Test saving config
        assert!(config.to_file(config_path.to_str().unwrap()).is_ok());

        // Test loading config
        let loaded_config = ClientConfig::from_file(config_path.to_str().unwrap()).unwrap();
        assert_eq!(config.client_addr_ipv4, loaded_config.client_addr_ipv4);
        assert_eq!(config.to_server_port, loaded_config.to_server_port);
        assert_eq!(config.tmp_dir, loaded_config.tmp_dir);
    }
}