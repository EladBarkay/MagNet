use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;
use uuid::Uuid;
use crate::commands::IntoTauri;
use crate::project::model::{Event, Photo, PhotoBatch};
use crate::AppState;

#[tauri::command]
pub async fn open_event(path: PathBuf, state: State<'_, AppState>) -> Result<Event, String> {
    // Resume by root_path first, then fall back to legacy batch-path lookup
    if let Some(event) = state.store.find_by_root_path(&path).tauri()? {
        return Ok(event);
    }
    if let Some(event) = state.store.find_by_source_path(&path).tauri()? {
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
    state.store.save(&event).tauri()?;
    Ok(event)
}

#[tauri::command]
pub async fn save_event(event: Event, state: State<'_, AppState>) -> Result<(), String> {
    state.store.save(&event).tauri()
}

/// Persist per-photo queued copies. The map holds only photos with >0 copies; any
/// photo not in the map is set to 0 (the user zeroed it). Called debounced by the
/// frontend as the queue changes.
#[tauri::command]
pub async fn set_photo_copies(
    event_id: Uuid,
    copies: HashMap<Uuid, u32>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut event = state.store.load(event_id).tauri()?;
    for batch in &mut event.batches {
        for photo in &mut batch.photos {
            photo.copies = copies.get(&photo.id).copied().unwrap_or(0);
        }
    }
    state.store.save(&event).tauri()
}

#[tauri::command]
pub async fn delete_event(event_id: Uuid, state: State<'_, AppState>) -> Result<(), String> {
    state.store.delete(event_id).tauri()
}

#[tauri::command]
pub async fn set_output_folder(
    event_id: Uuid,
    folder: PathBuf,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut event = state.store.load(event_id).tauri()?;
    event.output_folder = Some(folder);
    state.store.save(&event).tauri()
}

#[tauri::command]
pub async fn add_batch(
    event_id: Uuid,
    folder: PathBuf,
    state: State<'_, AppState>,
) -> Result<Event, String> {
    let mut event = state.store.load(event_id).tauri()?;
    let batch_name = folder
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut batch = PhotoBatch::new(batch_name, folder.clone());
    batch.photos = scan_folder(&folder)?;
    event.batches.push(batch);
    state.store.save(&event).tauri()?;

    // Start watching the new batch folder for changes
    if let Ok(mut watcher) = state.watcher.lock() {
        let _ = watcher.watch(&folder);
    }

    Ok(event)
}

#[tauri::command]
pub async fn delete_batch(
    event_id: Uuid,
    batch_id: Uuid,
    state: State<'_, AppState>,
) -> Result<Event, String> {
    let mut event = state.store.load(event_id).tauri()?;
    if let Some(batch) = event.batches.iter().find(|b| b.id == batch_id) {
        let batch_path = batch.source_path.clone();
        if let Ok(mut watcher) = state.watcher.lock() {
            let _ = watcher.unwatch(&batch_path);
        }
    }
    event.batches.retain(|b| b.id != batch_id);
    state.store.save(&event).tauri()?;
    Ok(event)
}

#[tauri::command]
pub async fn refresh_batch(
    event_id: Uuid,
    batch_id: Uuid,
    state: State<'_, AppState>,
) -> Result<Event, String> {
    let mut event = state.store.load(event_id).tauri()?;
    let batch = event
        .batches
        .iter_mut()
        .find(|b| b.id == batch_id)
        .ok_or_else(|| format!("batch {batch_id} not found"))?;

    let source_path = batch.source_path.clone();
    let fresh = scan_folder(&source_path)?;
    let old = std::mem::take(&mut batch.photos);
    let (photos, changed_ids) = merge_photos(old, fresh);
    batch.photos = photos;

    state.store.save(&event).tauri()?;
    // A changed content hash means the on-disk pixels changed (e.g. rotated in
    // Explorer); drop the stale cached preview so it re-renders.
    for id in changed_ids {
        state.invalidate_preview_for_photo(id);
    }
    Ok(event)
}

/// (Re)establish filesystem watches for all of an event's batch folders and
/// frame-PNG directories. Safe to call repeatedly; call after opening an event
/// (existing watches are not persisted across restarts).
#[tauri::command]
pub async fn sync_watches(event_id: Uuid, state: State<'_, AppState>) -> Result<(), String> {
    let event = state.store.load(event_id).tauri()?;
    if let Ok(mut watcher) = state.watcher.lock() {
        for batch in &event.batches {
            let _ = watcher.watch(&batch.source_path);
        }
        for fp in &event.frame_presets {
            for p in [&fp.landscape_frame_path, &fp.portrait_frame_path] {
                if let Some(dir) = p.parent() {
                    let _ = watcher.watch(dir);
                }
            }
        }
    }
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn scan_folder(path: &std::path::Path) -> Result<Vec<Photo>, String> {
    let entries = std::fs::read_dir(path).tauri()?;
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
/// - Same path + changed hash → keep id + orientation override; reset print_count + crop_override
/// - New path → add
/// - Path no longer present → drop
///
/// The id is keyed to the path (preserved across content changes), so frontend
/// state keyed by photo id — the session copy-queue and the `selected` preview —
/// survives a file being edited/rotated on disk. Returns the merged list plus
/// the ids of photos whose content hash changed (callers invalidate their
/// cached previews).
fn merge_photos(existing: Vec<Photo>, scanned: Vec<Photo>) -> (Vec<Photo>, Vec<Uuid>) {
    let mut existing_map: HashMap<PathBuf, Photo> = existing
        .into_iter()
        .map(|p| (p.path.clone(), p))
        .collect();

    let mut changed_ids = Vec::new();
    let photos = scanned
        .into_iter()
        .map(|new_p| match existing_map.remove(&new_p.path) {
            // Unchanged content: keep user data, but refresh file metadata so
            // sorting works (and back-fills events saved before these fields existed).
            Some(old) if old.content_hash == new_p.content_hash => Photo {
                size_bytes: new_p.size_bytes,
                created: new_p.created,
                modified: new_p.modified,
                ..old
            },
            Some(old) => {
                changed_ids.push(old.id);
                Photo {
                    id: old.id,
                    orientation_override: old.orientation_override,
                    // crop_override stores pixel coordinates specific to the old image's
                    // dimensions; clearing it (via ..new_p) prevents out-of-bounds crops
                    // if the replacement photo has a different resolution.
                    print_count: 0,
                    // Queued copies are user intent — keep them across a content change.
                    copies: old.copies,
                    ..new_p
                }
            }
            None => new_p,
        })
        .collect();
    (photos, changed_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo(path: &str, hash: &str) -> Photo {
        Photo {
            id: Uuid::new_v4(),
            path: PathBuf::from(path),
            width: 100,
            height: 100,
            exif_orientation: None,
            orientation_override: None,
            crop_override: None,
            print_count: 5,
            save_count: 0,
            content_hash: hash.to_string(),
            copies: 1,
            size_bytes: 0,
            created: 0,
            modified: 0,
        }
    }

    #[test]
    fn changed_hash_keeps_id_resets_count_and_is_reported() {
        let old = photo("/a.jpg", "h1");
        let id = old.id;
        let scanned = vec![photo("/a.jpg", "h2")]; // same path, new hash + new uuid
        let (merged, changed) = merge_photos(vec![old], scanned);
        assert_eq!(merged[0].id, id, "id must follow the path, not the rescan");
        assert_eq!(merged[0].print_count, 0, "content change resets print_count");
        assert_eq!(changed, vec![id]);
    }

    #[test]
    fn same_hash_keeps_everything_and_reports_nothing() {
        let old = photo("/a.jpg", "h1");
        let id = old.id;
        let (merged, changed) = merge_photos(vec![old], vec![photo("/a.jpg", "h1")]);
        assert_eq!(merged[0].id, id);
        assert_eq!(merged[0].print_count, 5);
        assert!(changed.is_empty());
    }
}
