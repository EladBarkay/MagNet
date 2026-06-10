use std::path::Path;
use anyhow::Result;
use image::{DynamicImage, Rgba, RgbaImage};
use crate::project::model::CanvasPreset;

/// Tile framed images onto a blank canvas according to a `CanvasPreset`.
/// `framed_images` must have exactly `preset.photos_per_canvas` entries.
/// Returns one canvas per group of photos_per_canvas.
pub fn compose_canvases(
    framed_images: &[DynamicImage],
    preset: &CanvasPreset,
) -> Vec<DynamicImage> {
    framed_images
        .chunks(preset.photos_per_canvas as usize)
        .map(|chunk| compose_one(chunk, preset))
        .collect()
}

/// Compose a single canvas from a slice of framed images (exposed for the export command).
pub fn compose_one_canvas(images: &[DynamicImage], preset: &CanvasPreset) -> DynamicImage {
    compose_one(images, preset)
}

fn compose_one(images: &[DynamicImage], preset: &CanvasPreset) -> DynamicImage {
    let slot_w = preset.slot_width();
    let slot_h = preset.slot_height();
    let margin = preset.margin_px;

    let mut canvas = RgbaImage::from_pixel(
        preset.canvas_width_px,
        preset.canvas_height_px,
        Rgba([255, 255, 255, 255]),
    );

    for (i, img) in images.iter().enumerate() {
        let col = (i as u32) % preset.cols as u32;
        let row = (i as u32) / preset.cols as u32;
        let x = margin + col * (slot_w + margin);
        let y = margin + row * (slot_h + margin);

        let resized = img.resize_exact(slot_w, slot_h, image::imageops::FilterType::Lanczos3);
        image::imageops::overlay(&mut canvas, &resized.to_rgba8(), x as i64, y as i64);
    }

    DynamicImage::ImageRgba8(canvas)
}

/// Composite a semi-transparent watermark in the bottom-right of each canvas.
pub fn apply_watermark(canvas: &DynamicImage, watermark_path: &Path) -> Result<DynamicImage> {
    let wm = image::open(watermark_path)?;
    let mut output = canvas.to_rgba8();
    let (cw, ch) = (output.width(), output.height());
    let wm_w = cw / 6;
    let wm_h = (wm.height() as f32 * wm_w as f32 / wm.width() as f32) as u32;
    let wm_resized = wm.resize_exact(wm_w, wm_h, image::imageops::FilterType::Lanczos3);
    let x = cw - wm_w - 20;
    let y = ch - wm_h - 20;
    image::imageops::overlay(&mut output, &wm_resized.to_rgba8(), x as i64, y as i64);
    Ok(DynamicImage::ImageRgba8(output))
}
