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

/// Apply a dependency-free, free-tier watermark: tiled translucent diagonal
/// stripes across the whole canvas. No bundled asset or font is required, so
/// this is robust regardless of install layout. Pro tier skips this entirely.
pub fn apply_watermark(canvas: &DynamicImage) -> DynamicImage {
    let mut output = canvas.to_rgba8();
    let (w, h) = (output.width(), output.height());

    // Stripe geometry scales with canvas size so it reads at any resolution.
    let band = (w.max(h) / 22).max(8); // width of one stripe pair component
    let period = band * 2;
    // Translucent white stripes — visible but non-destructive.
    let alpha: u32 = 38; // out of 255

    for y in 0..h {
        for x in 0..w {
            // Diagonal banding: stripe on when (x + y) falls in the first half.
            if ((x + y) % period) < band {
                let px = output.get_pixel_mut(x, y);
                let [r, g, b, a] = px.0;
                // Blend toward white by `alpha`.
                let blend = |c: u8| -> u8 {
                    ((c as u32 * (255 - alpha) + 255 * alpha) / 255) as u8
                };
                *px = Rgba([blend(r), blend(g), blend(b), a]);
            }
        }
    }

    DynamicImage::ImageRgba8(output)
}
