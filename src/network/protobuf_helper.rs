//! Helper functions for protobuf serialization/deserialization

use crate::proto::distributed_downloader::*;
use prost::Message as ProstMessage;

/// Serialize a protobuf message to bytes
pub fn serialize_message(message: &crate::proto::distributed_downloader::Message) -> Vec<u8> {
    let mut buf = Vec::new();
    ProstMessage::encode(message, &mut buf).expect("Failed to serialize message");
    buf
}

/// Deserialize a protobuf message from bytes
pub fn deserialize_message(data: &[u8]) -> Result<crate::proto::distributed_downloader::Message, prost::DecodeError> {
    ProstMessage::decode(data)
}

/// Create a server register message
pub fn create_server_register(server_id: u32) -> crate::proto::distributed_downloader::Message {
    let server_register = ServerRegister { server_id };
    crate::proto::distributed_downloader::Message {
        payload: Some(message::Payload::ServerRegister(server_register)),
    }
}

/// Create a server down message
pub fn create_server_down(server_id: u32) -> crate::proto::distributed_downloader::Message {
    let server_down = ServerDown { server_id };
    crate::proto::distributed_downloader::Message {
        payload: Some(message::Payload::ServerDown(server_down)),
    }
}

/// Create a server info message
pub fn create_server_info(id: u32) -> crate::proto::distributed_downloader::Message {
    let server_info = ServerInfo { id };
    crate::proto::distributed_downloader::Message {
        payload: Some(message::Payload::ServerInfo(server_info)),
    }
}

/// Create a download request message
pub fn create_download_request(url: String) -> crate::proto::distributed_downloader::Message {
    let download_request = DownloadRequest { url };
    crate::proto::distributed_downloader::Message {
        payload: Some(message::Payload::DownloadRequest(download_request)),
    }
}

/// Create a file info response message
pub fn create_file_info_response(
    file_size: u64,
    chunk_count: u32,
    chunk_sizes: Vec<u64>,
    final_url: String,
) -> crate::proto::distributed_downloader::Message {
    let file_info_response = FileInfoResponse {
        file_size,
        chunk_count,
        chunk_sizes,
        final_url,
    };
    crate::proto::distributed_downloader::Message {
        payload: Some(message::Payload::FileInfoResponse(file_info_response)),
    }
}

/// Create a download task message
pub fn create_download_task(
    url: String,
    start_position: u64,
    end_position: u64,
    task_id: String,
) -> crate::proto::distributed_downloader::Message {
    let download_task = DownloadTask {
        url,
        start_position,
        end_position,
        task_id,
    };
    crate::proto::distributed_downloader::Message {
        payload: Some(message::Payload::DownloadTask(download_task)),
    }
}

/// Create a task result message
pub fn create_task_result(
    task_id: String,
    success: bool,
    error_message: String,
) -> crate::proto::distributed_downloader::Message {
    let task_result = TaskResult {
        task_id,
        success,
        error_message,
    };
    crate::proto::distributed_downloader::Message {
        payload: Some(message::Payload::TaskResult(task_result)),
    }
}

/// Create a file data message
pub fn create_file_data(data: Vec<u8>) -> crate::proto::distributed_downloader::Message {
    crate::proto::distributed_downloader::Message {
        payload: Some(message::Payload::FileData(data)),
    }
}