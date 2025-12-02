# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a distributed file downloader system implemented in Rust with three main components:
1. **Manager** - Coordinates servers and clients
2. **Server** - Downloads file segments and serves them to clients
3. **Client** - Requests files from servers through the manager

## Common Development Commands

### Building
```bash
cargo build
```

### Running Tests
```bash
cargo test
```

### Running Components
```bash
# Run manager
cargo run --bin manager -- --config configs/manager.yml

# Run server
cargo run --bin server -- --config configs/server.yml

# Run client (with URL to download)
cargo run --bin client -- [URL] --config configs/client.yml
```

## Architecture Overview

The system consists of three main binaries that communicate over TCP:

1. **Manager** (`src/bin/manager.rs`)
   - Listens for server registrations and deregistrations
   - Provides server list to clients
   - Maintains a registry of available servers

2. **Server** (`src/bin/server.rs`)
   - Registers with the manager on startup
   - Listens for client connections
   - Downloads file segments using HTTP
   - Serves downloaded segments to clients

3. **Client** (`src/bin/client.rs`)
   - Requests server list from manager
   - Divides download into segments based on server count
   - Connects to servers to download segments
   - Combines segments into final file

### Key Modules

- **config** - YAML-based configuration management for all components
- **network** - TCP communication handling for manager, server, and client
- **downloader** - HTTP downloading with support for range requests and multi-threading
- **utils** - Utility functions including file operations and download distribution algorithms
- **error** - Custom error types and handling

### Data Flow

1. Server starts and registers with Manager
2. Client requests server list from Manager
3. Client divides file download into segments
4. Client connects to each server with download metadata
5. Servers download their assigned segments
6. Servers send segments back to Client
7. Client combines segments into final file

### Configuration Files

Each component has its own YAML configuration file in the `configs/` directory:
- `manager.yml` - Manager network settings
- `server.yml` - Server network settings, directories, thread count
- `client.yml` - Client network settings and directories