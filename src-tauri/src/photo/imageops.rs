//! Image primitives shared by the save/print hot path and the preview path.
//!
//! Each function keeps a fast implementation AND the simple `image`-crate
//! fallback in one body, so swapping the fast path for the slow-but-simple one
//! is a single-spot edit (delete the fast branch, keep the fallback).

use image::{imageops, DynamicImage, RgbaImage};
use crate::project::model::CropRect;

/// Crop `img` to `rect` and downscale to `(w, h)`.
///
/// Fast path: `fast_image_resize` does crop+resize in one SIMD pass (no
/// intermediate full-res copy). Falls back to `crop_imm` + `resize_exact` for
/// pixel formats outside the fast path (e.g. 16-bit TIFF) — that fallback is
/// also the whole simple alternative if the fast path is ever dropped.
pub fn crop_and_resize(img: &DynamicImage, rect: CropRect, w: u32, h: u32) -> DynamicImage {
    fast_crop_resize(img, rect, w, h).unwrap_or_else(|| {
        img.crop_imm(rect.x, rect.y, rect.width, rect.height)
            .resize_exact(w, h, imageops::FilterType::Triangle)
    })
}

/// Alpha-composite an RGBA `frame` (already at `base` dimensions) over `base`.
///
/// Fast path: when `base` is RGB8 at matching dims, blend the frame in place —
/// skips the RGB→RGBA→RGB round-trip (the composited result is opaque anyway).
/// Otherwise use the generic `image::imageops::overlay`, which handles any
/// `DynamicImage` after a single `to_rgba8`.
pub fn overlay_frame(base: &DynamicImage, frame: &RgbaImage) -> DynamicImage {
    if let DynamicImage::ImageRgb8(rgb) = base {
        if rgb.dimensions() == frame.dimensions() {
            let mut out = rgb.clone();
            blend_rgba_over_rgb(&mut out, frame);
            return DynamicImage::ImageRgb8(out);
        }
    }
    let mut out = base.to_rgba8();
    if out.dimensions() == frame.dimensions() {
        imageops::overlay(&mut out, frame, 0, 0);
    } else {
        let resized = imageops::resize(frame, out.width(), out.height(), imageops::FilterType::Triangle);
        imageops::overlay(&mut out, &resized, 0, 0);
    }
    DynamicImage::ImageRgba8(out)
}

/// Crop + downscale in a single SIMD pass via `fast_image_resize`.
/// Returns `None` for pixel formats outside the fast path (caller falls back).
fn fast_crop_resize(img: &DynamicImage, rect: CropRect, w: u32, h: u32) -> Option<DynamicImage> {
    use fast_image_resize as fir;

    let (pixel_type, bytes, src_w, src_h) = match img {
        DynamicImage::ImageRgb8(b) => (fir::PixelType::U8x3, b.as_raw().as_slice(), b.width(), b.height()),
        DynamicImage::ImageRgba8(b) => (fir::PixelType::U8x4, b.as_raw().as_slice(), b.width(), b.height()),
        DynamicImage::ImageLuma8(b) => (fir::PixelType::U8, b.as_raw().as_slice(), b.width(), b.height()),
        _ => return None,
    };

    let src = fir::images::ImageRef::new(src_w, src_h, bytes, pixel_type).ok()?;
    let mut dst = fir::images::Image::new(w, h, pixel_type);
    let mut resizer = fir::Resizer::new();
    let opts = fir::ResizeOptions::new()
        .crop(rect.x as f64, rect.y as f64, rect.width as f64, rect.height as f64)
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Bilinear));
    resizer.resize(&src, &mut dst, &Some(opts)).ok()?;

    match pixel_type {
        fir::PixelType::U8x3 => image::RgbImage::from_raw(w, h, dst.into_vec()).map(DynamicImage::ImageRgb8),
        fir::PixelType::U8x4 => image::RgbaImage::from_raw(w, h, dst.into_vec()).map(DynamicImage::ImageRgba8),
        _ => image::GrayImage::from_raw(w, h, dst.into_vec()).map(DynamicImage::ImageLuma8),
    }
}

/// Alpha-blend an RGBA frame directly over an RGB base in place (opaque result).
fn blend_rgba_over_rgb(base: &mut image::RgbImage, frame: &RgbaImage) {
    debug_assert_eq!(base.dimensions(), frame.dimensions());
    for (b, f) in base.pixels_mut().zip(frame.pixels()) {
        let a = f.0[3] as u32;
        if a == 0 {
            continue;
        }
        if a == 255 {
            b.0 = [f.0[0], f.0[1], f.0[2]];
            continue;
        }
        let na = 255 - a;
        b.0[0] = ((f.0[0] as u32 * a + b.0[0] as u32 * na) / 255) as u8;
        b.0[1] = ((f.0[1] as u32 * a + b.0[1] as u32 * na) / 255) as u8;
        b.0[2] = ((f.0[2] as u32 * a + b.0[2] as u32 * na) / 255) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid_rgba(w: u32, h: u32, c: Rgba<u8>) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, c))
    }

    #[test]
    fn crop_and_resize_produces_target_dims() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::new(200, 100));
        let rect = CropRect { x: 50, y: 0, width: 100, height: 100 };
        let out = crop_and_resize(&img, rect, 64, 64);
        assert_eq!((out.width(), out.height()), (64, 64));
    }

    #[test]
    fn transparent_frame_preserves_base() {
        let base = solid_rgba(20, 20, Rgba([200, 100, 50, 255]));
        let frame = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 0]));
        let out = overlay_frame(&base, &frame).to_rgba8();
        assert_eq!(out.get_pixel(5, 5), &Rgba([200, 100, 50, 255]));
    }

    #[test]
    fn opaque_frame_covers_base() {
        let base = solid_rgba(20, 20, Rgba([200, 100, 50, 255]));
        let frame = RgbaImage::from_pixel(20, 20, Rgba([10, 20, 30, 255]));
        let out = overlay_frame(&base, &frame).to_rgba8();
        assert_eq!(out.get_pixel(10, 10), &Rgba([10, 20, 30, 255]));
    }

    #[test]
    fn rgb8_fast_path_blends_semi_transparent() {
        // 50% white over black → mid gray. Exercises the in-place RGB8 branch.
        let base = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(4, 4, image::Rgb([0, 0, 0])));
        let frame = RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 128]));
        let out = overlay_frame(&base, &frame);
        let px = out.to_rgb8().get_pixel(2, 2).0;
        assert!((px[0] as i32 - 128).abs() <= 1, "expected ~128, got {}", px[0]);
    }

    #[test]
    fn overlay_resizes_mismatched_frame() {
        let base = solid_rgba(100, 60, Rgba([10, 20, 30, 255]));
        let frame = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        let out = overlay_frame(&base, &frame);
        assert_eq!((out.width(), out.height()), (100, 60));
    }
}
