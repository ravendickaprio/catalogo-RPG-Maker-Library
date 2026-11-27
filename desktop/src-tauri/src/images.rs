//! Procesamiento de portadas (Fase 5, ADR-014): equivalente a `comprimir_imagen` del legacy v04.
//! Decodifica (JPEG/PNG/WebP/GIF/BMP) con límites, aplana el alfa sobre blanco, reduce a un
//! máximo de 300x300 conservando la proporción (nunca amplía) y codifica WebP con pérdida q=65.
use crate::error::AppError;
use image::imageops::FilterType;
use image::{ImageFormat, ImageReader, Limits, RgbImage};
use std::io::Cursor;

pub const MAX_INPUT_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_DIMENSION: u32 = 12_000;
pub const MAX_ALLOC: u64 = 512 * 1024 * 1024;
pub const COVER_MAX: u32 = 300;
pub const WEBP_QUALITY: f32 = 65.0;

fn invalid(msg: &str) -> AppError {
    AppError::InvalidInput(msg.into())
}

/// Detects the container by magic bytes (never by extension).
pub fn detect_format(b: &[u8]) -> Option<ImageFormat> {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageFormat::Jpeg)
    } else if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(ImageFormat::Png)
    } else if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some(ImageFormat::WebP)
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some(ImageFormat::Gif)
    } else if b.starts_with(b"BM") {
        Some(ImageFormat::Bmp)
    } else {
        None
    }
}

/// Alpha composited over white, like the legacy `Image.new("RGB", ..., (255,255,255))` + paste.
fn flatten_on_white(rgba: &image::RgbaImage) -> RgbImage {
    let mut out = RgbImage::new(rgba.width(), rgba.height());
    for (o, p) in out.pixels_mut().zip(rgba.pixels()) {
        let a = p.0[3] as u32;
        for c in 0..3 {
            o.0[c] = ((p.0[c] as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
        }
    }
    out
}

pub fn fit_dimensions(w: u32, h: u32) -> (u32, u32) {
    if w <= COVER_MAX && h <= COVER_MAX {
        return (w, h);
    }
    let scale = (COVER_MAX as f64 / w as f64).min(COVER_MAX as f64 / h as f64);
    let nw = ((w as f64 * scale).round() as u32).clamp(1, COVER_MAX);
    let nh = ((h as f64 * scale).round() as u32).clamp(1, COVER_MAX);
    (nw, nh)
}

/// Turns any supported image into the stored cover (WebP lossy q=65, <= 300x300).
pub fn process_cover(bytes: &[u8]) -> Result<Vec<u8>, AppError> {
    if bytes.is_empty() {
        return Err(invalid("La imagen está vacía."));
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(invalid("La imagen supera el límite de 20 MB."));
    }
    let fmt = detect_format(bytes).ok_or_else(|| invalid("Formato no soportado"))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_ALLOC);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), fmt);
    reader.limits(limits);
    let img = reader
        .decode()
        .map_err(|e| AppError::InvalidInput(format!("No se pudo leer la imagen: {e}")))?;
    let rgb = flatten_on_white(&img.to_rgba8());
    drop(img);
    let (w, h) = (rgb.width(), rgb.height());
    let (nw, nh) = fit_dimensions(w, h);
    let rgb = if (nw, nh) == (w, h) {
        rgb
    } else {
        image::imageops::resize(&rgb, nw, nh, FilterType::Lanczos3)
    };
    let mut cfg = webp::WebPConfig::new()
        .map_err(|_| AppError::Database("No se pudo inicializar el codificador WebP.".into()))?;
    cfg.quality = WEBP_QUALITY;
    cfg.method = 6; // as the legacy (method=6)
    let enc = webp::Encoder::from_rgb(rgb.as_raw(), nw, nh);
    let out = enc
        .encode_advanced(&cfg)
        .map_err(|e| AppError::Database(format!("No se pudo codificar WebP: {e:?}")))?;
    Ok(out.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba, RgbaImage};

    fn encode(img: &DynamicImage, f: ImageFormat) -> Vec<u8> {
        let mut c = Cursor::new(Vec::new());
        img.write_to(&mut c, f).unwrap();
        c.into_inner()
    }

    fn decode(b: &[u8]) -> image::RgbImage {
        image::load_from_memory_with_format(b, ImageFormat::WebP)
            .unwrap()
            .to_rgb8()
    }

    fn gradient(w: u32, h: u32) -> image::RgbImage {
        image::RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        })
    }

    #[test]
    fn rgba_png_is_flattened_on_white_and_resized() {
        let mut img = RgbaImage::from_pixel(600, 400, Rgba([255, 0, 0, 255]));
        for y in 0..100 {
            for x in 0..100 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0])); // transparent corner
            }
        }
        let png = encode(&DynamicImage::ImageRgba8(img), ImageFormat::Png);
        let out = process_cover(&png).unwrap();
        assert_eq!(&out[0..4], b"RIFF");
        assert_eq!(&out[8..12], b"WEBP");
        let d = decode(&out);
        assert_eq!((d.width(), d.height()), (300, 200));
        let p = d.get_pixel(2, 2).0;
        assert!(p.iter().all(|&c| c >= 245), "corner not white: {p:?}");
        let q = d.get_pixel(250, 150).0;
        assert!(q[0] > 200 && q[1] < 60, "opaque area kept: {q:?}");
    }

    #[test]
    fn large_jpeg_keeps_aspect() {
        let jpg = encode(
            &DynamicImage::ImageRgb8(gradient(2000, 1000)),
            ImageFormat::Jpeg,
        );
        let d = decode(&process_cover(&jpg).unwrap());
        assert_eq!((d.width(), d.height()), (300, 150));
    }

    #[test]
    fn tall_image_fits_height() {
        let png = encode(
            &DynamicImage::ImageRgb8(gradient(100, 900)),
            ImageFormat::Png,
        );
        let d = decode(&process_cover(&png).unwrap());
        assert_eq!((d.width(), d.height()), (33, 300));
    }

    #[test]
    fn tiny_is_never_upscaled() {
        let png = encode(&DynamicImage::ImageRgb8(gradient(10, 10)), ImageFormat::Png);
        let d = decode(&process_cover(&png).unwrap());
        assert_eq!((d.width(), d.height()), (10, 10));
    }

    #[test]
    fn other_formats_are_accepted() {
        let img = DynamicImage::ImageRgb8(gradient(64, 48));
        for f in [ImageFormat::Gif, ImageFormat::Bmp, ImageFormat::WebP] {
            let out = process_cover(&encode(&img, f)).unwrap();
            assert_eq!(&out[8..12], b"WEBP", "{f:?}");
        }
    }

    #[test]
    fn rejects_bad_input() {
        let png = encode(&DynamicImage::ImageRgb8(gradient(64, 64)), ImageFormat::Png);
        // corrupted body behind a valid magic
        let mut bad = png[..40].to_vec();
        bad.extend_from_slice(&[0xAB; 20]);
        assert!(matches!(
            process_cover(&bad),
            Err(AppError::InvalidInput(_))
        ));
        assert!(
            matches!(process_cover(b"%PDF-1.4 not an image"), Err(AppError::InvalidInput(m)) if m == "Formato no soportado")
        );
        assert!(matches!(process_cover(&[]), Err(AppError::InvalidInput(_))));
        assert!(matches!(
            process_cover(&vec![0xFF; MAX_INPUT_BYTES + 1]),
            Err(AppError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_oversized_dimensions() {
        let img = image::GrayImage::from_pixel(MAX_DIMENSION + 1, 1, image::Luma([0]));
        let png = encode(&DynamicImage::ImageLuma8(img), ImageFormat::Png);
        assert!(matches!(
            process_cover(&png),
            Err(AppError::InvalidInput(_))
        ));
    }

    /// Manual timing: `cargo test --release --lib bench_process_cover -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bench_process_cover() {
        let noisy = |w: u32, h: u32| {
            image::RgbImage::from_fn(w, h, |x, y| {
                let n = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503)) >> 24;
                image::Rgb([
                    ((x / 8 + n) % 256) as u8,
                    ((y / 8 + n) % 256) as u8,
                    (n % 256) as u8,
                ])
            })
        };
        for (name, w, h, f) in [
            ("jpeg 2000x1000", 2000, 1000, ImageFormat::Jpeg),
            ("png 4000x3000", 4000, 3000, ImageFormat::Png),
        ] {
            let input = encode(&DynamicImage::ImageRgb8(noisy(w, h)), f);
            let mut best = f64::MAX;
            let mut out_len = 0;
            for _ in 0..5 {
                let t = std::time::Instant::now();
                let out = process_cover(&input).unwrap();
                best = best.min(t.elapsed().as_secs_f64() * 1000.0);
                out_len = out.len();
            }
            println!(
                "BENCH {name}: input {} B -> output {out_len} B, best of 5 = {best:.1} ms",
                input.len()
            );
        }
    }

    #[test]
    fn fit_dimensions_math() {
        assert_eq!(fit_dimensions(300, 300), (300, 300));
        assert_eq!(fit_dimensions(4000, 3000), (300, 225));
        assert_eq!(fit_dimensions(12000, 1), (300, 1));
    }
}
