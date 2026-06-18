use std::collections::HashMap;
use std::path::PathBuf;
use rayon::prelude::*;
use tauri::{Emitter, State};
use uuid::Uuid;
use serde::Serialize;
use crate::commands::IntoTauri;
use crate::photo::batch::{prepare_frames, PreparedFrames};
use crate::project::model::{CanvasPreset, Event, FramePreset, Photo};
use crate::AppState;

#[derive(Serialize, Clone)]
struct SaveProgress {
    done: usize,
    total: usize,
    current_file: String,
}

#[derive(Serialize, Clone)]
struct SaveComplete {
    errors: Vec<String>,
    output_dir: String,
}

/// Open both frame PNGs and pre-resize/convert them for a given slot — done once
/// per save/print run so the per-photo hot path does no frame I/O or conversion.
fn load_and_prepare_frames(
    preset: &FramePreset,
    slot_w: u32,
    slot_h: u32,
) -> Result<PreparedFrames, String> {
    let landscape_src = image::open(&preset.landscape_frame_path)
        .map_err(|e| format!("loading landscape frame: {e}"))?;
    let portrait_src = image::open(&preset.portrait_frame_path)
        .map_err(|e| format!("loading portrait frame: {e}"))?;
    Ok(prepare_frames(preset, slot_w, slot_h, &landscape_src, &portrait_src))
}

/// Run `f` on a dedicated 4-thread rayon pool so peak memory stays ~4 decoded
/// photos under the 500MB ceiling; fall back to the global pool if one can't build.
pub(crate) fn run_bounded<R, F>(f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    match rayon::ThreadPoolBuilder::new().num_threads(4).build() {
        Ok(pool) => pool.install(f),
        Err(_) => f(),
    }
}

/// Expand a photo list by per-photo quantity: a photo with qty=3 appears 3 times,
/// qty=0 skips it entirely.
fn expand_by_quantity(photos: &[Photo], qty: impl Fn(&Photo) -> u32) -> Vec<Photo> {
    photos
        .iter()
        .flat_map(|p| std::iter::repeat_n(p.clone(), qty(p) as usize))
        .collect()
}

/// Collect photos in canonical event order (batch order → photo order within batch)
/// for the given quantity map. Keys of `quantities` determine which photos are included.
fn collect_selected(event: &Event, quantities: &HashMap<Uuid, u32>) -> Vec<Photo> {
    event
        .batches.iter()
        .flat_map(|b| &b.photos)
        .filter(|p| quantities.contains_key(&p.id))
        .cloned()
        .collect()
}

/// Everything a save/print run needs once the event is loaded — shared by both
/// `save_batch` and `print_photos`, which differ only in how they emit output.
struct PreparedBatch {
    photos: Vec<Photo>,
    canvas_preset: CanvasPreset,
    frame_preset: FramePreset,
    frames: PreparedFrames,
    watermark: bool,
    slot_w: u32,
    slot_h: u32,
}

/// Resolve presets, expand the quantity map (using `default_qty` for photos with
/// no explicit entry), validate non-empty, and load+prepare the frames once.
fn prepare_batch(
    event: &Event,
    quantities: &HashMap<Uuid, u32>,
    frame_preset_id: Uuid,
    canvas_preset_id: Uuid,
    default_qty: u32,
    watermark: bool,
) -> Result<PreparedBatch, String> {
    let canvas_preset = event.find_canvas_preset(canvas_preset_id)?.clone();
    let frame_preset = event.find_frame_preset(frame_preset_id)?.clone();

    let selected = collect_selected(event, quantities);
    let photos = expand_by_quantity(&selected, |p| {
        quantities.get(&p.id).copied().unwrap_or(default_qty)
    });
    if photos.is_empty() {
        return Err("No photos queued — set quantities on gallery photos first".into());
    }

    let slot_w = canvas_preset.slot_width();
    let slot_h = canvas_preset.slot_height();
    let frames = load_and_prepare_frames(&frame_preset, slot_w, slot_h)?;

    Ok(PreparedBatch { photos, canvas_preset, frame_preset, frames, watermark, slot_w, slot_h })
}

/// Add each photo's queued quantity to one of its counters (`field` selects which).
fn bump_counts(event: &mut Event, quantities: &HashMap<Uuid, u32>, field: fn(&mut Photo) -> &mut u32) {
    for batch in &mut event.batches {
        for photo in &mut batch.photos {
            if let Some(&qty) = quantities.get(&photo.id) {
                *field(photo) += qty;
            }
        }
    }
}

#[tauri::command]
pub async fn save_batch(
    event_id: Uuid,
    quantities: HashMap<Uuid, u32>,
    frame_preset_id: Uuid,
    canvas_preset_id: Uuid,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let event = state.store.load(event_id).tauri()?;

    let PreparedBatch { photos, canvas_preset, frame_preset, frames, watermark, slot_w, slot_h } =
        prepare_batch(&event, &quantities, frame_preset_id, canvas_preset_id, 0, state.watermark())?;

    let output_root = event
        .output_folder.as_ref()
        .ok_or("no output folder configured — set one in event settings")?
        .clone();

    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let output_dir = output_root.join(&timestamp);
    std::fs::create_dir_all(&output_dir).tauri()?;

    let output_dir_clone = output_dir.clone();
    let store = state.store.clone();

    // Background thread — does not block the IPC handler
    std::thread::spawn(move || {
        let chunk_size = (canvas_preset.photos_per_canvas as usize).max(1);
        let total_canvases = if photos.is_empty() { 0 } else { photos.len().div_ceil(chunk_size) };
        let done = std::sync::atomic::AtomicUsize::new(0);

        let process_chunk = |(canvas_idx, chunk): (usize, &[Photo])| -> Vec<String> {
            let mut errs: Vec<String> = Vec::new();
            let framed: Vec<_> = chunk
                .iter()
                .filter_map(|photo| {
                    crate::photo::batch::frame_photo_for_canvas(
                        photo, &frame_preset, slot_w, slot_h, &frames,
                    )
                    .map_err(|e| {
                        log::warn!("framing {}: {e}", photo.path.display());
                        errs.push(format!("{}: {e}", photo.path.display()));
                    })
                    .ok()
                })
                .collect();

            let filename = format!("canvas_{:04}.jpg", canvas_idx + 1);
            if framed.is_empty() {
                errs.push(format!("canvas {}: all photos failed to frame", canvas_idx + 1));
            } else {
                let mut canvas =
                    crate::canvas::compositor::compose_one(&framed, &canvas_preset);
                if watermark {
                    canvas = crate::canvas::compositor::apply_watermark(&canvas);
                }
                let out_path = output_dir_clone.join(&filename);
                if let Err(e) = crate::photo::encode::write_print_ready(&canvas, &out_path) {
                    errs.push(format!("{filename}: {e}"));
                }
            }

            let d = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let _ = app.emit(crate::constants::events::SAVE_PROGRESS, SaveProgress {
                done: d,
                total: total_canvases,
                current_file: filename,
            });
            errs
        };

        let errors: Vec<String> = run_bounded(|| {
            photos
                .par_chunks(chunk_size)
                .enumerate()
                .flat_map_iter(process_chunk)
                .collect()
        });

        // Increment save_count for all saved photos
        if let Ok(mut evt) = store.load(event_id) {
            bump_counts(&mut evt, &quantities, |p| &mut p.save_count);
            let _ = store.save(&evt);
        }

        let _ = app.emit(crate::constants::events::SAVE_COMPLETE, SaveComplete {
            errors,
            output_dir: output_dir_clone.to_string_lossy().into_owned(),
        });
    });

    Ok(())
}

#[tauri::command]
pub async fn print_photos(
    event_id: Uuid,
    quantities: HashMap<Uuid, u32>,
    frame_preset_id: Uuid,
    canvas_preset_id: Uuid,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let mut event = state.store.load(event_id).tauri()?;
    let PreparedBatch { photos, canvas_preset, frame_preset, frames, watermark, slot_w, slot_h } =
        prepare_batch(&event, &quantities, frame_preset_id, canvas_preset_id, 1, state.watermark())?;

    let framed: Vec<_> = run_bounded(|| {
        photos
            .par_iter()
            .filter_map(|p| {
                crate::photo::batch::frame_photo_for_canvas(
                    p, &frame_preset, slot_w, slot_h, &frames,
                ).ok()
            })
            .collect()
    });

    let canvases: Vec<_> = crate::canvas::compositor::compose_canvases(&framed, &canvas_preset)
        .into_iter()
        .map(|c| if watermark { crate::canvas::compositor::apply_watermark(&c) } else { c })
        .collect();

    let tmp_dir = std::env::temp_dir().join("magnet_print");
    std::fs::create_dir_all(&tmp_dir).tauri()?;

    let mut paths: Vec<PathBuf> = Vec::new();
    for (i, canvas) in canvases.iter().enumerate() {
        let p = tmp_dir.join(format!("print_{i:04}.jpg"));
        crate::photo::encode::write_print_ready(canvas, &p).tauri()?;
        paths.push(p);
    }

    bump_counts(&mut event, &quantities, |p| &mut p.print_count);
    state.store.save(&event).tauri()?;

    Ok(paths.len())
}
