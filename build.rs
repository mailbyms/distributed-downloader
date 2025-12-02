//! Build script for distributed downloader

use std::fs;
use std::path::Path;

fn main() {
    // Create config directories if they don't exist
    let config_dir = Path::new("configs");
    if !config_dir.exists() {
        fs::create_dir_all(config_dir).expect("Failed to create configs directory");
    }

    // Create example config files if they don't exist
    create_example_config_if_not_exists(
        "configs/manager.yml",
        include_str!("src/config/examples/manager.yml"),
    );

    create_example_config_if_not_exists(
        "configs/server.yml",
        include_str!("src/config/examples/server.yml"),
    );

    create_example_config_if_not_exists(
        "configs/client.yml",
        include_str!("src/config/examples/client.yml"),
    );

    // Set environment variables for the build
    println!("cargo:rustc-env=PROJECT_NAME=distributed-downloader");
    println!("cargo:rustc-env=BUILD_TIME={}", chrono::Utc::now().to_rfc3339());
}

fn create_example_config_if_not_exists(path: &str, content: &str) {
    let config_path = Path::new(path);
    if !config_path.exists() {
        fs::write(config_path, content).expect(&format!("Failed to create {}", path));
    }
}