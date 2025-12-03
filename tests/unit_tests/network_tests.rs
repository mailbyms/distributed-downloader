//! 网络模块的单元测试

#[cfg(test)]
mod tests {
    use distributed_downloader::network::manager::ServerInfo;

    #[test]
    fn test_server_info_serialization() {
        let server_info = ServerInfo {
            addr: "127.0.0.1".to_string(),
            port: 8000,
        };

        // 测试序列化
        let json_str = serde_json::to_string(&server_info).unwrap();
        assert!(json_str.contains("127.0.0.1"));
        assert!(json_str.contains("8000"));

        // 测试反序列化
        let deserialized: ServerInfo = serde_json::from_str(&json_str).unwrap();
        assert_eq!(server_info.addr, deserialized.addr);
        assert_eq!(server_info.port, deserialized.port);
    }
}