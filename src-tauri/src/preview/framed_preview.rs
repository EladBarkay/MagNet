use anyhow::Result;
use image::DynamicImage;
use crate::photo::{loader, crop, frame};
use crate::project::model::{Photo, FramePreset};

const PREVIEW_MAX: u32 = 1200;

/// Generate an in-memory preview (scaled for UI display, not print quality).
/// With a preset: center-crop to the orientation-correct ratio + frame overlay.
/// Without a preset (`None`): the raw full photo, just scaled down — no crop,
/// no frame, so "None" shows the real image at full preview resolution.
pub fn generate_framed_preview(photo: &Photo, preset: Option<&FramePreset>) -> Result<Vec<u8>> {
    let loaded = loader::load_photo(&photo.path)?;

    let preview = match preset {
        None => scale_for_preview(loaded),
        Some(preset) => {
            let orient = photo.effective_orientation();
            // Portrait photos crop to the inverted ratio so the preview matches export.
            let ratio = crate::photo::batch::orientation_ratio(preset, orient);
            let crop_rect = photo.crop_override.unwrap_or_else(|| {
                crop::compute_crop_rect(loaded.width(), loaded.height(), ratio)
            });
            let cropped = crop::apply_crop(&loaded, crop_rect);
            let base = scale_for_preview(cropped);
            frame::apply_frame_overlay(&base, preset.frame_path(orient))?
        }
    };

    let mut buf = Vec::new();
    preview.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg)?;
    Ok(buf)
}

/// Scale down so the longest side is at most `PREVIEW_MAX`, preserving aspect.
fn scale_for_preview(img: DynamicImage) -> DynamicImage {
    if img.width() > PREVIEW_MAX || img.height() > PREVIEW_MAX {
        img.thumbnail(PREVIEW_MAX, PREVIEW_MAX)
    } else {
        img
    }
}
