use std::collections::HashMap;
use std::path::PathBuf;
use rayon::prelude::*;
use tauri::{Emitter, State};
use uuid::Uuid;
use serde::Serialize;
use crate::photo::batch::{prepare_frames, PreparedFrames};
use crate::project::model::{FramePreset, Photo};
use crate::AppState;

#[derive(Serialize, Clone)]
struct ExportProgress {
    done: usize,
    total: usize,
    current_file: String,
}

#[derive(Serialize, Clone)]
struct ExportComplete {
    errors: Vec<String>,
    output_dir: String,
}

/// Open both frame PNGs and pre-resize/convert them for a given slot — done once
/// per export/print run so the per-photo hot path does no frame I/O or conversion.
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
fn run_bounded<R, F>(f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    match rayon::ThreadPoolBuilder::new().num_threads(4).build() {
        Ok(pool) => pool.install(f),
        Err(_) => f(),
    }
}

/// Expand a photo list by per-photo quantity: a photo with qty=3 appears 3 times
/// (qty floored at 1 so every selected photo is produced at least once).
fn expand_by_quantity(photos: &[Photo], qty: impl Fn(&Photo) -> u32) -> Vec<Photo> {
    photos
        .iter()
        .flat_map(|p| std::iter::repeat_n(p.clone(), qty(p).max(1) as usize))
        .collect()
}

#[tauri::command]
pub async fn export_batch(
    event_id: Uuid,
    batch_id: Uuid,
    canvas_preset_id: Uuid,
    export_quantities: HashMap<String, u32>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let event = state.store.load(event_id).map_err(|e| e.to_string())?;

    let batch = event
        .batches.iter().find(|b| b.id == batch_id)
        .ok_or_else(|| format!("batch {batch_id} not found"))?;
    let canvas_preset = event
        .canvas_presets.iter().find(|p| p.id == canvas_preset_id)
        .ok_or_else(|| format!("canvas preset {canvas_preset_id} not found"))?
        .clone();
    let frame_preset = event
        .active_frame_preset_id
        .and_then(|id| event.frame_presets.iter().find(|p| p.id == id))
        .ok_or("no active frame preset set for this event")?
        .clone();
    let output_root = event
        .output_folder.as_ref()
        .ok_or("no output folder configured — set one in event settings")?
        .clone();

    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let output_dir = output_root.join(&timestamp);
    std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;

    let original_photo_ids: Vec<Uuid> = batch.photos.iter().map(|p| p.id).collect();
    let photos = expand_by_quantity(&batch.photos, |p| {
        export_quantities.get(&p.id.to_string()).copied().unwrap_or(1)
    });

    let output_dir_clone = output_dir.clone();

    let slot_w = canvas_preset.slot_width();
    let slot_h = canvas_preset.slot_height();

    // Free tier => watermark output; Pro => clean.
    let watermark = state.watermark();

    let frames = load_and_prepare_frames(&frame_preset, slot_w, slot_h)?;

    // Store a reference to the state so we can update export_count after export
    let store = state.store.clone();

    // Background thread — does not block the IPC handler
    std::thread::spawn(move || {
        let chunk_size = (canvas_preset.photos_per_canvas as usize).max(1);
        let total_canvases = if photos.is_empty() { 0 } else { photos.len().div_ceil(chunk_size) };
        let done = std::sync::atomic::AtomicUsize::new(0);

        // Process canvases in parallel, bounded to 4 threads so peak memory
        // stays ~4 decoded photos (~400MB worst case) under the 500MB ceiling.
        let process_chunk = |(canvas_idx, chunk): (usize, &[crate::project::model::Photo])| -> Vec<String> {
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
                if let Err(e) = crate::photo::export::export_print_ready(&canvas, &out_path) {
                    errs.push(format!("{filename}: {e}"));
                }
            }

            let d = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let _ = app.emit("export-progress", ExportProgress {
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

        // Increment export_count for all exported photos
        if let Ok(mut evt) = store.load(event_id) {
            for batch in &mut evt.batches {
                for photo in &mut batch.photos {
                    if original_photo_ids.contains(&photo.id) {
                        let photo_id_str = photo.id.to_string();
                        let qty = export_quantities.get(&photo_id_str).copied().unwrap_or(1).max(1);
                        photo.export_count += qty;
                    }
                }
            }
            let _ = store.save(&evt);
        }

        let _ = app.emit("export-complete", ExportComplete {
            errors,
            output_dir: output_dir_clone.to_string_lossy().into_owned(),
        });
    });

    Ok(())
}

#[tauri::command]
pub async fn print_photos(
    event_id: Uuid,
    photo_ids: Vec<Uuid>,
    quantities: HashMap<Uuid, u32>,
    canvas_preset_id: Uuid,
    frame_preset_id: Option<Uuid>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    let canvas_preset = event
        .canvas_presets.iter().find(|p| p.id == canvas_preset_id)
        .ok_or_else(|| format!("canvas preset {canvas_preset_id} not found"))?
        .clone();
    // Frame preset: explicit choice from the confirm popup, else the active one.
    let frame_preset = frame_preset_id
        .or(event.active_frame_preset_id)
        .and_then(|id| event.frame_presets.iter().find(|p| p.id == id))
        .ok_or("no frame preset selected")?
        .clone();

    let watermark = state.watermark();

    // Expand photos by their requested quantities: a photo with qty=3 appears 3 times.
    let selected: Vec<Photo> = event
        .batches.iter()
        .flat_map(|b| &b.photos)
        .filter(|p| photo_ids.contains(&p.id))
        .cloned()
        .collect();
    let photos = expand_by_quantity(&selected, |p| quantities.get(&p.id).copied().unwrap_or(1));

    if photos.is_empty() {
        return Err("no photos selected".into());
    }

    let slot_w = canvas_preset.slot_width();
    let slot_h = canvas_preset.slot_height();

    let frames = load_and_prepare_frames(&frame_preset, slot_w, slot_h)?;

    // Frame in parallel, bounded to 4 threads to respect the memory ceiling.
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

    // Compose canvases (apply free-tier watermark per canvas)
    let canvases: Vec<_> = crate::canvas::compositor::compose_canvases(&framed, &canvas_preset)
        .into_iter()
        .map(|c| if watermark { crate::canvas::compositor::apply_watermark(&c) } else { c })
        .collect();

    // Write composed canvases to a temp dir. Actual printer submission is
    // deferred — for now we only produce the print-ready files.
    let tmp_dir = std::env::temp_dir().join("magnet_print");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

    let mut paths: Vec<PathBuf> = Vec::new();
    for (i, canvas) in canvases.iter().enumerate() {
        let p = tmp_dir.join(format!("print_{i:04}.jpg"));
        crate::photo::export::export_print_ready(canvas, &p).map_err(|e| e.to_string())?;
        paths.push(p);
    }

    // Increment print counts and persist
    for batch in &mut event.batches {
        for photo in &mut batch.photos {
            if photo_ids.contains(&photo.id) {
                photo.print_count += quantities.get(&photo.id).copied().unwrap_or(1);
            }
        }
    }
    state.store.save(&event).map_err(|e| e.to_string())?;

    // Return the number of print-ready canvas files produced.
    Ok(paths.len())
}
