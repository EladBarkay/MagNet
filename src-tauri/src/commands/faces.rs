use crate::commands::batch::run_bounded;
use crate::commands::IntoTauri;
use crate::AppState;
use rayon::prelude::*;
use rustface::{Detector, ImageData};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{Emitter, State};

/// SeetaFace frontal detection model, embedded so there's no runtime path to
/// resolve or bundle separately. ~1.2MB.
const FACE_MODEL: &[u8] = include_bytes!("../../model/seeta_fd_frontal_v1.0.bin");

/// Downscale the longest side to this before detection. Keeps 24MP scans fast;
/// the count stays stable because `min_face_size` is set against this scale.
const MAX_SIDE: u32 = 1024;

#[derive(Serialize, Clone)]
struct FaceScanProgress {
    done: usize,
    total: usize,
}

/// Fresh detector from the embedded model — cheap (in-memory), one per rayon
/// worker since `Detector::detect` takes `&mut self` and isn't shareable.
fn new_detector() -> Box<dyn Detector> {
    let model = rustface::read_model(Cursor::new(FACE_MODEL)).expect("embedded face model parses");
    let mut det = rustface::create_detector_with_model(model);
    det.set_min_face_size(24);
    det.set_score_thresh(2.0);
    det.set_pyramid_scale_factor(0.8);
    det.set_slide_window_step(4, 4);
    det
}

/// Count faces in one photo. Decode failures count as 0 (skip, like the export
/// path) so one bad file doesn't abort the scan.
fn count_faces_in_image(detector: &mut dyn Detector, path: &Path) -> u32 {
    let Ok(img) = image::open(path) else { return 0 };
    let img = if img.width().max(img.height()) > MAX_SIDE {
        img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    if w == 0 || h == 0 {
        return 0;
    }
    detector.detect(&ImageData::new(&gray, w, h)).len() as u32
}

/// Scan the given photos and return a `photoPath → face count` map. The frontend
/// passes the active folder's photo paths (or the selected subset) and uses the
/// result to seed per-photo export quantities (a suggestion the user can adjust).
/// Runs on the bounded rayon pool off the async runtime; emits `face-scan-progress`
/// per photo so the UI shows a bar.
#[tauri::command]
pub async fn count_faces(
    photo_paths: Vec<std::path::PathBuf>,
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<HashMap<std::path::PathBuf, u32>, String> {
    let total = photo_paths.len();

    tokio::task::spawn_blocking(move || {
        let done = AtomicUsize::new(0);
        run_bounded(|| {
            photo_paths
                .par_iter()
                .map_init(new_detector, |det, path| {
                    let n = count_faces_in_image(det.as_mut(), path);
                    let d = done.fetch_add(1, Ordering::SeqCst) + 1;
                    let _ = app.emit(
                        crate::constants::events::FACE_SCAN_PROGRESS,
                        FaceScanProgress { done: d, total },
                    );
                    (path.clone(), n)
                })
                .collect::<HashMap<std::path::PathBuf, u32>>()
        })
    })
    .await
    .tauri()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Smallest check that the whole detect path is wired: the embedded model
    // parses, a detector builds, and a blank image yields zero faces.
    #[test]
    fn blank_image_has_no_faces() {
        let mut det = new_detector();
        let gray = image::GrayImage::from_pixel(200, 200, image::Luma([255]));
        assert_eq!(det.detect(&ImageData::new(&gray, 200, 200)).len(), 0);
    }
}
