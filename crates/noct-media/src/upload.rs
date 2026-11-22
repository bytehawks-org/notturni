//! Lettura dell'upload multipart, controllo del tipo reale dell'immagine e chiavi di storage.

use bytes::Bytes;
use futures::stream;
use noct_core::ApiError;
use uuid::Uuid;

pub const MAX_MEDIA_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_FAVICON_BYTES: usize = 512 * 1024;

/// Tipi ammessi per i media dei post e per la copertina.
pub const MEDIA_TYPES: &[(&str, &str)] =
    &[("image/png", "png"), ("image/jpeg", "jpg"), ("image/webp", "webp"), ("image/gif", "gif")];
/// Tipi ammessi per la favicon (niente GIF).
pub const FAVICON_TYPES: &[(&str, &str)] = &[("image/png", "png"), ("image/jpeg", "jpg"), ("image/webp", "webp")];

pub struct Upload {
    pub filename: String,
    pub content_type: String,
    pub data: Bytes,
}

/// Il campo `file` di un `multipart/form-data`.
pub async fn read_file(content_type: Option<&str>, body: Bytes) -> Result<Upload, ApiError> {
    let boundary = content_type
        .and_then(|ct| multer::parse_boundary(ct).ok())
        .ok_or_else(|| ApiError::unprocessable("Atteso un upload multipart/form-data con il campo `file`."))?;
    let mut mp = multer::Multipart::new(stream::once(async move { Ok::<_, std::convert::Infallible>(body) }), boundary);
    let bad = |e: multer::Error| ApiError::unprocessable(format!("Upload non valido: {e}"));
    while let Some(field) = mp.next_field().await.map_err(bad)? {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().unwrap_or("image").chars().take(255).collect();
        let content_type = field.content_type().map(|m| m.essence_str().to_string()).unwrap_or_default();
        let data = field.bytes().await.map_err(bad)?;
        return Ok(Upload { filename, content_type, data });
    }
    Err(ApiError::unprocessable("Campo `file` mancante."))
}

/// Tipo reale dall'intestazione del file (non ci si fida del `Content-Type` dichiarato).
pub fn sniff(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else {
        None
    }
}

/// Valida formato, coerenza con il tipo dichiarato e dimensione; restituisce l'estensione.
pub fn check_image(
    up: &Upload,
    allowed: &[(&str, &'static str)],
    max: usize,
    formats: &str,
    max_label: &str,
) -> Result<&'static str, ApiError> {
    let ext = |ct: &str| allowed.iter().find(|(c, _)| *c == ct).map(|(_, e)| *e);
    let declared = ext(&up.content_type);
    let real = sniff(&up.data).and_then(ext);
    match (declared, real) {
        (Some(d), Some(r)) if d == r => {}
        _ => return Err(ApiError::bad_request(format!("Formato immagine non supportato (usare {formats})."))),
    }
    if up.data.len() > max {
        return Err(ApiError::bad_request(format!("Il file supera la dimensione massima di {max_label}.")));
    }
    Ok(real.expect("verificato sopra"))
}

/// `{site}/userdata/{proprietario}/{blog}/media/{uuid}.{ext}`: il prefisso per utente e blog
/// permette di calcolare e cancellare lo spazio occupato da un blog.
///
/// Con `dims` (larghezza, altezza) il nome diventa `{uuid}-{W}x{H}.{ext}`: dice al frontend che esistono
/// le varianti responsive e quali (vedi [`crate::variants`]).
pub fn media_key(site_slug: &str, owner: Uuid, blog: Uuid, ext: &str, dims: Option<(u32, u32)>) -> String {
    let id = Uuid::new_v4();
    let name = match dims {
        Some((w, h)) => format!("{id}-{w}x{h}"),
        None => id.to_string(),
    };
    format!("{site_slug}/userdata/{owner}/{blog}/media/{name}.{ext}")
}

pub fn favicon_key(blog: Uuid, ext: &str) -> String {
    format!("favicons/{blog}/{}.{ext}", Uuid::new_v4())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";
    const JPG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0];
    const GIF: &[u8] = b"GIF89a....";

    fn up(ct: &str, data: &[u8]) -> Upload {
        Upload { filename: "x".into(), content_type: ct.into(), data: Bytes::copy_from_slice(data) }
    }

    #[test]
    fn real_format_must_match_the_declared_one() {
        assert_eq!(sniff(PNG), Some("image/png"));
        assert_eq!(sniff(JPG), Some("image/jpeg"));
        assert_eq!(sniff(GIF), Some("image/gif"));
        assert_eq!(sniff(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"<svg onload=alert(1)>"), None);
        assert_eq!(sniff(b""), None);

        assert_eq!(check_image(&up("image/png", PNG), MEDIA_TYPES, 100, "PNG", "x").unwrap(), "png");
        // HTML/SVG/script travestiti da immagine
        assert!(check_image(&up("image/png", b"<html></html>"), MEDIA_TYPES, 100, "PNG", "x").is_err());
        // dichiarato diverso dal reale
        assert!(check_image(&up("image/jpeg", PNG), MEDIA_TYPES, 100, "PNG", "x").is_err());
        // tipo non ammesso
        assert!(check_image(&up("image/gif", GIF), FAVICON_TYPES, 100, "PNG", "x").is_err());
        assert_eq!(check_image(&up("image/gif", GIF), MEDIA_TYPES, 100, "PNG", "x").unwrap(), "gif");
        let err = check_image(&up("image/png", PNG), MEDIA_TYPES, 4, "PNG", "4 byte").unwrap_err();
        assert!(err.detail.contains("4 byte"), "{}", err.detail);
    }

    #[test]
    fn keys_are_scoped_by_owner_and_blog() {
        let (o, b) = (Uuid::new_v4(), Uuid::new_v4());
        let k = media_key("notturni", o, b, "png", None);
        assert!(k.starts_with(&format!("notturni/userdata/{o}/{b}/media/")) && k.ends_with(".png"));
        assert!(media_key("notturni", o, b, "jpg", Some((800, 600))).ends_with("-800x600.jpg"));
        assert!(favicon_key(b, "webp").starts_with(&format!("favicons/{b}/")));
    }

    #[tokio::test]
    async fn multipart_file_field_is_extracted() {
        let body = Bytes::from(
            "--B\r\nContent-Disposition: form-data; name=\"altro\"\r\n\r\nx\r\n--B\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\nDATA\r\n--B--\r\n",
        );
        let u = read_file(Some("multipart/form-data; boundary=B"), body.clone()).await.unwrap();
        assert_eq!((u.filename.as_str(), u.content_type.as_str(), &u.data[..]), ("a.png", "image/png", &b"DATA"[..]));
        assert!(read_file(Some("application/json"), body.clone()).await.is_err());
        assert!(read_file(None, body).await.is_err());
        let none = Bytes::from("--B\r\nContent-Disposition: form-data; name=\"x\"\r\n\r\nv\r\n--B--\r\n");
        let err = read_file(Some("multipart/form-data; boundary=B"), none).await.err().unwrap();
        assert!(err.detail.contains("mancante"));
        assert!(read_file(Some("multipart/form-data; boundary=B"), Bytes::from("spazzatura")).await.is_err());
    }
}
