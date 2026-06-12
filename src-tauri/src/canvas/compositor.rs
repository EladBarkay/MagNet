use std::borrow::Cow;
use image::{DynamicImage, Rgb, RgbImage};
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

    // RGB throughout: canvases are opaque white and exported as RGB JPEG, so
    // an alpha channel would only add memory traffic.
    let mut canvas = RgbImage::from_pixel(
        preset.canvas_width_px,
        preset.canvas_height_px,
        Rgb([255, 255, 255]),
    );

    for (i, img) in images.iter().enumerate() {
        let col = (i as u32) % preset.cols as u32;
        let row = (i as u32) / preset.cols as u32;
        let slot_x = margin + col * (slot_w + margin);
        let slot_y = margin + row * (slot_h + margin);

        // Framed images arrive pre-fitted to the slot — center them with white
        // letterboxing, never stretch. Contain-fit only if somehow oversized.
        let rgb: Cow<RgbImage> = if img.width() > slot_w || img.height() > slot_h {
            Cow::Owned(img.resize(slot_w, slot_h, image::imageops::FilterType::Triangle).to_rgb8())
        } else {
            match img.as_rgb8() {
                Some(b) => Cow::Borrowed(b),
                None => Cow::Owned(img.to_rgb8()),
            }
        };
        let x = slot_x + (slot_w - rgb.width()) / 2;
        let y = slot_y + (slot_h - rgb.height()) / 2;
        image::imageops::overlay(&mut canvas, &*rgb, x as i64, y as i64);
    }

    DynamicImage::ImageRgb8(canvas)
}

/// Apply a dependency-free, free-tier watermark: tiled translucent diagonal
/// stripes across the whole canvas. No bundled asset or font is required, so
/// this is robust regardless of install layout. Pro tier skips this entirely.
pub fn apply_watermark(canvas: &DynamicImage) -> DynamicImage {
    let mut output = canvas.to_rgb8();
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
                let [r, g, b] = px.0;
                // Blend toward white by `alpha`.
                let blend = |c: u8| -> u8 {
                    ((c as u32 * (255 - alpha) + 255 * alpha) / 255) as u8
                };
                *px = Rgb([blend(r), blend(g), blend(b)]);
            }
        }
    }

    DynamicImage::ImageRgb8(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use uuid::Uuid;

    const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
    const GREEN: Rgba<u8> = Rgba([0, 255, 0, 255]);

    fn two_up_preset() -> CanvasPreset {
        CanvasPreset {
            id: Uuid::new_v4(),
            name: "2-up 240×160".into(),
            canvas_width_px: 240,
            canvas_height_px: 160,
            photos_per_canvas: 2,
            dpi: 300,
            margin_px: 0,
            cols: 2,
            rows: 1,
        }
    }

    fn solid(w: u32, h: u32, color: Rgba<u8>) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, color))
    }

    #[test]
    fn smaller_image_is_centered_with_white_letterbox_not_stretched() {
        // Slot = 120×160. Image 100×160 → 10px white letterbox on each side.
        let preset = two_up_preset();
        let canvas = compose_one_canvas(&[solid(100, 160, GREEN)], &preset).to_rgba8();

        assert_eq!((canvas.width(), canvas.height()), (240, 160));
        // Letterbox edges stay white.
        assert_eq!(canvas.get_pixel(5, 80), &WHITE);
        assert_eq!(canvas.get_pixel(115, 80), &WHITE);
        // Centered content: x ∈ [10, 110).
        assert_eq!(canvas.get_pixel(15, 80), &GREEN);
        assert_eq!(canvas.get_pixel(60, 80), &GREEN);
        assert_eq!(canvas.get_pixel(105, 80), &GREEN);
    }

    #[test]
    fn exact_slot_size_image_fills_slot_without_resampling() {
        let preset = two_up_preset();
        let canvas = compose_one_canvas(&[solid(120, 160, GREEN), solid(120, 160, GREEN)], &preset)
            .to_rgba8();
        // Both slots fully covered, corner-to-corner.
        assert_eq!(canvas.get_pixel(0, 0), &GREEN);
        assert_eq!(canvas.get_pixel(119, 159), &GREEN);
        assert_eq!(canvas.get_pixel(120, 0), &GREEN);
        assert_eq!(canvas.get_pixel(239, 159), &GREEN);
    }

    #[test]
    fn second_slot_stays_white_when_canvas_is_partial() {
        let preset = two_up_preset();
        let canvas = compose_one_canvas(&[solid(120, 160, GREEN)], &preset).to_rgba8();
        // Second slot untouched.
        assert_eq!(canvas.get_pixel(180, 80), &WHITE);
    }

    #[test]
    fn oversized_image_is_contained_not_stretched() {
        // 300×160 (1.875:1) into 120×160 slot → contain-fit to 120×64, aspect kept.
        let preset = two_up_preset();
        let canvas = compose_one_canvas(&[solid(300, 160, GREEN)], &preset).to_rgba8();
        // Vertically centered band: white above/below, green in the middle.
        assert_eq!(canvas.get_pixel(60, 10), &WHITE);
        assert_eq!(canvas.get_pixel(60, 80), &GREEN);
        assert_eq!(canvas.get_pixel(60, 150), &WHITE);
    }
}
