//! Varianti responsive delle immagini di contenuto, generate all'upload.
//!
//! Dall'originale (PNG/JPEG/WEBP, mai le GIF: potrebbero essere animate) si ricavano versioni WebP
//! al 100% (limitato a [`MAX_BASE_WIDTH`]), 75%, 50% e 25% della larghezza, scartando quelle sotto
//! [`MIN_VARIANT_WIDTH`]. L'originale resta com'è.
//!
//! Il frontend non interroga nessun elenco: le dimensioni stanno nel nome dell'originale
//! (`{uuid}-{W}x{H}.{ext}`) e le larghezze delle varianti si ricalcolano con [`variant_widths`]
//! (stessa regola in `frontend/src/lib/responsive-image.ts`: vanno tenute allineate). Una variante è
//! `{uuid}-{W}x{H}-{w}.webp`. Le immagini senza dimensioni nel nome (precedenti a questa funzione,
//! GIF) non hanno varianti e si servono così come sono.

use image::{DynamicImage, ImageDecoder, ImageReader, Limits, imageops::FilterType};
use std::io::Cursor;

pub const MAX_BASE_WIDTH: u32 = 2560;
pub const MIN_VARIANT_WIDTH: u32 = 320;
pub const WEBP_QUALITY: f32 = 80.0;
/// Oltre questa area una piccola immagine compressa può occupare gigabyte da decodificata.
pub const MAX_PIXELS: u64 = 40_000_000;

/// Larghezze (px) delle varianti WebP di un originale largo `width`, dalla più grande.
pub fn variant_widths(width: u32) -> Vec<u32> {
    let base = width.min(MAX_BASE_WIDTH);
    let mut out = vec![base];
    for (num, den) in [(3u64, 4u64), (1, 2), (1, 4)] {
        let w = (u64::from(base) * num / den) as u32;
        if w >= MIN_VARIANT_WIDTH && !out.contains(&w) {
            out.push(w);
        }
    }
    out
}

pub struct Variant {
    pub width: u32,
    pub data: Vec<u8>,
}

pub struct Prepared {
    pub width: u32,
    pub height: u32,
    pub variants: Vec<Variant>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PrepareError {
    /// L'immagine dichiara un'area oltre [`MAX_PIXELS`].
    TooLarge,
}

fn encode_webp(img: &DynamicImage) -> Option<Vec<u8>> {
    let encoded = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        webp::Encoder::from_rgba(&rgba, rgba.width(), rgba.height()).encode(WEBP_QUALITY).to_vec()
    } else {
        let rgb = img.to_rgb8();
        webp::Encoder::from_rgb(&rgb, rgb.width(), rgb.height()).encode(WEBP_QUALITY).to_vec()
    };
    (!encoded.is_empty()).then_some(encoded)
}

/// Decodifica (applicando l'orientamento EXIF, come fa il browser con l'originale) e genera le
/// varianti. `Ok(None)` se il file non si decodifica: resta l'originale, senza varianti.
/// Operazione pesante in CPU: da chiamare fuori dal thread del runtime.
pub fn prepare(data: &[u8]) -> Result<Option<Prepared>, PrepareError> {
    let open = || ImageReader::new(Cursor::new(data)).with_guessed_format().ok();
    let (w0, h0) = match open().and_then(|r| r.into_dimensions().ok()) {
        Some(d) => d,
        None => return Ok(None),
    };
    if u64::from(w0) * u64::from(h0) > MAX_PIXELS {
        return Err(PrepareError::TooLarge);
    }
    let Some(mut reader) = open() else { return Ok(None) };
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_PIXELS * 8);
    reader.limits(limits);
    let Ok(mut decoder) = reader.into_decoder() else { return Ok(None) };
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let Ok(mut img) = DynamicImage::from_decoder(decoder) else { return Ok(None) };
    img.apply_orientation(orientation);

    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let mut variants = Vec::new();
    for w in variant_widths(width) {
        let encoded = if w == width {
            encode_webp(&img)
        } else {
            let h = ((u64::from(height) * u64::from(w) / u64::from(width)) as u32).max(1);
            encode_webp(&img.resize_exact(w, h, FilterType::Lanczos3))
        };
        match encoded {
            Some(data) => variants.push(Variant { width: w, data }),
            None => return Ok(None),
        }
    }
    Ok(Some(Prepared { width, height, variants }))
}

/// Chiave di una variante dalla chiave dell'originale `…/{uuid}-{W}x{H}.{ext}`.
pub fn variant_key(original_key: &str, width: u32) -> Option<String> {
    let (stem, _ext) = original_key.rsplit_once('.')?;
    parse_dims(stem)?;
    Some(format!("{stem}-{width}.webp"))
}

/// Tutte le chiavi delle varianti attese per un originale; vuoto se non ne ha.
pub fn variant_keys(original_key: &str) -> Vec<String> {
    let Some((stem, _)) = original_key.rsplit_once('.') else { return Vec::new() };
    let Some((w, _)) = parse_dims(stem) else { return Vec::new() };
    variant_widths(w).into_iter().filter_map(|vw| variant_key(original_key, vw)).collect()
}

/// `…-{W}x{H}` → (W, H).
fn parse_dims(stem: &str) -> Option<(u32, u32)> {
    let tail = stem.rsplit('/').next()?;
    let dims = tail.rsplit_once('-')?.1;
    let (w, h) = dims.split_once('x')?;
    let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_the_percentage_steps_and_drop_the_tiny_ones() {
        assert_eq!(variant_widths(2400), [2400, 1800, 1200, 600]);
        assert_eq!(variant_widths(4000), [2560, 1920, 1280, 640]);
        assert_eq!(variant_widths(800), [800, 600, 400]);
        assert_eq!(variant_widths(400), [400]);
        assert_eq!(variant_widths(100), [100]);
    }

    #[test]
    fn keys_carry_the_dimensions_of_the_original() {
        let k = "notturni/userdata/o/b/media/4f0c-2400x1600.png";
        assert_eq!(variant_key(k, 1200).as_deref(), Some("notturni/userdata/o/b/media/4f0c-2400x1600-1200.webp"));
        assert_eq!(variant_keys(k).len(), 4);
        // senza dimensioni (immagini precedenti, GIF) nessuna variante
        assert!(variant_keys("notturni/userdata/o/b/media/4f0c.png").is_empty());
        assert!(variant_key("x/4f0c-abc.png", 100).is_none());
    }

    #[test]
    fn a_small_png_gets_webp_variants_with_the_right_sizes() {
        let img = DynamicImage::new_rgb8(800, 400);
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let p = prepare(&png).unwrap().unwrap();
        assert_eq!((p.width, p.height), (800, 400));
        assert_eq!(p.variants.iter().map(|v| v.width).collect::<Vec<_>>(), [800, 600, 400]);
        assert!(p.variants.iter().all(|v| v.data.starts_with(b"RIFF") && &v.data[8..12] == b"WEBP"));
    }

    #[test]
    fn garbage_is_left_alone() {
        assert!(prepare(b"not an image").unwrap().is_none());
    }
}
