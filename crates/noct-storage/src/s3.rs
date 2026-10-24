use crate::{Object, Storage, StorageError, validate_bucket, validate_key};
use async_trait::async_trait;
use aws_sdk_s3::{
    Client,
    config::{BehaviorVersion, Credentials, Region, RequestChecksumCalculation, ResponseChecksumValidation},
    error::DisplayErrorContext,
    presigning::PresigningConfig,
    primitives::ByteStream,
    types::{BucketLocationConstraint, CreateBucketConfiguration},
};
use bytes::Bytes;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct S3Config {
    /// Endpoint dello storage: mai fisso nel codice (Garage in locale, S3/R2 in produzione).
    pub endpoint_url: Option<String>,
    pub region: Option<String>,
    /// Credenziali esplicite solo se entrambe presenti; altrimenti catena di
    /// default dell'SDK (ruolo AWS, variabili `AWS_*`, ...).
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    /// Base degli URL pubblici (`<base>/<bucket>/<key>`); senza, si usa l'endpoint.
    pub public_url: Option<String>,
}

pub struct S3Storage {
    client: Client,
    public_url: String,
    /// Regione effettiva: fuori da `us-east-1` AWS richiede il `LocationConstraint` alla creazione dei bucket.
    region: String,
}

fn backend<E: std::error::Error + 'static>(e: E) -> StorageError {
    StorageError::Backend(DisplayErrorContext(e).to_string())
}

impl S3Storage {
    pub async fn new(cfg: &S3Config) -> Self {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());
        // gli S3-compatible self-hosted vogliono comunque una regione: se manca ne basta una qualunque
        if let Some(region) = cfg.region.clone().or_else(|| cfg.endpoint_url.is_some().then(|| "us-east-1".to_string()))
        {
            loader = loader.region(Region::new(region));
        }
        let shared = loader.load().await;
        let mut b = aws_sdk_s3::config::Builder::from(&shared)
            // Garage e gli S3-compatible non risolvono `bucket.host` in DNS
            .force_path_style(true)
            // i checksum di default dell'SDK non sono accettati da tutti gli S3-compatible
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired);
        if let Some(endpoint) = &cfg.endpoint_url {
            b = b.endpoint_url(endpoint);
        }
        if let (Some(ak), Some(sk)) = (&cfg.access_key_id, &cfg.secret_access_key) {
            b = b.credentials_provider(Credentials::new(ak, sk, None, None, "noct"));
        }
        let public_url = cfg.public_url.clone().or_else(|| cfg.endpoint_url.clone()).unwrap_or_default();
        let config = b.build();
        let region = config.region().map(|r| r.to_string()).unwrap_or_else(|| "us-east-1".into());
        Self { client: Client::from_conf(config), public_url: public_url.trim_end_matches('/').to_string(), region }
    }
}

#[async_trait]
impl Storage for S3Storage {
    fn backend(&self) -> &'static str {
        "s3"
    }

    async fn ensure_bucket(&self, bucket: &str) -> Result<(), StorageError> {
        validate_bucket(bucket)?;
        if self.client.head_bucket().bucket(bucket).send().await.is_ok() {
            return Ok(());
        }
        let mut create = self.client.create_bucket().bucket(bucket);
        if self.region != "us-east-1" {
            create = create.create_bucket_configuration(
                CreateBucketConfiguration::builder()
                    .location_constraint(BucketLocationConstraint::from(self.region.as_str()))
                    .build(),
            );
        }
        match create.send().await {
            Ok(_) => Ok(()),
            Err(e) => {
                let already = e
                    .as_service_error()
                    .is_some_and(|s| s.is_bucket_already_owned_by_you() || s.is_bucket_already_exists());
                if already { Ok(()) } else { Err(backend(e)) }
            }
        }
    }

    async fn put(&self, bucket: &str, key: &str, data: Bytes, content_type: &str) -> Result<(), StorageError> {
        validate_bucket(bucket)?;
        validate_key(key)?;
        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from(data))
            .send()
            .await
            .map(|_| ())
            .map_err(backend)
    }

    async fn get(&self, bucket: &str, key: &str) -> Result<Option<Object>, StorageError> {
        validate_bucket(bucket)?;
        validate_key(key)?;
        match self.client.get_object().bucket(bucket).key(key).send().await {
            Ok(out) => {
                let content_type = out.content_type().map(str::to_string);
                let data = out.body.collect().await.map_err(|e| StorageError::Backend(e.to_string()))?.into_bytes();
                Ok(Some(Object { data, content_type }))
            }
            Err(e) if e.as_service_error().is_some_and(|s| s.is_no_such_key()) => Ok(None),
            Err(e) => Err(backend(e)),
        }
    }

    async fn delete(&self, bucket: &str, key: &str) -> Result<(), StorageError> {
        validate_bucket(bucket)?;
        validate_key(key)?;
        self.client.delete_object().bucket(bucket).key(key).send().await.map(|_| ()).map_err(backend)
    }

    async fn list(&self, bucket: &str, prefix: &str) -> Result<Vec<String>, StorageError> {
        validate_bucket(bucket)?;
        let mut keys = Vec::new();
        let mut pages = self.client.list_objects_v2().bucket(bucket).prefix(prefix).into_paginator().send();
        while let Some(page) = pages.next().await {
            let page = page.map_err(backend)?;
            keys.extend(page.contents().iter().filter_map(|o| o.key().map(str::to_string)));
        }
        keys.sort();
        Ok(keys)
    }

    async fn presigned_get(&self, bucket: &str, key: &str, ttl: Duration) -> Result<Option<String>, StorageError> {
        validate_bucket(bucket)?;
        validate_key(key)?;
        let cfg = PresigningConfig::expires_in(ttl).map_err(|e| StorageError::Invalid(e.to_string()))?;
        let req = self.client.get_object().bucket(bucket).key(key).presigned(cfg).await.map_err(backend)?;
        Ok(Some(req.uri().to_string()))
    }

    fn public_url(&self, bucket: &str, key: &str) -> String {
        format!("{}/{bucket}/{key}", self.public_url)
    }

    async fn health(&self) -> Result<(), StorageError> {
        self.client.list_buckets().send().await.map(|_| ()).map_err(backend)
    }
}
