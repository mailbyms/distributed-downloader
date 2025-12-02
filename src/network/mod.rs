//! Network communication module

pub mod manager;
pub mod server;
pub mod client;
pub mod protobuf_helper;

// Re-export commonly used types
pub use manager::{ManagerNetwork, ServerInfo};
pub use server::ServerNetwork;
pub use client::ClientNetwork;