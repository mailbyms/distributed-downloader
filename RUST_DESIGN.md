# Rust版本分布式下载器设计

## 项目结构

```
distributed-downloader-rs/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   ├── bin/
│   │   ├── manager.rs
│   │   ├── server.rs
│   │   └── client.rs
│   ├── config/
│   │   ├── mod.rs
│   │   ├── manager.rs
│   │   ├── server.rs
│   │   └── client.rs
│   ├── network/
│   │   ├── mod.rs
│   │   ├── manager.rs
│   │   ├── server.rs
│   │   └── client.rs
│   ├── downloader/
│   │   ├── mod.rs
│   │   └── http.rs
│   ├── utils/
│   │   ├── mod.rs
│   │   ├── file.rs
│   │   └── distributor.rs
│   └── error/
│       ├── mod.rs
│       └── types.rs
├── configs/
│   ├── manager.yml
│   ├── server.yml
│   └── client.yml
└── tests/
    ├── integration_tests.rs
    └── unit_tests/
        ├── config_tests.rs
        ├── network_tests.rs
        ├── downloader_tests.rs
        └── utils_tests.rs
```

## 模块职责说明

### 1. 核心模块 (src/lib.rs)
- 导出所有公共模块
- 定义共享的数据结构和类型
- 提供通用的工具函数

### 2. 配置模块 (src/config/)
- `mod.rs`: 配置模块的入口，导出子模块
- `manager.rs`: 管理器配置结构和解析
- `server.rs`: 服务器配置结构和解析
- `client.rs`: 客户端配置结构和解析

### 3. 网络模块 (src/network/)
- `mod.rs`: 网络模块的入口，定义通用的网络类型和函数
- `manager.rs`: 管理器网络通信实现
- `server.rs`: 服务器网络通信实现
- `client.rs`: 客户端网络通信实现

### 4. 下载器模块 (src/downloader/)
- `mod.rs`: 下载器模块的入口
- `http.rs`: HTTP下载实现，支持范围请求和多线程下载

### 5. 工具模块 (src/utils/)
- `mod.rs`: 工具模块的入口
- `file.rs`: 文件操作工具（创建目录、合并文件、删除文件等）
- `distributor.rs`: 下载区间分配算法

### 6. 错误处理模块 (src/error/)
- `mod.rs`: 错误模块的入口
- `types.rs`: 自定义错误类型定义

### 7. 二进制文件 (src/bin/)
- `manager.rs`: 管理器主程序入口
- `server.rs`: 服务器主程序入口
- `client.rs`: 客户端主程序入口

## 依赖选择

### 核心依赖
- `tokio`: 异步运行时
- `serde` + `serde_yaml`: 配置文件序列化/反序列化
- `reqwest`: HTTP客户端
- `clap`: 命令行参数解析
- `tracing` + `tracing-subscriber`: 日志记录
- `uuid`: 生成唯一标识符
- `indicatif`: 进度条显示

### 开发依赖
- `tokio-test`: 异步测试工具
- `assert_fs`: 文件系统测试工具

## 并发模型

使用Tokio异步运行时：
- 异步网络I/O
- 异步文件I/O
- 任务并发而非线程并发

## 错误处理

使用Rust的`Result<T, E>`类型和自定义错误类型。

## 配置管理

使用YAML格式的配置文件，通过serde进行序列化/反序列化。