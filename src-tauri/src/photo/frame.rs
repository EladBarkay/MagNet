use std::path::Path;
use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, RgbaImage};

/// Alpha-composite a frame PNG (with transparency) over a cropped photo.
/// The frame is scaled to exactly match `base` dimensions.
pub fn apply_frame_overlay(base: &DynamicImage, frame_path: &Path) -> Result<DynamicImage> {
    let frame = image::open(frame_path)
        .with_context(|| format!("loading frame {}", frame_path.display()))?;
    apply_frame_overlay_image(base, &frame)
}

/// Same as `apply_frame_overlay` but uses an already-loaded frame image.
/// Use this in batch contexts to avoid re-reading the same frame PNG for every photo.
pub fn apply_frame_overlay_image(base: &DynamicImage, frame: &DynamicImage) -> Result<DynamicImage> {
    let (w, h) = base.dimensions();
    let frame = frame.resize_exact(w, h, image::imageops::FilterType::Triangle);
    let mut output = base.to_rgba8();
    image::imageops::overlay(&mut output, &frame.to_rgba8(), 0, 0);
    Ok(DynamicImage::ImageRgba8(output))
}

/// Hot-path overlay for batch export/print: the frame is already RGBA8 and
/// already at the base dimensions, so no per-photo resample or buffer
/// conversion happens. Falls back to a resize only on a dimension mismatch.
pub fn apply_frame_overlay_prepared(base: &DynamicImage, frame: &RgbaImage) -> DynamicImage {
    let (w, h) = base.dimensions();
    let mut output = base.to_rgba8();
    if frame.dimensions() == (w, h) {
        image::imageops::overlay(&mut output, frame, 0, 0);
    } else {
        let resized = image::imageops::resize(frame, w, h, image::imageops::FilterType::Triangle);
        image::imageops::overlay(&mut output, &resized, 0, 0);
    }
    DynamicImage::ImageRgba8(output)
}

/// Fastest path: alpha-blend an RGBA frame directly over an RGB base in place.
/// Skips the RGB→RGBA→RGB round-trip entirely (the composited result is opaque
/// anyway — canvases are white, output is RGB JPEG).
/// Caller must guarantee equal dimensions.
pub fn blend_rgba_over_rgb(base: &mut image::RgbImage, frame: &RgbaImage) {
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
    use image::{Rgba, RgbaImage};

    #[test]
    fn overlay_matches_base_dimensions_and_is_resized() {
        // Base 100x60, frame a different size — output must match base dims.
        let base = DynamicImage::ImageRgba8(RgbaImage::from_pixel(100, 60, Rgba([10, 20, 30, 255])));
        let frame = DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0])));
        let out = apply_frame_overlay_image(&base, &frame).unwrap();
        assert_eq!((out.width(), out.height()), (100, 60));
    }

    #[test]
    fn fully_transparent_frame_preserves_base_pixels() {
        let base = DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([200, 100, 50, 255])));
        let frame = DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 0])));
        let out = apply_frame_overlay_image(&base, &frame).unwrap().to_rgba8();
        assert_eq!(out.get_pixel(5, 5), &Rgba([200, 100, 50, 255]));
    }

    #[test]
    fn opaque_frame_covers_base_pixels() {
        let base = DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([200, 100, 50, 255])));
        let frame = DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 255])));
        let out = apply_frame_overlay_image(&base, &frame).unwrap().to_rgba8();
        assert_eq!(out.get_pixel(10, 10), &Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn blend_rgb_transparent_frame_preserves_base() {
        let mut base = image::RgbImage::from_pixel(20, 20, image::Rgb([200, 100, 50]));
        let frame = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 0]));
        blend_rgba_over_rgb(&mut base, &frame);
        assert_eq!(base.get_pixel(5, 5), &image::Rgb([200, 100, 50]));
    }

    #[test]
    fn blend_rgb_opaque_frame_covers_base() {
        let mut base = image::RgbImage::from_pixel(20, 20, image::Rgb([200, 100, 50]));
        let frame = RgbaImage::from_pixel(20, 20, Rgba([10, 20, 30, 255]));
        blend_rgba_over_rgb(&mut base, &frame);
        assert_eq!(base.get_pixel(5, 5), &image::Rgb([10, 20, 30]));
    }

    #[test]
    fn blend_rgb_semi_transparent_frame_mixes_colors() {
        // 50% white over black → mid gray.
        let mut base = image::RgbImage::from_pixel(4, 4, image::Rgb([0, 0, 0]));
        let frame = RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 128]));
        blend_rgba_over_rgb(&mut base, &frame);
        let px = base.get_pixel(2, 2).0;
        assert!((px[0] as i32 - 128).abs() <= 1, "expected ~128, got {}", px[0]);
    }
}
