use crate::commands::IntoTauri;
use crate::project::model::{Event, Photo, PhotoBatch};
use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;
use uuid::Uuid;

/// A folder in the event's filesystem tree, with the count of image files
/// directly inside it (not counting subfolders). Drives the Lightroom-style
/// sidebar.
#[derive(Serialize)]
pub struct FolderNode {
    name: String,
    path: PathBuf,
    photo_count: usize,
    children: Vec<FolderNode>,
}

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

/// List the event's folder tree under `root`, with per-folder direct image
/// counts, for the sidebar. Read-only; does not touch the event store.
#[tauri::command]
pub async fn list_folder_tree(root: PathBuf) -> Result<FolderNode, String> {
    build_folder_node(&root).tauri()
}

/// Select a folder in the tree: find-or-create its batch (lazy), re-scanning it
/// so new/changed photos show up. Idempotent — clicking the same folder again
/// just refreshes it. The frontend picks the active batch by matching
/// `source_path == folder` in the returned event.
#[tauri::command]
pub async fn select_folder(
    event_id: Uuid,
    folder: PathBuf,
    state: State<'_, AppState>,
) -> Result<Event, String> {
    let mut event = state.store.load(event_id).tauri()?;
    let fresh = scan_folder(&folder)?;

    if let Some(batch) = event.batches.iter_mut().find(|b| b.source_path == folder) {
        let old = std::mem::take(&mut batch.photos);
        let (photos, changed_ids) = merge_photos(old, fresh);
        batch.photos = photos;
        state.store.save(&event).tauri()?;
        for id in changed_ids {
            state.invalidate_preview_for_photo(id);
        }
    } else {
        let batch_name = folder
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut batch = PhotoBatch::new(batch_name, folder.clone());
        batch.photos = fresh;
        event.batches.push(batch);
        state.store.save(&event).tauri()?;
    }
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
        // One recursive watch on the event root covers every (current and future)
        // folder the photographer browses to — new SD dumps appear automatically.
        // ponytail: recursive watch on the event root; per-folder watches if it proves heavy
        if let Some(root) = &event.root_path {
            let _ = watcher.watch_recursive(root);
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

/// Recursively build a folder tree. `photo_count` is the number of supported
/// image files directly in the folder (no decode — extension check only).
/// Skips hidden/dot directories. Children sorted by name.
fn build_folder_node(path: &std::path::Path) -> std::io::Result<FolderNode> {
    let mut photo_count = 0;
    let mut children = Vec::new();
    for entry in std::fs::read_dir(path)?.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let hidden = p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'));
            if !hidden {
                if let Ok(node) = build_folder_node(&p) {
                    children.push(node);
                }
            }
        } else if p.is_file() && crate::photo::loader::is_supported_image(&p) {
            photo_count += 1;
        }
    }
    children.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(FolderNode {
        name: path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned(),
        path: path.to_path_buf(),
        photo_count,
        children,
    })
}

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
    let mut existing_map: HashMap<PathBuf, Photo> =
        existing.into_iter().map(|p| (p.path.clone(), p)).collect();

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
        assert_eq!(
            merged[0].print_count, 0,
            "content change resets print_count"
        );
        assert_eq!(changed, vec![id]);
    }

    #[test]
    fn folder_tree_counts_direct_images_and_recurses() {
        let dir = std::env::temp_dir().join(format!("orenew_tree_{}", Uuid::new_v4()));
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("a.jpg"), b"x").unwrap();
        std::fs::write(dir.join("b.png"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap(); // ignored
        std::fs::write(sub.join("c.jpeg"), b"x").unwrap();
        std::fs::create_dir_all(dir.join(".hidden")).unwrap(); // skipped

        let node = build_folder_node(&dir).unwrap();
        assert_eq!(node.photo_count, 2, "direct images only, txt excluded");
        assert_eq!(node.children.len(), 1, "hidden dir skipped");
        assert_eq!(node.children[0].photo_count, 1);

        std::fs::remove_dir_all(&dir).ok();
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
