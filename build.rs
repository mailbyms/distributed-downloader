//! Build script for distributed downloader

use std::fs;
use std::path::Path;

fn main() {
    // Create src/proto directory if it doesn't exist
    let proto_dir = Path::new("src/proto");
    if !proto_dir.exists() {
        fs::create_dir_all(proto_dir).expect("Failed to create src/proto directory");
    }

    // Configure prost-build to generate Rust code in src/proto directory
    let mut config = prost_build::Config::new();
    config.out_dir("src/proto");

    // Add derives for ServerInfo to make it work with HashMap
    config.type_attribute("distributed_downloader.ServerInfo", "#[derive(Eq, Hash)]");

    // Compile protobuf files
    match config.compile_protos(&["protobuf/messages.proto"], &["protobuf/"]) {
        Ok(_) => {
            println!("Successfully compiled protobuf files");
            // Create mod.rs file to export generated modules
            create_proto_mod_file();
        },
        Err(e) => {
            println!("Warning: Failed to compile protobuf files: {}", e);
            println!("Continuing without protobuf support");
        }
    }

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

fn create_proto_mod_file() {
    let mod_content = r#"//! Auto-generated protobuf modules

pub mod distributed_downloader;
"#;

    // Create src/proto directory if it doesn't exist
    let proto_dir = Path::new("src/proto");
    if !proto_dir.exists() {
        fs::create_dir_all(proto_dir).expect("Failed to create src/proto directory");
    }

    // Create mod.rs file
    let mod_path = proto_dir.join("mod.rs");
    fs::write(mod_path, mod_content).expect("Failed to create src/proto/mod.rs");
}