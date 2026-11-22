//! Icona del portale: da un'unica immagine quadrata si ricavano le PNG per favicon, scorciatoie e manifest.

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, imageops::FilterType};
use std::io::Cursor;

/// Lati generati, dal più piccolo; si saltano quelli più grandi dell'immagine di partenza (mai ingrandire).
pub const SIZES: [u32; 6] = [16, 32, 48, 180, 192, 512];
pub const MIN_SIDE: u32 = 128;
/// Rapporto lato lungo / lato corto ammesso: oltre, l'immagine non è «quasi quadrata» e ritagliarla taglierebbe troppo.
pub const MAX_RATIO: f64 = 1.1;
const MAX_PIXELS: u64 = 40_000_000;

#[derive(Debug, PartialEq, Eq)]
pub enum IconError {
    Undecodable,
    TooLarge,
    TooSmall,
    NotSquare,
}

impl IconError {
    pub fn message(&self) -> String {
        match self {
            IconError::Undecodable => "L'immagine non si riesce a leggere.".into(),
            IconError::TooLarge => "L'immagine ha troppi pixel.".into(),
            IconError::TooSmall => format!("L'immagine deve essere di almeno {MIN_SIDE}×{MIN_SIDE} pixel."),
            IconError::NotSquare => "L'immagine deve essere quadrata (o quasi): ritaglia i bordi e riprova.".into(),
        }
    }
}

/// Lati da generare per un'immagine il cui lato corto misura `side`.
pub fn sizes_for(side: u32) -> Vec<u32> {
    SIZES.iter().copied().filter(|s| *s <= side).collect()
}

/// `(lato, PNG)` per ogni dimensione. Applica l'orientamento EXIF, ritaglia al centro il lato lungo in eccesso.
/// Operazione pesante in CPU: da chiamare fuori dal thread del runtime.
pub fn derive(data: &[u8]) -> Result<Vec<(u32, Vec<u8>)>, IconError> {
    let open = || ImageReader::new(Cursor::new(data)).with_guessed_format().ok();
    let (w0, h0) = open().and_then(|r| r.into_dimensions().ok()).ok_or(IconError::Undecodable)?;
    if u64::from(w0) * u64::from(h0) > MAX_PIXELS {
        return Err(IconError::TooLarge);
    }
    let mut reader = open().ok_or(IconError::Undecodable)?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_PIXELS * 8);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| IconError::Undecodable)?;
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|_| IconError::Undecodable)?;
    img.apply_orientation(orientation);

    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    if side < MIN_SIDE {
        return Err(IconError::TooSmall);
    }
    if f64::from(w.max(h)) / f64::from(side) > MAX_RATIO {
        return Err(IconError::NotSquare);
    }
    let square = img.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let mut out = Vec::new();
    for s in sizes_for(side) {
        let resized = if s == side { square.clone() } else { square.resize_exact(s, s, FilterType::Lanczos3) };
        let mut png = Cursor::new(Vec::new());
        resized.write_to(&mut png, ImageFormat::Png).map_err(|_| IconError::Undecodable)?;
        out.push((s, png.into_inner()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_fn(w, h, |x, y| Rgba([(x % 256) as u8, (y % 256) as u8, 120, 255]));
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn sizes_never_exceed_the_source() {
        assert_eq!(sizes_for(128), [16, 32, 48]);
        assert_eq!(sizes_for(192), [16, 32, 48, 180, 192]);
        assert_eq!(sizes_for(1024), SIZES);
    }

    #[test]
    fn square_images_are_resized_and_nearly_square_ones_are_cropped() {
        let out = derive(&png(600, 600)).unwrap();
        assert_eq!(out.iter().map(|(s, _)| *s).collect::<Vec<_>>(), [16, 32, 48, 180, 192, 512]);
        for (s, data) in &out {
            let img = image::load_from_memory(data).unwrap();
            assert_eq!((img.width(), img.height()), (*s, *s));
        }
        let cropped = derive(&png(200, 210)).unwrap();
        assert_eq!(cropped.iter().map(|(s, _)| *s).collect::<Vec<_>>(), [16, 32, 48, 180, 192]);
        assert_eq!(derive(&png(128, 128)).unwrap().len(), 3);
    }

    #[test]
    fn unsuitable_images_are_refused_with_a_clear_reason() {
        assert_eq!(derive(&png(100, 100)), Err(IconError::TooSmall));
        assert_eq!(derive(&png(400, 200)), Err(IconError::NotSquare));
        assert_eq!(derive(b"not an image"), Err(IconError::Undecodable));
        assert!(IconError::NotSquare.message().contains("quadrata"));
    }
}
