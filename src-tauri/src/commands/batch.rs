use std::collections::HashMap;
use std::path::PathBuf;
use rayon::prelude::*;
use tauri::{Emitter, State};
use uuid::Uuid;
use serde::Serialize;
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

#[tauri::command]
pub async fn export_batch(
    event_id: Uuid,
    batch_id: Uuid,
    canvas_preset_id: Uuid,
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

    let photos = batch.photos.clone();
    let output_dir_clone = output_dir.clone();

    let slot_w = canvas_preset.slot_width();
    let slot_h = canvas_preset.slot_height();

    // Free tier => watermark output; Pro => clean.
    let watermark = matches!(state.tier(), crate::license::validator::Tier::Free);

    // Pre-load and pre-resize both frames to canvas slot dimensions.
    // This avoids per-photo disk I/O and makes compositing work on slot-sized
    // images (~1–2 MP) rather than full-resolution photos (~20 MP).
    let landscape_frame = image::open(&frame_preset.landscape_frame_path)
        .map_err(|e| format!("loading landscape frame: {e}"))?
        .resize_exact(slot_w, slot_h, image::imageops::FilterType::Triangle);
    let portrait_frame = image::open(&frame_preset.portrait_frame_path)
        .map_err(|e| format!("loading portrait frame: {e}"))?
        .resize_exact(slot_w, slot_h, image::imageops::FilterType::Triangle);

    // Background thread — does not block the IPC handler
    std::thread::spawn(move || {
        let chunk_size = canvas_preset.photos_per_canvas as usize;
        let total_canvases = photos.len().div_ceil(chunk_size);
        let mut errors: Vec<String> = Vec::new();

        for (canvas_idx, chunk) in photos.chunks(chunk_size).enumerate() {
            // Frame each photo in the chunk in parallel
            let framed: Vec<_> = chunk
                .par_iter()
                .filter_map(|photo| {
                    crate::photo::batch::frame_photo_for_canvas(
                        photo, &frame_preset, slot_w, slot_h, &landscape_frame, &portrait_frame,
                    )
                    .map_err(|e| {
                        log::warn!("framing {}: {e}", photo.path.display());
                        e
                    })
                    .ok()
                })
                .collect();

            if framed.is_empty() {
                errors.push(format!("canvas {}: all photos failed to frame", canvas_idx + 1));
                continue;
            }

            // Compose and write the canvas
            let mut canvas = crate::canvas::compositor::compose_one_canvas(&framed, &canvas_preset);
            if watermark {
                canvas = crate::canvas::compositor::apply_watermark(&canvas);
            }
            let filename = format!("canvas_{:04}.jpg", canvas_idx + 1);
            let out_path = output_dir_clone.join(&filename);

            if let Err(e) = crate::photo::export::export_print_ready(&canvas, &out_path) {
                errors.push(format!("{filename}: {e}"));
            }

            let _ = app.emit("export-progress", ExportProgress {
                done: canvas_idx + 1,
                total: total_canvases,
                current_file: filename,
            });
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

    let watermark = matches!(state.tier(), crate::license::validator::Tier::Free);

    // Expand photos by their requested quantities: a photo with qty=3 appears 3 times
    let photos: Vec<_> = event
        .batches.iter()
        .flat_map(|b| &b.photos)
        .filter(|p| photo_ids.contains(&p.id))
        .flat_map(|p| {
            let qty = quantities.get(&p.id).copied().unwrap_or(1).max(1);
            std::iter::repeat(p.clone()).take(qty as usize)
        })
        .collect();

    if photos.is_empty() {
        return Err("no photos selected".into());
    }

    let slot_w = canvas_preset.slot_width();
    let slot_h = canvas_preset.slot_height();

    // Pre-load and pre-resize frames to slot dimensions (same speedup as export_batch)
    let landscape_frame = image::open(&frame_preset.landscape_frame_path)
        .map_err(|e| format!("loading landscape frame: {e}"))?
        .resize_exact(slot_w, slot_h, image::imageops::FilterType::Triangle);
    let portrait_frame = image::open(&frame_preset.portrait_frame_path)
        .map_err(|e| format!("loading portrait frame: {e}"))?
        .resize_exact(slot_w, slot_h, image::imageops::FilterType::Triangle);

    let framed: Vec<_> = photos
        .par_iter()
        .filter_map(|p| {
            crate::photo::batch::frame_photo_for_canvas(
                p, &frame_preset, slot_w, slot_h, &landscape_frame, &portrait_frame,
            ).ok()
        })
        .collect();

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
    let unique_ids: Vec<Uuid> = photo_ids.clone();
    for batch in &mut event.batches {
        for photo in &mut batch.photos {
            if unique_ids.contains(&photo.id) {
                photo.print_count += quantities.get(&photo.id).copied().unwrap_or(1);
            }
        }
    }
    state.store.save(&event).map_err(|e| e.to_string())?;

    // Return the number of print-ready canvas files produced.
    Ok(paths.len())
}
