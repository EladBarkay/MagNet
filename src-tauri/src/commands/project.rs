use std::path::PathBuf;
use tauri::State;
use uuid::Uuid;
use crate::project::model::{Event, PhotoBatch};
use crate::AppState;

#[tauri::command]
pub async fn list_events(state: State<'_, AppState>) -> Result<Vec<Event>, String> {
    state.store.list_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_event(path: PathBuf, state: State<'_, AppState>) -> Result<Event, String> {
    // Check for existing event matching this source path
    if let Some(event) = state.store.find_by_source_path(&path).map_err(|e| e.to_string())? {
        return Ok(event);
    }
    // New event: scan folder for photos and create record
    let folder_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut event = Event::new(folder_name.clone());
    let mut batch = crate::project::model::PhotoBatch::new(folder_name, path.clone());
    batch.photos = scan_folder(&path)?;
    event.batches.push(batch);
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
pub async fn set_output_folder(
    event_id: uuid::Uuid,
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
    Ok(event)
}

fn scan_folder(path: &std::path::Path) -> Result<Vec<crate::project::model::Photo>, String> {
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
