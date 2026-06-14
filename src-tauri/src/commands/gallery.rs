use tauri::State;
use uuid::Uuid;
use crate::commands::IntoTauri;
use crate::project::model::Orientation;
use crate::AppState;

#[tauri::command]
pub async fn get_thumbnail(
    photo_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    let path = std::path::PathBuf::from(&photo_path);
    state.thumbs.get_or_generate(&path).tauri()
}

#[tauri::command]
pub async fn get_framed_preview(
    event_id: Uuid,
    photo_id: Uuid,
    preset_id: Uuid,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    // Check in-memory cache first.
    if let Some(cached) = state.preview_cache.lock().unwrap().get(&(photo_id, preset_id)).cloned() {
        return Ok(cached);
    }
    let event = state.store.load(event_id).tauri()?;
    let photo = event.find_photo(photo_id)?;
    let preset = event.find_frame_preset(preset_id)?;
    let bytes = crate::preview::framed_preview::generate_framed_preview(photo, preset)
        .tauri()?;
    state.preview_cache.lock().unwrap().insert((photo_id, preset_id), bytes.clone());
    Ok(bytes)
}

/// Clear cached previews for a specific frame preset, or all if `preset_id` is None.
/// Called by the frontend when a frame PNG changes on disk.
#[tauri::command]
pub async fn clear_framed_preview_cache(
    preset_id: Option<Uuid>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    match preset_id {
        Some(pid) => state.invalidate_preview_for_preset(pid),
        None => state.preview_cache.lock().unwrap().clear(),
    }
    Ok(())
}

#[tauri::command]
pub async fn set_orientation_override(
    event_id: Uuid,
    photo_id: Uuid,
    orientation: Orientation,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut event = state.store.load(event_id).tauri()?;
    event.find_photo_mut(photo_id)?.orientation_override = Some(orientation);
    state.store.save(&event).tauri()?;
    // Invalidate cached previews for this photo across all presets.
    state.invalidate_preview_for_photo(photo_id);
    Ok(())
}

#[tauri::command]
pub async fn clear_orientation_override(
    event_id: Uuid,
    photo_id: Uuid,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut event = state.store.load(event_id).tauri()?;
    event.find_photo_mut(photo_id)?.orientation_override = None;
    state.store.save(&event).tauri()?;
    state.invalidate_preview_for_photo(photo_id);
    Ok(())
}
