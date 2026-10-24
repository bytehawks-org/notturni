//! Storage a oggetti dietro un trait, con due implementazioni intercambiabili:
//! S3-compatibile (Garage, AWS, R2: endpoint sempre configurabile) e filesystem
//! locale (installazioni `solo`).

mod local;
mod s3;

pub use local::LocalStorage;
pub use s3::{S3Config, S3Storage};

use async_trait::async_trait;
pub use bytes::Bytes;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("chiave o bucket non valido: {0}")]
    Invalid(String),
    #[error("errore dello storage: {0}")]
    Backend(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub data: Bytes,
    /// Noto con S3; il backend locale non lo conserva (si deduce dall'estensione).
    pub content_type: Option<String>,
}

#[async_trait]
pub trait Storage: Send + Sync {
    fn backend(&self) -> &'static str;
    /// Crea il bucket se manca (idempotente).
    async fn ensure_bucket(&self, bucket: &str) -> Result<(), StorageError>;
    async fn put(&self, bucket: &str, key: &str, data: Bytes, content_type: &str) -> Result<(), StorageError>;
    async fn get(&self, bucket: &str, key: &str) -> Result<Option<Object>, StorageError>;
    /// Idempotente: cancellare ciò che non esiste non è un errore.
    async fn delete(&self, bucket: &str, key: &str) -> Result<(), StorageError>;
    async fn list(&self, bucket: &str, prefix: &str) -> Result<Vec<String>, StorageError>;
    /// URL temporaneo firmato per la lettura di un oggetto privato; `None` se il
    /// backend non lo supporta (locale).
    async fn presigned_get(&self, bucket: &str, key: &str, ttl: Duration) -> Result<Option<String>, StorageError>;
    /// URL pubblico stabile (per i bucket serviti pubblicamente).
    fn public_url(&self, bucket: &str, key: &str) -> String;
    async fn health(&self) -> Result<(), StorageError>;
}

pub(crate) fn validate_bucket(bucket: &str) -> Result<(), StorageError> {
    let ok = (3..=63).contains(&bucket.len())
        && bucket.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
        && bucket.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && !bucket.contains("..");
    if ok { Ok(()) } else { Err(StorageError::Invalid(format!("bucket `{bucket}`"))) }
}

/// Chiavi relative e senza trucchi di percorso: valide in modo identico per
/// tutti i backend, così il passaggio da uno all'altro non cambia semantica.
pub(crate) fn validate_key(key: &str) -> Result<(), StorageError> {
    let bad = key.is_empty()
        || key.len() > 1024
        || key.starts_with('/')
        || key.ends_with('/')
        || key.contains('\\')
        || key.contains('\0')
        || key.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..");
    if bad { Err(StorageError::Invalid(format!("key `{key}`"))) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_cannot_escape_or_be_ambiguous() {
        for ok in ["a", "a/b.png", "post/2026/x-y_z.webp"] {
            assert!(validate_key(ok).is_ok(), "{ok}");
        }
        for bad in ["", "/a", "a/", "a//b", "../a", "a/../b", "a/./b", "a\\b", "a\0b", &"x".repeat(1025)] {
            assert!(validate_key(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn bucket_names_follow_s3_rules() {
        assert!(validate_bucket("notturni-content").is_ok());
        assert!(validate_bucket("av").is_err());
        assert!(validate_bucket("Avatars").is_err());
        assert!(validate_bucket("-abc").is_err());
        assert!(validate_bucket("a..b").is_err());
    }
}
