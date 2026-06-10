use std::path::{Path, PathBuf};
use std::sync::mpsc;
use rayon::prelude::*;
use anyhow::Result;
use serde::Serialize;
use crate::project::model::{Photo, FramePreset};
use crate::photo::{loader, orientation, crop, frame, export};

#[derive(Debug, Clone, Serialize)]
pub struct BatchProgress {
    pub done: usize,
    pub total: usize,
    pub current_file: String,
}

#[derive(Debug)]
pub struct ProcessedPhoto {
    pub photo_id: uuid::Uuid,
    pub output_path: PathBuf,
}

pub struct BatchJob<'a> {
    pub photos: &'a [Photo],
    pub frame_preset: &'a FramePreset,
    pub output_dir: &'a Path,
}

/// Process all photos in parallel (bounded to 4 concurrent to cap memory).
/// Sends progress updates via `tx` after each photo completes.
/// Errors per photo are logged and skipped; the batch continues.
pub fn process_batch(
    job: BatchJob<'_>,
    tx: mpsc::Sender<BatchProgress>,
) -> Vec<Result<ProcessedPhoto>> {
    let total = job.photos.len();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4.min(rayon::current_num_threads()))
        .build()
        .expect("rayon pool");

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let done = Arc::new(AtomicUsize::new(0));

    pool.install(|| {
        job.photos
            .par_iter()
            .map(|photo| {
                let result = process_one(photo, job.frame_preset, job.output_dir);
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = tx.send(BatchProgress {
                    done: n,
                    total,
                    current_file: photo
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                });
                result
            })
            .collect()
    })
}

/// Frame a single photo (crop + overlay). Used by both batch export and print flows.
pub fn frame_photo(photo: &Photo, preset: &FramePreset) -> Result<image::DynamicImage> {
    let loaded = loader::load_photo(&photo.path)?;
    let orient = orientation::detect_orientation(photo);
    let frame_path = preset.frame_path(orient);
    let crop_rect = photo.crop_override.unwrap_or_else(|| {
        crop::compute_crop_rect(
            loaded.image.width(),
            loaded.image.height(),
            preset.target_ratio(),
            preset.crop_method,
        )
    });
    let cropped = crop::apply_crop(&loaded.image, crop_rect);
    frame::apply_frame_overlay(&cropped, frame_path)
}

fn process_one(
    photo: &Photo,
    preset: &FramePreset,
    output_dir: &Path,
) -> Result<ProcessedPhoto> {
    let framed = frame_photo(photo, preset)?;
    let filename = photo.path.file_stem().unwrap_or_default();
    let output_path = output_dir.join(format!("{}_framed.jpg", filename.to_string_lossy()));
    export::export_print_ready(&framed, &output_path)?;
    Ok(ProcessedPhoto {
        photo_id: photo.id,
        output_path,
    })
}
