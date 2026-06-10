use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;
use uuid::Uuid;
use crate::project::model::{Event, Photo, PhotoBatch};
use crate::AppState;

#[tauri::command]
pub async fn list_events(state: State<'_, AppState>) -> Result<Vec<Event>, String> {
    state.store.list_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_event(path: PathBuf, state: State<'_, AppState>) -> Result<Event, String> {
    // Resume by root_path first, then fall back to legacy batch-path lookup
    if let Some(event) = state.store.find_by_root_path(&path).map_err(|e| e.to_string())? {
        return Ok(event);
    }
    if let Some(event) = state.store.find_by_source_path(&path).map_err(|e| e.to_string())? {
        return Ok(event);
    }
    // New event — create record only, no auto-batch.
    // The user adds batches manually via the "+ Add" button.
    let folder_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut event = Event::new(folder_name);
    event.root_path = Some(path);
    state.store.save(&event).map_err(|e| e.to_string())?;
    Ok(event)
}

#[tauri::command]
pub async fn create_event(name: String, state: State<'_, AppState>) -> Result<Event, String> {
    let event = Event::new(name);
    state.store.save(&event).map_err(|e| e.to_string())?;
    Ok(event)
}

#[tauri::command]
pub async fn save_event(event: Event, state: State<'_, AppState>) -> Result<(), String> {
    state.store.save(&event).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_event(event_id: Uuid, state: State<'_, AppState>) -> Result<(), String> {
    state.store.delete(event_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_output_folder(
    event_id: Uuid,
    folder: PathBuf,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    event.output_folder = Some(folder);
    state.store.save(&event).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_batch(
    event_id: Uuid,
    folder: PathBuf,
    state: State<'_, AppState>,
) -> Result<Event, String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    let batch_name = folder
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut batch = PhotoBatch::new(batch_name, folder.clone());
    batch.photos = scan_folder(&folder)?;
    event.batches.push(batch);
    state.store.save(&event).map_err(|e| e.to_string())?;

    // Start watching the new batch folder for changes
    if let Ok(mut watcher) = state.watcher.lock() {
        let _ = watcher.watch(&folder);
    }

    Ok(event)
}

#[tauri::command]
pub async fn refresh_batch(
    event_id: Uuid,
    batch_id: Uuid,
    state: State<'_, AppState>,
) -> Result<Event, String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    let batch = event
        .batches
        .iter_mut()
        .find(|b| b.id == batch_id)
        .ok_or_else(|| format!("batch {batch_id} not found"))?;

    let source_path = batch.source_path.clone();
    let fresh = scan_folder(&source_path)?;
    let old = std::mem::take(&mut batch.photos);
    batch.photos = merge_photos(old, fresh);

    state.store.save(&event).map_err(|e| e.to_string())?;
    Ok(event)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn scan_folder(path: &std::path::Path) -> Result<Vec<Photo>, String> {
    let entries = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    let mut photos = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() && crate::photo::loader::is_supported_image(&p) {
            match crate::photo::loader::scan_photo(p) {
                Ok(photo) => photos.push(photo),
                Err(e) => log::warn!("skipping {}: {e}", entry.path().display()),
            }
        }
    }
    photos.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(photos)
}

/// Merge a re-scanned photo list into the existing batch, preserving user data.
/// - Same path + same hash → keep existing (print_count, overrides)
/// - Same path + changed hash → reset print_count, keep orientation/crop overrides
/// - New path → add
/// - Path no longer present → drop
fn merge_photos(existing: Vec<Photo>, scanned: Vec<Photo>) -> Vec<Photo> {
    let mut existing_map: HashMap<PathBuf, Photo> = existing
        .into_iter()
        .map(|p| (p.path.clone(), p))
        .collect();

    scanned
        .into_iter()
        .map(|new_p| match existing_map.remove(&new_p.path) {
            Some(old) if old.content_hash == new_p.content_hash => old,
            Some(old) => Photo {
                orientation_override: old.orientation_override,
                crop_override: old.crop_override,
                print_count: 0,
                ..new_p
            },
            None => new_p,
        })
        .collect()
}
