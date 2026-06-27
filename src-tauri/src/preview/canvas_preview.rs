use crate::commands::export;
use crate::photo::compose::frame_image_for_canvas;
use crate::preview::thumbnail::ThumbnailCache;
use crate::project::model::{CanvasPreset, Event, FramePreset};
use anyhow::{anyhow, bail, Result};
use image::DynamicImage;
use std::collections::HashMap;
use std::path::PathBuf;

/// Downscale a canvas preset so its longest side is ~`max_px`, keeping the grid
/// (cols/rows/photos_per_canvas) intact. Small canvas ⇒ small slots ⇒ frames and
/// photos render at thumbnail resolution — a fast, low-memory preview.
fn scale_preset(preset: &CanvasPreset, max_px: u32) -> CanvasPreset {
    let longest = preset.canvas_width_px.max(preset.canvas_height_px);
    if longest == 0 || longest <= max_px {
        return preset.clone();
    }
    let scale = max_px as f32 / longest as f32;
    let mut p = preset.clone();
    // Keep at least one pixel per column/row so slot dims never collapse to 0.
    p.canvas_width_px =
        ((preset.canvas_width_px as f32 * scale).round() as u32).max(preset.cols.into());
    p.canvas_height_px =
        ((preset.canvas_height_px as f32 * scale).round() as u32).max(preset.rows.into());
    p
}

/// Render one export canvas page (`page_index`) as JPEG bytes, exactly as it would
/// export — same photo selection/order (`collect_selected` + `expand_by_quantity`),
/// same framing (`frame_image_for_canvas`), same tiling (`compose_one`) and free-tier
/// watermark — but at thumbnail resolution: photos come from the 256px thumbnail
/// cache and frames are prepared at the downscaled slot size.
#[allow(clippy::too_many_arguments)]
pub fn render_canvas_page(
    thumbs: &ThumbnailCache,
    event: &Event,
    frame_preset: &FramePreset,
    canvas_preset: &CanvasPreset,
    quantities: &HashMap<PathBuf, u32>,
    page_index: usize,
    watermark: bool,
    max_canvas_px: u32,
) -> Result<Vec<u8>> {
    let selected = export::collect_selected(event, quantities);
    let photos =
        export::expand_by_quantity(&selected, |p| quantities.get(&p.path).copied().unwrap_or(0));

    let ppc = (canvas_preset.photos_per_canvas as usize).max(1);
    let start = page_index * ppc;
    let end = (start + ppc).min(photos.len());
    let page = photos.get(start..end).unwrap_or(&[]);
    if page.is_empty() {
        bail!("canvas page {page_index} is empty");
    }

    let scaled = scale_preset(canvas_preset, max_canvas_px);
    let slot_w = scaled.slot_width();
    let slot_h = scaled.slot_height();
    let frames =
        export::load_and_prepare_frames(frame_preset, slot_w, slot_h).map_err(|e| anyhow!(e))?;

    // Per-photo framing from the cached thumbnail; a failed photo is skipped so
    // one unreadable file doesn't blank the whole page (mirrors export).
    let framed: Vec<DynamicImage> = page
        .iter()
        .filter_map(|photo| {
            let hash = (!photo.content_hash.is_empty()).then_some(photo.content_hash.as_str());
            let bytes = thumbs
                .get_or_generate(&photo.path, hash)
                .map_err(|e| log::warn!("preview thumb {}: {e}", photo.path.display()))
                .ok()?;
            let src = image::load_from_memory(&bytes).ok()?;
            Some(frame_image_for_canvas(
                &src,
                photo,
                frame_preset,
                slot_w,
                slot_h,
                &frames,
            ))
        })
        .collect();

    let mut canvas = crate::canvas::compositor::compose_one(&framed, &scaled);
    if watermark {
        canvas = crate::canvas::compositor::apply_watermark(&canvas);
    }
    let mut buf = Vec::new();
    canvas.write_to(
        &mut std::io::Cursor::new(&mut buf),
        image::ImageFormat::Jpeg,
    )?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::model::Photo;
    use image::{Rgba, RgbaImage};
    use uuid::Uuid;

    fn temp_jpeg(name: &str, w: u32, h: u32, color: Rgba<u8>) -> PathBuf {
        let dir = std::env::temp_dir().join("orenew_canvas_preview_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, color))
            .to_rgb8()
            .save(&path)
            .unwrap();
        path
    }

    fn temp_png(name: &str, w: u32, h: u32, color: Rgba<u8>) -> PathBuf {
        let dir = std::env::temp_dir().join("orenew_canvas_preview_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, color))
            .save(&path)
            .unwrap();
        path
    }

    fn photo(path: PathBuf, w: u32, h: u32) -> Photo {
        Photo {
            path,
            width: w,
            height: h,
            exif_orientation: None,
            orientation_override: None,
            crop_override: None,
            print_count: 0,
            save_count: 0,
            content_hash: String::new(),
            copies: 1,
            size_bytes: 0,
            created: 0,
            modified: 0,
        }
    }

    #[test]
    fn renders_a_nonempty_page_at_thumbnail_res() {
        let frame_preset = FramePreset {
            id: Uuid::new_v4(),
            name: "f".into(),
            landscape_frame_path: temp_png("cp_land.png", 60, 40, Rgba([0, 0, 0, 0])),
            portrait_frame_path: temp_png("cp_port.png", 40, 60, Rgba([0, 0, 0, 0])),
            target_ratio_w: 3.0,
            target_ratio_h: 2.0,
        };
        let canvas_preset = CanvasPreset {
            id: Uuid::new_v4(),
            name: "2-up".into(),
            canvas_width_px: 2400,
            canvas_height_px: 1600,
            photos_per_canvas: 2,
            dpi: 300,
            cols: 2,
            rows: 1,
        };

        let p1 = photo(
            temp_jpeg("cp_1.jpg", 600, 400, Rgba([200, 50, 50, 255])),
            600,
            400,
        );
        let p2 = photo(
            temp_jpeg("cp_2.jpg", 600, 400, Rgba([50, 50, 200, 255])),
            600,
            400,
        );

        let mut event = Event::new("e".into());
        event.frame_presets.push(frame_preset.clone());
        event.canvas_presets.push(canvas_preset.clone());
        event.photos.insert(p1.path.clone(), p1.clone());
        event.photos.insert(p2.path.clone(), p2.clone());

        // 3 + 1 copies, 2-up → 2 pages (matches the frontend's ceil(total/ppc)).
        let mut quantities = HashMap::new();
        quantities.insert(p1.path.clone(), 3u32);
        quantities.insert(p2.path.clone(), 1u32);
        let total: u32 = quantities.values().sum();
        let expected_pages = (total as usize).div_ceil(canvas_preset.photos_per_canvas as usize);
        assert_eq!(expected_pages, 2);

        let thumbs =
            ThumbnailCache::new(std::env::temp_dir().join("orenew_canvas_preview_tests/thumbs"))
                .unwrap();

        let render = |page: usize| {
            render_canvas_page(
                &thumbs,
                &event,
                &frame_preset,
                &canvas_preset,
                &quantities,
                page,
                true,
                900,
            )
        };

        // Every page in range renders; the first page after the last errors —
        // proving the backend's chunk boundary equals the frontend's page count.
        for page in 0..expected_pages {
            let bytes = render(page).unwrap();
            assert!(!bytes.is_empty());
            let img = image::load_from_memory(&bytes).unwrap();
            // Downscaled canvas (~900 longest side, 3:2 → ~600 tall).
            assert!(img.width() <= 901 && img.height() <= 901);
            assert!(img.width() > 1 && img.height() > 1);
        }
        assert!(render(expected_pages).is_err());
    }
}
