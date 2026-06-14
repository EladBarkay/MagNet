use tauri::State;
use uuid::Uuid;
use crate::project::model::{CropRect, Orientation};
use crate::AppState;

#[tauri::command]
pub async fn get_thumbnail(
    photo_path: String,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    let path = std::path::PathBuf::from(&photo_path);
    state.thumbs.get_or_generate(&path).map_err(|e| e.to_string())
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
    let event = state.store.load(event_id).map_err(|e| e.to_string())?;
    let photo = find_photo(&event, photo_id)?;
    let preset = event
        .frame_presets
        .iter()
        .find(|p| p.id == preset_id)
        .ok_or_else(|| format!("preset {preset_id} not found"))?;
    let bytes = crate::preview::framed_preview::generate_framed_preview(photo, preset)
        .map_err(|e| e.to_string())?;
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
    let mut cache = state.preview_cache.lock().unwrap();
    match preset_id {
        Some(pid) => cache.retain(|(_, fpid), _| *fpid != pid),
        None => cache.clear(),
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
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    find_photo_mut(&mut event, photo_id)?.orientation_override = Some(orientation);
    state.store.save(&event).map_err(|e| e.to_string())?;
    // Invalidate cached previews for this photo across all presets.
    state.preview_cache.lock().unwrap().retain(|(pid, _), _| *pid != photo_id);
    Ok(())
}

#[tauri::command]
pub async fn set_crop_override(
    event_id: Uuid,
    photo_id: Uuid,
    crop: CropRect,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut event = state.store.load(event_id).map_err(|e| e.to_string())?;
    find_photo_mut(&mut event, photo_id)?.crop_override = Some(crop);
    state.store.save(&event).map_err(|e| e.to_string())?;
    // Invalidate cached previews for this photo across all presets.
    state.preview_cache.lock().unwrap().retain(|(pid, _), _| *pid != photo_id);
    Ok(())
}

fn find_photo(
    event: &crate::project::model::Event,
    photo_id: Uuid,
) -> Result<&crate::project::model::Photo, String> {
    event
        .batches
        .iter()
        .flat_map(|b| &b.photos)
        .find(|p| p.id == photo_id)
        .ok_or_else(|| format!("photo {photo_id} not found"))
}

fn find_photo_mut(
    event: &mut crate::project::model::Event,
    photo_id: Uuid,
) -> Result<&mut crate::project::model::Photo, String> {
    event
        .batches
        .iter_mut()
        .flat_map(|b| &mut b.photos)
        .find(|p| p.id == photo_id)
        .ok_or_else(|| format!("photo {photo_id} not found"))
}
