use crate::{Object, Storage, StorageError, validate_bucket, validate_key};
use async_trait::async_trait;
use bytes::Bytes;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

/// Oggetti come file sotto `<base>/<bucket>/<key>`. Servito dall'API stessa
/// (`/storage`), pensato per installazioni senza uno storage S3-compatible.
pub struct LocalStorage {
    base: PathBuf,
    public_url: String,
}

impl LocalStorage {
    /// `public_url`: prefisso con cui il browser raggiunge `<base>` (es. `http://localhost:8000/storage`).
    pub fn new(base: impl Into<PathBuf>, public_url: impl Into<String>) -> Self {
        Self { base: base.into(), public_url: public_url.into().trim_end_matches('/').to_string() }
    }

    pub fn base_path(&self) -> &Path {
        &self.base
    }

    fn path(&self, bucket: &str, key: &str) -> Result<PathBuf, StorageError> {
        validate_bucket(bucket)?;
        validate_key(key)?;
        Ok(self.base.join(bucket).join(key))
    }
}

fn io(e: std::io::Error) -> StorageError {
    StorageError::Backend(e.to_string())
}

#[async_trait]
impl Storage for LocalStorage {
    fn backend(&self) -> &'static str {
        "localstorage"
    }

    async fn ensure_bucket(&self, bucket: &str) -> Result<(), StorageError> {
        validate_bucket(bucket)?;
        tokio::fs::create_dir_all(self.base.join(bucket)).await.map_err(io)
    }

    async fn put(&self, bucket: &str, key: &str, data: Bytes, _content_type: &str) -> Result<(), StorageError> {
        let path = self.path(bucket, key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(io)?;
        }
        // scrittura atomica: un lettore non vede mai un file a metà
        let tmp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
        tokio::fs::write(&tmp, &data).await.map_err(io)?;
        tokio::fs::rename(&tmp, &path).await.map_err(io)
    }

    async fn get(&self, bucket: &str, key: &str) -> Result<Option<Object>, StorageError> {
        match tokio::fs::read(self.path(bucket, key)?).await {
            Ok(bytes) => Ok(Some(Object { data: Bytes::from(bytes), content_type: None })),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io(e)),
        }
    }

    async fn delete(&self, bucket: &str, key: &str) -> Result<(), StorageError> {
        match tokio::fs::remove_file(self.path(bucket, key)?).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(e)),
        }
    }

    async fn list(&self, bucket: &str, prefix: &str) -> Result<Vec<String>, StorageError> {
        validate_bucket(bucket)?;
        let root = self.base.join(bucket);
        let mut out = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let mut rd = match tokio::fs::read_dir(&dir).await {
                Ok(rd) => rd,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(io(e)),
            };
            while let Some(entry) = rd.next_entry().await.map_err(io)? {
                let path = entry.path();
                if entry.file_type().await.map_err(io)?.is_dir() {
                    stack.push(path);
                } else if let Ok(rel) = path.strip_prefix(&root) {
                    let key = rel.to_string_lossy().replace('\\', "/");
                    if key.starts_with(prefix) && !key.contains(".tmp-") {
                        out.push(key);
                    }
                }
            }
        }
        out.sort();
        Ok(out)
    }

    async fn presigned_get(&self, _bucket: &str, _key: &str, _ttl: Duration) -> Result<Option<String>, StorageError> {
        Ok(None)
    }

    fn public_url(&self, bucket: &str, key: &str) -> String {
        format!("{}/{bucket}/{key}", self.public_url)
    }

    async fn health(&self) -> Result<(), StorageError> {
        tokio::fs::create_dir_all(&self.base).await.map_err(io)?;
        let probe = self.base.join(format!(".health-{}", uuid::Uuid::new_v4().simple()));
        tokio::fs::write(&probe, b"ok").await.map_err(io)?;
        tokio::fs::remove_file(&probe).await.map_err(io)
    }
}
