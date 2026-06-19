use tauri::State;
use uuid::Uuid;
use crate::commands::IntoTauri;
use crate::project::model::Orientation;
use crate::AppState;

#[tauri::command]
pub async fn get_thumbnail(
    photo_path: String,
    content_hash: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    let path = std::path::PathBuf::from(&photo_path);
    state.thumbs.get_or_generate(&path, content_hash.as_deref()).tauri()
}

/// Small PNG preview of a frame file (landscape/portrait), alpha preserved so the
/// transparent border shows. In-memory, no disk cache — only ever a couple of small
/// images open in the frame-preset dialog.
#[tauri::command]
pub async fn get_frame_thumbnail(path: String) -> Result<Vec<u8>, String> {
    use std::io::Cursor;
    let img = image::ImageReader::open(&path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(256, 256);
    let mut buf = Cursor::new(Vec::new());
    thumb
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}

#[tauri::command]
pub async fn get_framed_preview(
    event_id: Uuid,
    photo_id: Uuid,
    preset_id: Option<Uuid>,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    // `None` preset → raw full-photo preview; keyed under the nil UUID so it
    // shares the same cache without a separate map.
    let cache_key = (photo_id, preset_id.unwrap_or_else(Uuid::nil));
    if let Some(cached) = state.preview_cache.lock().unwrap().get(&cache_key).cloned() {
        return Ok(cached);
    }
    let event = state.store.load(event_id).tauri()?;
    let photo = event.find_photo(photo_id)?;
    let preset = match preset_id {
        Some(pid) => Some(event.find_frame_preset(pid)?),
        None => None,
    };
    let bytes = crate::preview::framed_preview::generate_framed_preview(photo, preset)
        .tauri()?;
    state.preview_cache.lock().unwrap().insert(cache_key, bytes.clone());
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
