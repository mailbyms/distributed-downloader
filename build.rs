//! 分布式下载器的构建脚本

use std::fs;
use std::path::Path;

fn main() {
    // 如果不存在则创建 src/proto 目录
    let proto_dir = Path::new("src/proto");
    if !proto_dir.exists() {
        fs::create_dir_all(proto_dir).expect("创建 src/proto 目录失败");
    }

    // 使用 tonic_build 编译 protobuf 文件，它会同时生成 gRPC 服务和消息结构体
    match tonic_build::configure()
        .out_dir("src/proto") // 设置输出目录
        .build_server(true)   // 生成 gRPC 服务端代码
        .build_client(true)   // 生成 gRPC 客户端代码
        .compile(&["protobuf/messages.proto"], &["protobuf/"])
    {
        Ok(_) => {
            println!("成功编译 protobuf 文件");
            // 创建 mod.rs 文件以导出生成的模块
            create_proto_mod_file();
        },
        Err(e) => {
            println!("警告：编译 protobuf 文件失败: {}", e);
            println!("在没有 protobuf 支持的情况下继续");
        }
    }

    // 如果不存在则创建配置目录
    let config_dir = Path::new("configs");
    if !config_dir.exists() {
        fs::create_dir_all(config_dir).expect("创建 configs 目录失败");
    }

    // 如果不存在则创建示例配置文件
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

    // 为构建设置环境变量
    println!("cargo:rustc-env=PROJECT_NAME=distributed-downloader");
    println!("cargo:rustc-env=BUILD_TIME={}", chrono::Utc::now().to_rfc3339());
}

fn create_example_config_if_not_exists(path: &str, content: &str) {
    let config_path = Path::new(path);
    if !config_path.exists() {
        fs::write(config_path, content).expect(&format!("创建 {} 失败", path));
    }
}

fn create_proto_mod_file() {
    let mod_content = r#"//! 自动生成的 protobuf 模块

pub mod distributed_downloader;
"#;

    // 如果不存在则创建 src/proto 目录
    let proto_dir = Path::new("src/proto");
    if !proto_dir.exists() {
        fs::create_dir_all(proto_dir).expect("创建 src/proto 目录失败");
    }

    // 创建 mod.rs 文件
    let mod_path = proto_dir.join("mod.rs");
    fs::write(mod_path, mod_content).expect("创建 src/proto/mod.rs 失败");
}