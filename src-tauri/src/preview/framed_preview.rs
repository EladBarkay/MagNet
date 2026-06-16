use anyhow::Result;
use crate::photo::{loader, crop, frame};
use crate::project::model::{Photo, FramePreset};

const PREVIEW_MAX: u32 = 1200;

/// Generate an in-memory framed preview (scaled for UI display, not print quality).
pub fn generate_framed_preview(photo: &Photo, preset: &FramePreset) -> Result<Vec<u8>> {
    let loaded = loader::load_photo(&photo.path)?;
    let orient = photo.effective_orientation();
    let frame_path = preset.frame_path(orient);

    // Portrait photos crop to the inverted ratio so the preview matches export.
    let ratio = crate::photo::batch::orientation_ratio(preset, orient);
    let crop_rect = photo.crop_override.unwrap_or_else(|| {
        crop::compute_crop_rect(
            loaded.width(),
            loaded.height(),
            ratio,
            preset.crop_method,
        )
    });

    let cropped = crop::apply_crop(&loaded, crop_rect);

    // Scale down for preview speed — not 300 DPI output
    let preview_base = if cropped.width() > PREVIEW_MAX || cropped.height() > PREVIEW_MAX {
        cropped.thumbnail(PREVIEW_MAX, PREVIEW_MAX)
    } else {
        cropped
    };

    let framed = frame::apply_frame_overlay(&preview_base, frame_path)?;

    let mut buf = Vec::new();
    framed.write_to(
        &mut std::io::Cursor::new(&mut buf),
        image::ImageFormat::Jpeg,
    )?;
    Ok(buf)
}
