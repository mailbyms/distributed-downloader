//! 下载器模块的单元测试

#[cfg(test)]
mod tests {
    use distributed_downloader::downloader::http::HttpDownloader;

    #[test]
    fn test_http_downloader_creation() {
        let downloader = HttpDownloader::new(None);
        assert!(true); // 如果我们到达这里，说明创建成功
    }

    #[test]
    fn test_http_downloader_clone() {
        let downloader = HttpDownloader::new(None);
        let cloned_downloader = downloader.clone();
        assert!(true); // 如果我们到达这里，说明克隆成功
    }
}