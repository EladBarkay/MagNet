use std::path::Path;
use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView};

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
