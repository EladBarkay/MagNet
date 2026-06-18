use std::path::Path;
use anyhow::{Context, Result};
use image::DynamicImage;
use crate::photo::imageops;

/// Alpha-composite a frame PNG (with transparency) over `base`, scaling the
/// frame to match `base` dimensions. Used by the preview path, which loads the
/// frame from disk per request.
pub fn apply_frame_overlay(base: &DynamicImage, frame_path: &Path) -> Result<DynamicImage> {
    let frame = image::open(frame_path)
        .with_context(|| format!("loading frame {}", frame_path.display()))?
        .to_rgba8();
    Ok(imageops::overlay_frame(base, &frame))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn opaque_frame_covers_base_pixels() {
        let dir = std::env::temp_dir().join("magnet_frame_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let frame_path = dir.join("opaque.png");
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 255])))
            .save(&frame_path)
            .unwrap();

        let base = DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([200, 100, 50, 255])));
        let out = apply_frame_overlay(&base, &frame_path).unwrap().to_rgba8();
        assert_eq!(out.get_pixel(10, 10), &Rgba([0, 0, 0, 255]));
    }
}
