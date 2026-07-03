use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use panorama_core::object_store::{BucketInfo, ObjectMetadata};
use panorama_core::types::ObjectRef;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Simple file-based object storage.
/// Objects are stored as files on disk, organized by bucket.
/// Resumable uploads are supported via temporary chunk files.
/// This is intentionally a simple implementation for v0.0.
#[derive(Clone)]
pub struct ObjectStorage {
    base_dir: PathBuf,
    /// In-memory metadata index
    objects: Arc<DashMap<String, ObjectMetadata>>, // key: "bucket/key"
    /// Pending resumable uploads
    uploads: Arc<DashMap<Uuid, PendingUpload>>,
}

#[derive(Debug, Clone)]
struct PendingUpload {
    bucket: String,
    key: String,
    mime_type: String,
    total_size: u64,
    chunks: HashMap<u32, Vec<u8>>,
}

impl ObjectStorage {
    pub fn new(base_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&base_dir).ok();
        let storage = Self {
            base_dir,
            objects: Arc::new(DashMap::new()),
            uploads: Arc::new(DashMap::new()),
        };
        storage.load_metadata();
        storage
    }

    fn bucket_dir(&self, bucket: &str) -> PathBuf {
        self.base_dir.join(bucket)
    }

    fn object_path(&self, bucket: &str, key: &str) -> PathBuf {
        self.bucket_dir(bucket).join(key)
    }

    fn meta_path(&self, bucket: &str, key: &str) -> PathBuf {
        self.bucket_dir(bucket).join(format!("{}.meta.json", key))
    }

    fn load_metadata(&self) {
        if let Ok(entries) = std::fs::read_dir(&self.base_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let bucket = entry.file_name().to_string_lossy().to_string();
                    if let Ok(files) = std::fs::read_dir(entry.path()) {
                        for file in files.flatten() {
                            let name = file.file_name().to_string_lossy().to_string();
                            if name.ends_with(".meta.json") {
                                if let Ok(content) = std::fs::read_to_string(file.path()) {
                                    if let Ok(meta) =
                                        serde_json::from_str::<ObjectMetadata>(&content)
                                    {
                                        let key = format!("{}/{}", bucket, meta.key);
                                        self.objects.insert(key, meta);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn save_metadata(&self, meta: &ObjectMetadata) {
        std::fs::create_dir_all(self.bucket_dir(&meta.bucket)).ok();
        if let Ok(json) = serde_json::to_string_pretty(meta) {
            std::fs::write(self.meta_path(&meta.bucket, &meta.key), json).ok();
        }
    }

    pub fn put(
        &self,
        bucket: &str,
        key: &str,
        data: &[u8],
        mime_type: &str,
    ) -> Result<ObjectRef, String> {
        std::fs::create_dir_all(self.bucket_dir(bucket)).map_err(|e| e.to_string())?;
        std::fs::write(self.object_path(bucket, key), data).map_err(|e| e.to_string())?;

        let mut hasher = Sha256::new();
        hasher.update(data);
        let etag = format!("{:x}", hasher.finalize());

        let meta = ObjectMetadata {
            id: Uuid::new_v4(),
            bucket: bucket.to_string(),
            key: key.to_string(),
            size: data.len() as u64,
            mime_type: mime_type.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            etag: etag.clone(),
        };

        let store_key = format!("{}/{}", bucket, key);
        self.objects.insert(store_key, meta.clone());
        self.save_metadata(&meta);

        Ok(ObjectRef {
            object_id: meta.id,
            bucket: bucket.to_string(),
            key: key.to_string(),
            size: meta.size,
            mime_type: meta.mime_type.clone(),
        })
    }

    pub fn get(&self, bucket: &str, key: &str) -> Result<Option<(Bytes, String)>, String> {
        let path = self.object_path(bucket, key);
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read(&path).map_err(|e| e.to_string())?;
        let store_key = format!("{}/{}", bucket, key);
        let mime = self
            .objects
            .get(&store_key)
            .map(|m| m.mime_type.clone())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        Ok(Some((Bytes::from(data), mime)))
    }

    pub fn get_range(
        &self,
        bucket: &str,
        key: &str,
        start: Option<u64>,
        end: Option<u64>,
    ) -> Result<Option<Bytes>, String> {
        let path = self.object_path(bucket, key);
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read(&path).map_err(|e| e.to_string())?;
        let s = start.unwrap_or(0) as usize;
        let e = end.map(|v| v as usize + 1).unwrap_or(data.len());
        let slice = &data[s..e.min(data.len())];
        Ok(Some(Bytes::copy_from_slice(slice)))
    }

    pub fn delete(&self, bucket: &str, key: &str) -> Result<(), String> {
        let store_key = format!("{}/{}", bucket, key);
        self.objects.remove(&store_key);
        std::fs::remove_file(self.object_path(bucket, key)).ok();
        std::fs::remove_file(self.meta_path(bucket, key)).ok();
        Ok(())
    }

    pub fn list(
        &self,
        bucket: &str,
        prefix: Option<&str>,
    ) -> Result<Vec<ObjectRef>, String> {
        let prefix_key = format!("{}/", bucket);
        let mut results: Vec<ObjectRef> = self
            .objects
            .iter()
            .filter(|entry| {
                let k = entry.key();
                if !k.starts_with(&prefix_key) {
                    return false;
                }
                if let Some(p) = prefix {
                    let obj_key = &k[prefix_key.len()..];
                    obj_key.starts_with(p)
                } else {
                    true
                }
            })
            .map(|entry| {
                let meta = entry.value();
                ObjectRef {
                    object_id: meta.id,
                    bucket: meta.bucket.clone(),
                    key: meta.key.clone(),
                    size: meta.size,
                    mime_type: meta.mime_type.clone(),
                }
            })
            .collect();
        results.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(results)
    }

    /// Initiate a resumable upload
    pub fn initiate_upload(
        &self,
        bucket: &str,
        key: &str,
        mime_type: &str,
        total_size: u64,
    ) -> Result<Uuid, String> {
        let upload_id = Uuid::new_v4();
        self.uploads.insert(
            upload_id,
            PendingUpload {
                bucket: bucket.to_string(),
                key: key.to_string(),
                mime_type: mime_type.to_string(),
                total_size,
                chunks: HashMap::new(),
            },
        );
        Ok(upload_id)
    }

    /// Upload a chunk for a resumable upload
    pub fn upload_chunk(
        &self,
        upload_id: &Uuid,
        chunk_index: u32,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let mut upload = self
            .uploads
            .get_mut(upload_id)
            .ok_or("Upload not found")?;
        upload.chunks.insert(chunk_index, data);
        Ok(())
    }

    /// Complete a resumable upload by assembling all chunks
    pub fn complete_upload(&self, upload_id: &Uuid) -> Result<ObjectRef, String> {
        let upload = self
            .uploads
            .remove(upload_id)
            .ok_or("Upload not found")?
            .1;

        let mut indices: Vec<u32> = upload.chunks.keys().copied().collect();
        indices.sort();

        let mut all_data = Vec::new();
        for idx in indices {
            if let Some(chunk) = upload.chunks.get(&idx) {
                all_data.extend_from_slice(chunk);
            }
        }

        self.put(&upload.bucket, &upload.key, &all_data, &upload.mime_type)
    }

    pub fn list_buckets(&self) -> Vec<BucketInfo> {
        let mut buckets: HashMap<String, (u64, u64)> = HashMap::new();
        for entry in self.objects.iter() {
            let meta = entry.value();
            let info = buckets
                .entry(meta.bucket.clone())
                .or_insert((0, 0));
            info.0 += 1;
            info.1 += meta.size;
        }
        buckets
            .into_iter()
            .map(|(name, (count, size))| BucketInfo {
                name,
                object_count: count,
                total_size: size,
            })
            .collect()
    }
}
