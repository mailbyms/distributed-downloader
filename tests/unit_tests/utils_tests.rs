//! Unit tests for utility modules

#[cfg(test)]
mod tests {
    use distributed_downloader::utils::distributor::DownloadDistributor;
    use distributed_downloader::utils::{create_dir, remove_dir, append_files, delete_file};
    use tempfile::TempDir;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_download_distributor() {
        // Test normal distribution
        let intervals = DownloadDistributor::download_interval_list(0, 99, 4);
        assert_eq!(intervals.len(), 4);
        assert_eq!(intervals[0], [0, 24]);
        assert_eq!(intervals[1], [25, 49]);
        assert_eq!(intervals[2], [50, 74]);
        assert_eq!(intervals[3], [75, 99]);

        // Test distribution with remainder
        let intervals = DownloadDistributor::download_interval_list(0, 100, 3);
        assert_eq!(intervals.len(), 3);
        assert_eq!(intervals[0], [0, 33]);
        assert_eq!(intervals[1], [34, 67]);
        assert_eq!(intervals[2], [68, 100]);
    }

    #[test]
    #[should_panic(expected = "Number of parts")]
    fn test_download_distributor_panic() {
        // This should panic because number of parts > file size
        DownloadDistributor::download_interval_list(0, 5, 10);
    }

    #[test]
    fn test_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test_dir");
        let test_dir_str = test_dir.to_str().unwrap();

        // Test create_dir
        assert!(create_dir(test_dir_str).is_ok());
        assert!(test_dir.exists());

        // Test remove_dir
        assert!(remove_dir(test_dir_str).is_ok());
        assert!(!test_dir.exists());

        // Test append_files
        let file1_path = temp_dir.path().join("file1.txt");
        let file2_path = temp_dir.path().join("file2.txt");
        let target_path = temp_dir.path().join("target.txt");

        // Create test files
        fs::write(&file1_path, "Hello ").unwrap();
        fs::write(&file2_path, "World!").unwrap();

        // Test append_files
        assert!(append_files(
            &[file1_path.to_str().unwrap().to_string(), file2_path.to_str().unwrap().to_string()],
            target_path.to_str().unwrap()
        ).is_ok());

        // Check result
        let content = fs::read_to_string(&target_path).unwrap();
        assert_eq!(content, "Hello World!");

        // Test delete_file
        assert!(delete_file(target_path.to_str().unwrap()).is_ok());
        assert!(!target_path.exists());
    }
}