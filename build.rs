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
        },
        Err(e) => {
            eprintln!("错误：编译 protobuf 文件失败: {}", e);
            panic!("Protobuf compilation failed."); // Should not continue if this fails
        }
    }

    // 为构建设置环境变量
    println!("cargo:rustc-env=PROJECT_NAME=distributed-downloader");
    // Removed BUILD_TIME and chrono as config files are removed
}