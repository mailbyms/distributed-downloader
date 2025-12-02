//! Unit tests for network modules

#[cfg(test)]
mod tests {
    use distributed_downloader::network::manager::ServerInfo;

    #[test]
    fn test_server_info_serialization() {
        let server_info = ServerInfo {
            addr: "127.0.0.1".to_string(),
            port: 8000,
        };

        // Test serialization
        let json_str = serde_json::to_string(&server_info).unwrap();
        assert!(json_str.contains("127.0.0.1"));
        assert!(json_str.contains("8000"));

        // Test deserialization
        let deserialized: ServerInfo = serde_json::from_str(&json_str).unwrap();
        assert_eq!(server_info.addr, deserialized.addr);
        assert_eq!(server_info.port, deserialized.port);
    }
}