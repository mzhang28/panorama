use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Object storage API - similar to S3 but custom implementation.
/// Objects are referenced from nodes via ObjectRef.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMetadata {
    pub id: Uuid,
    pub bucket: String,
    pub key: String,
    pub size: u64,
    pub mime_type: String,
    pub created_at: String,
    pub etag: String,
}

/// Request to initiate a resumable upload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiateUploadRequest {
    pub bucket: String,
    pub key: String,
    pub mime_type: String,
    pub total_size: u64,
}

/// Response when initiating a resumable upload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiateUploadResponse {
    pub upload_id: Uuid,
    pub chunk_size: u64,
}

/// A chunk in a resumable upload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadChunkRequest {
    pub upload_id: Uuid,
    pub chunk_index: u32,
    pub data: Vec<u8>,
}

/// Object storage bucket info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketInfo {
    pub name: String,
    pub object_count: u64,
    pub total_size: u64,
}

/// Range request for partial object reads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRange {
    pub start: Option<u64>,
    pub end: Option<u64>,
}
