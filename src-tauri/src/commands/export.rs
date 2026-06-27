use crate::commands::IntoTauri;
use crate::photo::compose::{prepare_frames, PreparedFrames};
use crate::project::model::{CanvasPreset, Event, FramePreset, Photo};
use crate::AppState;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{Emitter, State};
use uuid::Uuid;

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

/// Result of a print run. `dialog_opened` is true only where we can launch a real
/// OS print dialog (Windows); elsewhere the canvases are written to `output_dir`
/// and the frontend offers to open that folder so the user prints manually.
#[derive(Serialize, Clone)]
pub struct PrintResult {
    count: usize,
    dialog_opened: bool,
    output_dir: String,
}

/// Open both frame PNGs and pre-resize/convert them for a given slot — done once
/// per save/print run so the per-photo hot path does no frame I/O or conversion.
pub(crate) fn load_and_prepare_frames(
    preset: &FramePreset,
    slot_w: u32,
    slot_h: u32,
) -> Result<PreparedFrames, String> {
    let landscape_src = image::open(&preset.landscape_frame_path)
        .map_err(|e| format!("loading landscape frame: {e}"))?;
    let portrait_src = image::open(&preset.portrait_frame_path)
        .map_err(|e| format!("loading portrait frame: {e}"))?;
    Ok(prepare_frames(
        preset,
        slot_w,
        slot_h,
        &landscape_src,
        &portrait_src,
    ))
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
pub(crate) fn expand_by_quantity(photos: &[Photo], qty: impl Fn(&Photo) -> u32) -> Vec<Photo> {
    photos
        .iter()
        .flat_map(|p| std::iter::repeat_n(p.clone(), qty(p) as usize))
        .collect()
}

/// Collect the queued photos sorted by path (stable output order). Keys of
/// `quantities` determine which photos are included.
pub(crate) fn collect_selected(event: &Event, quantities: &HashMap<PathBuf, u32>) -> Vec<Photo> {
    let mut photos: Vec<Photo> = event
        .photos
        .values()
        .filter(|p| quantities.contains_key(&p.path))
        .cloned()
        .collect();
    photos.sort_by(|a, b| a.path.cmp(&b.path));
    photos
}

/// Everything a save/print run needs once the event is loaded — shared by both
/// `save_photos` and `print_photos`, which differ only in how they emit output.
struct PreparedExport {
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
fn prepare_export(
    event: &Event,
    quantities: &HashMap<PathBuf, u32>,
    frame_preset_id: Uuid,
    canvas_preset_id: Uuid,
    default_qty: u32,
    watermark: bool,
) -> Result<PreparedExport, String> {
    let canvas_preset = event.find_canvas_preset(canvas_preset_id)?.clone();
    let frame_preset = event.find_frame_preset(frame_preset_id)?.clone();

    let selected = collect_selected(event, quantities);
    let photos = expand_by_quantity(&selected, |p| {
        quantities.get(&p.path).copied().unwrap_or(default_qty)
    });
    if photos.is_empty() {
        return Err("No photos queued — set quantities on gallery photos first".into());
    }

    let slot_w = canvas_preset.slot_width();
    let slot_h = canvas_preset.slot_height();
    let frames = load_and_prepare_frames(&frame_preset, slot_w, slot_h)?;

    Ok(PreparedExport {
        photos,
        canvas_preset,
        frame_preset,
        frames,
        watermark,
        slot_w,
        slot_h,
    })
}

/// Add each photo's queued quantity to one of its counters (`field` selects which).
fn bump_counts(
    event: &mut Event,
    quantities: &HashMap<PathBuf, u32>,
    field: fn(&mut Photo) -> &mut u32,
) {
    for (path, photo) in &mut event.photos {
        if let Some(&qty) = quantities.get(path) {
            *field(photo) += qty;
        }
    }
}

#[tauri::command]
pub async fn save_photos(
    event_id: Uuid,
    quantities: HashMap<PathBuf, u32>,
    frame_preset_id: Uuid,
    canvas_preset_id: Uuid,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let event = state.store.load(event_id).tauri()?;

    let PreparedExport {
        photos,
        canvas_preset,
        frame_preset,
        frames,
        watermark,
        slot_w,
        slot_h,
    } = prepare_export(
        &event,
        &quantities,
        frame_preset_id,
        canvas_preset_id,
        0,
        state.watermark(),
    )?;

    let output_root = event
        .output_folder
        .as_ref()
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
        let total_canvases = if photos.is_empty() {
            0
        } else {
            photos.len().div_ceil(chunk_size)
        };
        let done = std::sync::atomic::AtomicUsize::new(0);

        let process_chunk = |(canvas_idx, chunk): (usize, &[Photo])| -> Vec<String> {
            let mut errs: Vec<String> = Vec::new();
            let framed: Vec<_> = chunk
                .iter()
                .filter_map(|photo| {
                    crate::photo::compose::frame_photo_for_canvas(
                        photo,
                        &frame_preset,
                        slot_w,
                        slot_h,
                        &frames,
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
                errs.push(format!(
                    "canvas {}: all photos failed to frame",
                    canvas_idx + 1
                ));
            } else {
                let mut canvas = crate::canvas::compositor::compose_one(&framed, &canvas_preset);
                if watermark {
                    canvas = crate::canvas::compositor::apply_watermark(&canvas);
                }
                let out_path = output_dir_clone.join(&filename);
                if let Err(e) = crate::photo::encode::write_print_ready(&canvas, &out_path) {
                    errs.push(format!("{filename}: {e}"));
                }
            }

            let d = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let _ = app.emit(
                crate::constants::events::SAVE_PROGRESS,
                SaveProgress {
                    done: d,
                    total: total_canvases,
                    current_file: filename,
                },
            );
            errs
        };

        let errors: Vec<String> = run_bounded(|| {
            photos
                .par_chunks(chunk_size)
                .enumerate()
                .flat_map_iter(process_chunk)
                .collect()
        });

        // Increment save_count for all saved photos. Flush immediately —
        // billing-relevant counts must survive a crash, not wait for the
        // coalesced 1s flush.
        if let Ok(mut evt) = store.load(event_id) {
            bump_counts(&mut evt, &quantities, |p| &mut p.save_count);
            let _ = store.save(&evt);
            let _ = store.flush_one(event_id);
        }

        let _ = app.emit(
            crate::constants::events::SAVE_COMPLETE,
            SaveComplete {
                errors,
                output_dir: output_dir_clone.to_string_lossy().into_owned(),
            },
        );
    });

    Ok(())
}

#[tauri::command]
pub async fn print_photos(
    event_id: Uuid,
    quantities: HashMap<PathBuf, u32>,
    frame_preset_id: Uuid,
    canvas_preset_id: Uuid,
    state: State<'_, AppState>,
) -> Result<PrintResult, String> {
    let mut event = state.store.load(event_id).tauri()?;
    let PreparedExport {
        photos,
        canvas_preset,
        frame_preset,
        frames,
        watermark,
        slot_w,
        slot_h,
    } = prepare_export(
        &event,
        &quantities,
        frame_preset_id,
        canvas_preset_id,
        1,
        state.watermark(),
    )?;

    let framed: Vec<_> = run_bounded(|| {
        photos
            .par_iter()
            .filter_map(|p| {
                crate::photo::compose::frame_photo_for_canvas(
                    p,
                    &frame_preset,
                    slot_w,
                    slot_h,
                    &frames,
                )
                .ok()
            })
            .collect()
    });

    let canvases: Vec<_> = crate::canvas::compositor::compose_canvases(&framed, &canvas_preset)
        .into_iter()
        .map(|c| {
            if watermark {
                crate::canvas::compositor::apply_watermark(&c)
            } else {
                c
            }
        })
        .collect();

    let tmp_dir = std::env::temp_dir().join("orenew_print");
    std::fs::create_dir_all(&tmp_dir).tauri()?;

    let mut paths: Vec<PathBuf> = Vec::new();
    for (i, canvas) in canvases.iter().enumerate() {
        let p = tmp_dir.join(format!("print_{i:04}.jpg"));
        crate::photo::encode::write_print_ready(canvas, &p).tauri()?;
        paths.push(p);
    }

    // Windows: launch the native print dialog (Photos print wizard) per canvas.
    // The filenames are app-generated (`print_NNNN.jpg`), so the single-quoted
    // PowerShell argument needs no further escaping.
    #[cfg(windows)]
    for p in &paths {
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command"])
            .arg(format!(
                "Start-Process -FilePath '{}' -Verb Print",
                p.display()
            ))
            .spawn();
    }

    bump_counts(&mut event, &quantities, |p| &mut p.print_count);
    state.store.save(&event).tauri()?;
    // Flush immediately — billing-relevant counts must survive a crash.
    // ponytail: print_count is optimistic — we can't detect the user cancelling
    // the OS print dialog. Accurate counts would need per-platform spooler
    // polling; add that only if customers dispute print billing.
    state.store.flush_one(event_id).tauri()?;

    Ok(PrintResult {
        count: paths.len(),
        dialog_opened: cfg!(windows),
        output_dir: tmp_dir.to_string_lossy().into_owned(),
    })
}
