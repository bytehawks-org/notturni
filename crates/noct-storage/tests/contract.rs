//! Lo stesso contratto verificato su entrambi i backend: sostituirli non deve
//! cambiare il comportamento osservabile. Il test S3 richiede un server
//! S3-compatible (`NOCT_TEST_S3_ENDPOINT`, es. moto o Garage; credenziali `test`/`test`).

use bytes::Bytes;
use noct_storage::{LocalStorage, S3Config, S3Storage, Storage};
use std::time::Duration;

async fn contract(s: &dyn Storage, bucket: &str) {
    s.health().await.unwrap();
    s.ensure_bucket(bucket).await.unwrap();
    s.ensure_bucket(bucket).await.unwrap(); // idempotente

    assert_eq!(s.get(bucket, "mancante.txt").await.unwrap(), None);
    s.put(bucket, "avatar/u1.png", Bytes::from_static(b"\x89PNG-dati"), "image/png").await.unwrap();
    s.put(bucket, "avatar/u2.png", Bytes::from_static(b"due"), "image/png").await.unwrap();
    s.put(bucket, "post/a/cover.webp", Bytes::from_static(b"cover"), "image/webp").await.unwrap();

    let got = s.get(bucket, "avatar/u1.png").await.unwrap().expect("oggetto presente");
    assert_eq!(got.data, Bytes::from_static(b"\x89PNG-dati"));

    // sovrascrittura
    s.put(bucket, "avatar/u1.png", Bytes::from_static(b"v2"), "image/png").await.unwrap();
    assert_eq!(s.get(bucket, "avatar/u1.png").await.unwrap().unwrap().data, Bytes::from_static(b"v2"));

    assert_eq!(s.list(bucket, "avatar/").await.unwrap(), ["avatar/u1.png", "avatar/u2.png"]);
    assert_eq!(s.list(bucket, "").await.unwrap().len(), 3);
    assert!(s.list(bucket, "niente/").await.unwrap().is_empty());

    s.delete(bucket, "avatar/u1.png").await.unwrap();
    s.delete(bucket, "avatar/u1.png").await.unwrap(); // idempotente
    assert_eq!(s.get(bucket, "avatar/u1.png").await.unwrap(), None);

    // chiavi pericolose rifiutate prima di toccare il backend
    for bad in ["../fuori", "a/../../b", "/assoluta", "a//b", ""] {
        assert!(s.put(bucket, bad, Bytes::new(), "text/plain").await.is_err(), "{bad:?}");
        assert!(s.get(bucket, bad).await.is_err(), "{bad:?}");
    }
    assert!(s.put("Bucket-Non-Valido", "k", Bytes::new(), "text/plain").await.is_err());

    assert!(s.public_url(bucket, "post/a/cover.webp").ends_with(&format!("/{bucket}/post/a/cover.webp")));
}

#[tokio::test]
async fn local_backend_honours_the_contract() {
    let dir = std::env::temp_dir().join(format!("noct-storage-{}", std::process::id()));
    let s = LocalStorage::new(&dir, "http://localhost:8000/storage");
    contract(&s, "contract-local").await;
    assert_eq!(s.presigned_get("contract-local", "post/a/cover.webp", Duration::from_secs(60)).await.unwrap(), None);
    assert_eq!(s.public_url("b-ucket", "k.png"), "http://localhost:8000/storage/b-ucket/k.png");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn s3_backend_honours_the_contract() {
    let endpoint = std::env::var("NOCT_TEST_S3_ENDPOINT").expect("NOCT_TEST_S3_ENDPOINT non impostata");
    let s = S3Storage::new(&S3Config {
        endpoint_url: Some(endpoint),
        region: Some("garage".into()),
        access_key_id: Some("test".into()),
        secret_access_key: Some("test".into()),
        public_url: Some("https://cdn.example.com".into()),
    })
    .await;
    let bucket = format!("contract-s3-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    contract(&s, &bucket).await;
    assert_eq!(s.public_url(&bucket, "k.png"), format!("https://cdn.example.com/{bucket}/k.png"));

    let url = s
        .presigned_get(&bucket, "post/a/cover.webp", Duration::from_secs(60))
        .await
        .unwrap()
        .expect("S3 firma gli URL");
    assert!(url.contains("X-Amz-Signature") && url.contains("cover.webp"), "{url}");
    let body = reqwest_get(&url).await;
    assert_eq!(body, b"cover");
}

/// GET minimale via TCP, per non aggiungere un client HTTP solo al test.
async fn reqwest_get(url: &str) -> Vec<u8> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rest = url.strip_prefix("http://").expect("endpoint di test in chiaro");
    let (host, path) = rest.split_once('/').unwrap();
    let mut sock = tokio::net::TcpStream::connect(host).await.unwrap();
    sock.write_all(format!("GET /{path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut buf = Vec::new();
    sock.read_to_end(&mut buf).await.unwrap();
    let split = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    assert!(buf.starts_with(b"HTTP/1.1 200"), "{}", String::from_utf8_lossy(&buf[..split]));
    buf[split..].to_vec()
}
