# 分布式下载器 (Rust版本)

一个用Rust编写的高性能分布式文件下载器。该项目通过将工作负载分布到多个服务器来加速文件下载。

## 特性

- **分布式架构**：采用管理器-服务器-客户端架构实现高效文件下载
- **多线程下载**：每个服务器可以使用多个线程下载文件片段
- **范围请求**：支持HTTP范围请求以实现高效的分段下载
- **进度跟踪**：为下载提供可视化进度条
- **配置管理**：基于YAML的配置文件
- **错误处理**：全面的错误处理和日志记录
- **跨平台**：支持Windows、macOS和Linux
- **高性能**：使用Rust构建，确保内存安全和性能

## 架构

系统由三个主要组件组成：

1. **管理器**：中央协调器，维护可用服务器列表，并代理客户端与服务器之间的所有通信
2. **服务器**：下载节点，实际下载文件片段，与管理器保持长连接以接收任务
3. **客户端**：启动下载并向管理器发送下载请求

## 系统工作流程

```
1. 启动管理器
   ↓
2. 启动服务器（向管理器注册并保持长连接）
   ↓
3. 客户端向管理器发送下载请求
   ↓
4. 管理器分割下载任务并分发给服务器
   ↓
5. 服务器下载文件段并通过管理器返回给客户端
   ↓
6. 客户端通过管理器接收所有文件段并合并生成完整文件
```

## 下载请求处理流程

### 客户端发起下载请求时的处理流程

当客户端发起下载请求时，管理器和服务器会按照以下步骤进行处理：

```mermaid
sequenceDiagram
    participant C as 客户端
    participant M as 管理器
    participant S1 as 服务器1
    participant S2 as 服务器2
    participant S3 as 服务器N

    C->>M: 发送下载请求和URL
    M->>M: 分析文件大小
    M->>M: 分割下载任务
    M->>S1: 转发任务1 (文件段1)
    M->>S2: 转发任务2 (文件段2)
    M->>S3: 转发任务N (文件段N)

    S1->>S1: 下载文件段1
    S1->>M: 发送文件段1给管理器
    M->>C: 转发文件段1给客户端

    S2->>S2: 下载文件段2
    S2->>M: 发送文件段2给管理器
    M->>C: 转发文件段2给客户端

    S3->>S3: 下载文件段N
    S3->>M: 发送文件段N给管理器
    M->>C: 转发文件段N给客户端

    C->>C: 接收所有文件段
    C->>C: 合并所有文件段生成完整文件
```

### 管理器处理流程

```mermaid
graph TD
    A[管理器启动] --> B[监听连接请求]
    B --> C{接收到请求}
    C -->|服务器注册| D[接收服务器信息]
    D --> E[添加到服务器列表]
    E --> F[记录服务器状态]

    C -->|服务器心跳| G[更新服务器状态]
    G --> H[保持长连接]

    C -->|客户端下载请求| I[接收下载请求]
    I --> J[分析文件信息]
    J --> K[分割下载任务]
    K --> L[分配任务给服务器]

    L --> M[转发任务给服务器]
    M --> N[接收服务器返回的文件段]
    N --> O[转发文件段给客户端]

    C -->|服务器断开连接| P[接收断开信息]
    P --> Q[从服务器列表移除]
    Q --> R[更新服务器状态]

    F --> B
    H --> B
    O --> B
    R --> B
```

### 服务器处理流程

```mermaid
graph TD
    A[服务器启动] --> B[向管理器注册]
    B --> C[与管理器建立长连接]
    C --> D[等待管理器任务]

    D --> E{接收到管理器任务}
    E -->|下载任务| F[接收下载元数据]
    F --> G[解析下载区间]
    G --> H[下载文件段]
    H --> I[发送文件段给管理器]
    I --> D

    E -->|心跳任务| J[发送心跳响应]
    J --> D

    E -->|其他指令| K[处理其他指令]
    K --> D

    C -->|连接断开| L[尝试重连]
    L --> B
```

## 安装

### 先决条件

- Rust 1.56或更高版本
- Cargo包管理器

### 从源码构建

```bash
git clone <repository-url>
cd distributed-downloader
cargo build --release
```

## 使用方法

### 1. 配置组件

根据`configs/`目录中的示例创建每个组件的配置文件：

- `configs/manager.yml`：管理器配置
- `configs/server.yml`：服务器配置
- `configs/client.yml`：客户端配置

### 2. 启动管理器

```bash
./target/release/manager --config configs/manager.yml
```

### 3. 启动服务器

```bash
./target/release/server --config configs/server.yml
```

您可以启动多个服务器以提高下载速度。

### 4. 启动客户端

```bash
./target/release/client --config configs/client.yml <URL>
```

将`<URL>`替换为您要下载的文件URL。

## 配置

### 管理器配置

```yaml
manager_addr_ipv4: 127.0.0.1
manager_port: 5000
```

### 服务器配置

```yaml
manager_addr_ipv4: 127.0.0.1
manager_port: 5000
tmp_dir: ./ddr-download/server/tmp/
threads_num: 4
target_dir: ./ddr-download/server/target/
```

### 客户端配置

```yaml
manager_addr_ipv4: 127.0.0.1
manager_port: 5000
tmp_dir: ./ddr-download/client/tmp/
target_dir: ./ddr-download/client/target/
```

## 调试
下载测试文件 `https://httpbin.org/json`
```bash
cargo run --bin manager -- --config configs/manager.yml
cargo run --bin server -- --config configs/server.yml
cargo run --bin client -- https://httpbin.org/json --config configs/client.yml
```

## 构建和运行测试

### 单元测试

```bash
cargo test
```

### 集成测试

```bash
cargo test --test integration_tests
```

## 项目结构

```
distributed-downloader/
├── src/
│   ├── lib.rs
│   ├── bin/
│   │   ├── manager.rs
│   │   ├── server.rs
│   │   └── client.rs
│   ├── config/
│   ├── network/
│   ├── downloader/
│   ├── utils/
│   └── error/
├── configs/
├── tests/
└── Cargo.toml
```

## 技术栈

- **语言**: Rust
- **异步运行时**: Tokio
- **HTTP客户端**: reqwest
- **序列化**: serde (JSON, YAML)
- **命令行解析**: clap
- **日志**: tracing
- **进度显示**: indicatif
- **测试**: tokio-test, assert_fs

## 已知限制

1. 当前实现仅支持HTTP/HTTPS协议
2. 服务器端的多线程下载功能在某些服务器上可能不可用
3. 暂不支持断点续传功能
4. 缺少图形用户界面

## 未来改进

1. 添加对FTP等其他协议的支持
2. 实现完整的断点续传功能
3. 开发图形用户界面
4. 增加更多配置选项
5. 改进错误处理和恢复机制