use crate::photo::{crop, imageops, loader};
use crate::project::model::{FramePreset, Orientation, Photo};
use anyhow::Result;
use image::{DynamicImage, RgbaImage};

/// Frames pre-resized to their final placement dimensions and pre-converted
/// to RGBA8 — prepared **once** per save/print run so the per-photo hot path
/// does zero frame decoding, resizing, or buffer conversion.
pub struct PreparedFrames {
    pub landscape: RgbaImage,
    pub portrait: RgbaImage,
}

/// The crop ratio (w/h) a photo of `orient` should use. The preset ratio is
/// normalized so landscape photos always get the wide variant and portrait
/// photos the inverted (tall) variant, regardless of how the user entered it.
pub fn orientation_ratio(preset: &FramePreset, orient: Orientation) -> f32 {
    let r = preset.target_ratio();
    let inv = 1.0 / r;
    match orient {
        Orientation::Landscape => r.max(inv),
        Orientation::Portrait => r.min(inv),
    }
}

/// Largest (w, h) with aspect `ratio` (w/h) that fits inside (box_w, box_h).
pub fn fit_within(ratio: f32, box_w: u32, box_h: u32) -> (u32, u32) {
    let box_ratio = box_w as f32 / box_h as f32;
    if box_ratio > ratio {
        let h = box_h;
        let w = ((h as f32) * ratio).round() as u32;
        (w.max(1), h.max(1))
    } else {
        let w = box_w;
        let h = ((w as f32) / ratio).round() as u32;
        (w.max(1), h.max(1))
    }
}

/// Where a photo of `orient` lands inside a (slot_w × slot_h) slot.
/// Returns the framed image dimensions *before* rotation plus whether the
/// framed result should be rotated 90° for placement. Rotation is chosen only
/// when it lets the photo occupy strictly more of the slot (e.g. a landscape
/// photo in a portrait slot), never for equal fits.
pub fn placement(
    preset: &FramePreset,
    orient: Orientation,
    slot_w: u32,
    slot_h: u32,
) -> (u32, u32, bool) {
    let ratio = orientation_ratio(preset, orient);
    let (w0, h0) = fit_within(ratio, slot_w, slot_h);
    let (w1, h1) = fit_within(ratio, slot_h, slot_w);
    if (w1 as u64 * h1 as u64) > (w0 as u64 * h0 as u64) {
        (w1, h1, true)
    } else {
        (w0, h0, false)
    }
}

/// Prepare both frame PNGs for a given slot: resize each to its own
/// orientation-correct placement dimensions (aspect preserved — the portrait
/// frame is no longer squashed into landscape slot dims) and convert to RGBA8.
pub fn prepare_frames(
    preset: &FramePreset,
    slot_w: u32,
    slot_h: u32,
    landscape_src: &DynamicImage,
    portrait_src: &DynamicImage,
) -> PreparedFrames {
    let (lw, lh, _) = placement(preset, Orientation::Landscape, slot_w, slot_h);
    let (pw, ph, _) = placement(preset, Orientation::Portrait, slot_w, slot_h);
    PreparedFrames {
        landscape: landscape_src
            .resize_exact(lw, lh, image::imageops::FilterType::Triangle)
            .to_rgba8(),
        portrait: portrait_src
            .resize_exact(pw, ph, image::imageops::FilterType::Triangle)
            .to_rgba8(),
    }
}

/// Frame a photo for canvas placement:
/// 1. decode photo
/// 2. crop (centered) to the orientation-correct target ratio (portrait gets the inverted ratio)
/// 3. downscale (aspect-true) to the slot placement dimensions
/// 4. overlay the matching pre-prepared frame
/// 5. rotate 90° when that fills the slot better (frame rotates with the photo)
///
/// The result is never stretched; the compositor centers it in the slot.
pub fn frame_photo_for_canvas(
    photo: &Photo,
    preset: &FramePreset,
    slot_w: u32,
    slot_h: u32,
    frames: &PreparedFrames,
) -> Result<DynamicImage> {
    let loaded = loader::load_photo(&photo.path)?;
    Ok(frame_image_for_canvas(
        &loaded, photo, preset, slot_w, slot_h, frames,
    ))
}

/// Same framing as [`frame_photo_for_canvas`] but on an already-decoded source —
/// lets callers supply a full-res photo (export) or a downscaled thumbnail
/// (canvas preview) without re-reading the file. Steps 2–5 above.
///
/// ponytail: `crop_override` is applied directly to `source`. Today no UI sets it
/// (always `None` → centered crop), so a thumbnail source is correct. If a manual
/// crop UI lands, scale the rect to the source's dims for the preview path.
pub fn frame_image_for_canvas(
    source: &DynamicImage,
    photo: &Photo,
    preset: &FramePreset,
    slot_w: u32,
    slot_h: u32,
    frames: &PreparedFrames,
) -> DynamicImage {
    let orient = photo.effective_orientation();
    let ratio = orientation_ratio(preset, orient);
    let (target_w, target_h, rotate) = placement(preset, orient, slot_w, slot_h);
    let frame_img = match orient {
        Orientation::Landscape => &frames.landscape,
        Orientation::Portrait => &frames.portrait,
    };

    let crop_rect = photo
        .crop_override
        .unwrap_or_else(|| crop::compute_crop_rect(source.width(), source.height(), ratio));
    let scaled = imageops::crop_and_resize(source, crop_rect, target_w, target_h);
    let framed = imageops::overlay_frame(&scaled, frame_img);
    if rotate {
        framed.rotate90()
    } else {
        framed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn test_preset() -> FramePreset {
        FramePreset {
            id: Uuid::new_v4(),
            name: "test".into(),
            landscape_frame_path: PathBuf::new(),
            portrait_frame_path: PathBuf::new(),
            target_ratio_w: 3.0,
            target_ratio_h: 2.0,
        }
    }

    fn test_photo(path: PathBuf, w: u32, h: u32) -> Photo {
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

    fn write_temp_jpeg(name: &str, w: u32, h: u32, color: Rgba<u8>) -> PathBuf {
        let dir = std::env::temp_dir().join("orenew_tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, color))
            .to_rgb8()
            .save(&path)
            .unwrap();
        path
    }

    fn solid(w: u32, h: u32, color: Rgba<u8>) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, color))
    }

    const RED: Rgba<u8> = Rgba([255, 0, 0, 255]);
    const BLUE: Rgba<u8> = Rgba([0, 0, 255, 255]);
    const GRAY: Rgba<u8> = Rgba([128, 128, 128, 255]);

    #[test]
    fn orientation_ratio_inverts_for_portrait() {
        let preset = test_preset(); // 3:2
        let land = orientation_ratio(&preset, Orientation::Landscape);
        let port = orientation_ratio(&preset, Orientation::Portrait);
        assert!((land - 1.5).abs() < 1e-5);
        assert!((port - (2.0 / 3.0)).abs() < 1e-5);
    }

    #[test]
    fn orientation_ratio_normalizes_inverted_preset_input() {
        // User entered the ratio as 2:3 — landscape photos must still get the wide variant.
        let mut preset = test_preset();
        preset.target_ratio_w = 2.0;
        preset.target_ratio_h = 3.0;
        assert!((orientation_ratio(&preset, Orientation::Landscape) - 1.5).abs() < 1e-5);
        assert!((orientation_ratio(&preset, Orientation::Portrait) - (2.0 / 3.0)).abs() < 1e-5);
    }

    #[test]
    fn fit_within_preserves_ratio_and_bounds() {
        let (w, h) = fit_within(1.5, 300, 400);
        assert_eq!((w, h), (300, 200));
        let (w, h) = fit_within(1.5, 400, 300);
        assert_eq!((w, h), (400, 267));
    }

    #[test]
    fn landscape_photo_in_portrait_slot_rotates_for_bigger_fit() {
        let preset = test_preset();
        // Portrait slot 300×400: unrotated landscape fit = 300×200 (60k px),
        // rotated fit = 400×267 (107k px) → rotate.
        let (w, h, rotate) = placement(&preset, Orientation::Landscape, 300, 400);
        assert!(rotate);
        assert_eq!((w, h), (400, 267));
    }

    #[test]
    fn portrait_photo_in_portrait_slot_does_not_rotate() {
        let preset = test_preset();
        let (w, h, rotate) = placement(&preset, Orientation::Portrait, 300, 400);
        assert!(!rotate);
        assert_eq!((w, h), (267, 400));
    }

    #[test]
    fn square_slot_never_rotates() {
        let preset = test_preset();
        let (_, _, rot_l) = placement(&preset, Orientation::Landscape, 400, 400);
        let (_, _, rot_p) = placement(&preset, Orientation::Portrait, 400, 400);
        assert!(!rot_l);
        assert!(!rot_p);
    }

    #[test]
    fn prepared_frames_keep_distinct_orientations() {
        let preset = test_preset();
        let frames = prepare_frames(&preset, 300, 400, &solid(60, 40, RED), &solid(40, 60, BLUE));
        // Landscape frame is wide (rotated placement 400×267), portrait frame is tall (267×400).
        assert!(frames.landscape.width() > frames.landscape.height());
        assert!(frames.portrait.height() > frames.portrait.width());
    }

    #[test]
    fn landscape_photo_gets_landscape_frame_and_is_rotated_into_portrait_slot() {
        let preset = test_preset();
        let path = write_temp_jpeg("land_600x400.jpg", 600, 400, GRAY);
        let photo = test_photo(path, 600, 400);
        let frames = prepare_frames(&preset, 300, 400, &solid(60, 40, RED), &solid(40, 60, BLUE));

        let out = frame_photo_for_canvas(&photo, &preset, 300, 400, &frames).unwrap();

        // Rotated: 400×267 → 267×400. Fits the slot, taller than wide, not stretched.
        assert_eq!((out.width(), out.height()), (267, 400));
        assert!(out.width() <= 300 && out.height() <= 400);

        // Opaque RED frame covers the photo → proves the landscape frame was picked.
        let px = out.to_rgba8().get_pixel(10, 10).0;
        assert!(
            px[0] > 200 && px[1] < 50 && px[2] < 50,
            "expected red frame, got {px:?}"
        );
    }

    #[test]
    fn portrait_photo_gets_portrait_frame_without_rotation() {
        let preset = test_preset();
        let path = write_temp_jpeg("port_400x600.jpg", 400, 600, GRAY);
        let photo = test_photo(path, 400, 600);
        let frames = prepare_frames(&preset, 300, 400, &solid(60, 40, RED), &solid(40, 60, BLUE));

        let out = frame_photo_for_canvas(&photo, &preset, 300, 400, &frames).unwrap();

        assert_eq!((out.width(), out.height()), (267, 400));

        // Opaque BLUE frame → proves the portrait frame was picked (no landscape leak).
        let px = out.to_rgba8().get_pixel(10, 10).0;
        assert!(
            px[2] > 200 && px[0] < 50 && px[1] < 50,
            "expected blue frame, got {px:?}"
        );
    }

    /// Stage-by-stage timing breakdown for the hot path (diagnostic only).
    /// Run: `cargo test --release -- --ignored perf_breakdown --nocapture`
    #[test]
    #[ignore]
    fn perf_breakdown() {
        let preset = test_preset();
        let path = write_temp_jpeg(
            "perf_bd_6000x4000.jpg",
            6000,
            4000,
            Rgba([142, 95, 60, 255]),
        );
        let photo = test_photo(path.clone(), 6000, 4000);
        let frames = prepare_frames(
            &preset,
            1200,
            1600,
            &solid(1200, 800, RED),
            &solid(800, 1200, BLUE),
        );

        let t = std::time::Instant::now();
        let loaded = loader::load_photo(&path).unwrap();
        println!("decode:        {:?}", t.elapsed());

        let orient = photo.effective_orientation();
        let ratio = orientation_ratio(&preset, orient);
        let (tw, th, rotate) = placement(&preset, orient, 1200, 1600);
        let rect = crop::compute_crop_rect(6000, 4000, ratio);

        let t = std::time::Instant::now();
        let scaled = imageops::crop_and_resize(&loaded, rect, tw, th);
        println!("crop+resize:   {:?}", t.elapsed());

        let t = std::time::Instant::now();
        let framed = imageops::overlay_frame(&scaled, &frames.landscape);
        println!("overlay:       {:?}", t.elapsed());

        let t = std::time::Instant::now();
        let out = if rotate { framed.rotate90() } else { framed };
        println!(
            "rotate90:      {:?} (rotated={rotate}, {}x{})",
            t.elapsed(),
            out.width(),
            out.height()
        );
    }

    /// Performance regression guard for the per-photo export hot path.
    /// Target: < 100ms per photo (24MP source). Debug builds are 10-50× slower,
    /// so this only runs explicitly: `cargo test --release -- --ignored perf`
    #[test]
    #[ignore]
    fn perf_frame_photo_for_canvas_under_100ms() {
        let preset = test_preset();
        let path = write_temp_jpeg("perf_6000x4000.jpg", 6000, 4000, Rgba([142, 95, 60, 255]));
        let photo = test_photo(path, 6000, 4000);
        let frames = prepare_frames(
            &preset,
            1200,
            1600,
            &solid(1200, 800, RED),
            &solid(800, 1200, BLUE),
        );

        // Warm-up (page cache, allocator).
        frame_photo_for_canvas(&photo, &preset, 1200, 1600, &frames).unwrap();

        let iters = 5u32;
        let start = std::time::Instant::now();
        for _ in 0..iters {
            frame_photo_for_canvas(&photo, &preset, 1200, 1600, &frames).unwrap();
        }
        let avg = start.elapsed() / iters;
        assert!(
            avg < std::time::Duration::from_millis(100),
            "frame_photo_for_canvas avg {avg:?} ≥ 100ms target"
        );
    }
}
