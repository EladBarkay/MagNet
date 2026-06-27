use crate::commands::IntoTauri;
use crate::project::model::{Event, FolderEntry, Photo};
use crate::AppState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn open_event(path: PathBuf, state: State<'_, AppState>) -> Result<Event, String> {
    // Resume by root_path first, then fall back to a legacy source-folder lookup
    if let Some(event) = state.store.find_by_root_path(&path).tauri()? {
        return Ok(event);
    }
    if let Some(event) = state.store.find_by_source_path(&path).tauri()? {
        return Ok(event);
    }
    // New event — create the record only; folders are browsed from the sidebar tree.
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
    copies: HashMap<PathBuf, u32>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut event = state.store.load(event_id).tauri()?;
    for (path, photo) in &mut event.photos {
        photo.copies = copies.get(path).copied().unwrap_or(0);
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

/// List the immediate subfolders of `folder` (one level only — no recursion), for
/// the lazy sidebar tree. Each entry carries its direct image count and whether it
/// has subfolders (drives the expand chevron). Cheap: reads `folder` plus one
/// `read_dir` per child. Read-only.
#[tauri::command]
pub async fn list_folder(
    folder: PathBuf,
    state: State<'_, AppState>,
) -> Result<Vec<FolderEntry>, String> {
    let mut entries = Vec::new();
    for child in std::fs::read_dir(&folder).tauri()?.flatten() {
        let p = child.path();
        if !p.is_dir() {
            continue;
        }
        let hidden = p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if hidden {
            continue;
        }
        let (photo_count, has_subfolders) = folder_summary(&p);
        // Watch each folder we surface so changes inside it (new/edited photos)
        // are picked up — only on the paths the user is actually browsing.
        if let Ok(mut watcher) = state.watcher.lock() {
            let _ = watcher.watch(&p);
        }
        entries.push(FolderEntry {
            name: p
                .file_name()
                .unwrap_or(p.as_os_str())
                .to_string_lossy()
                .into_owned(),
            path: p,
            photo_count,
            has_subfolders,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Select a folder in the tree: scan its photos and merge them into the event's
/// path-keyed photo map (preserving overrides/counts/copies per file). Idempotent
/// — re-selecting just re-scans. Returns the updated event; the frontend derives
/// the folder's photos by filtering the map by parent path. Also used by the
/// watcher to refresh a folder whose contents changed.
#[tauri::command]
pub async fn select_folder(
    event_id: Uuid,
    folder: PathBuf,
    state: State<'_, AppState>,
) -> Result<Event, String> {
    let mut event = state.store.load(event_id).tauri()?;
    let fresh = scan_folder(&folder)?;
    let changed = merge_folder(&mut event, &folder, fresh);
    state.store.save(&event).tauri()?;
    if let Ok(mut watcher) = state.watcher.lock() {
        let _ = watcher.watch(&folder);
    }
    for path in changed {
        state.invalidate_preview_for_photo(&path);
    }
    Ok(event)
}

/// (Re)establish filesystem watches after opening an event (watches are not
/// persisted across restarts). Watches only the folders the user has already
/// browsed to — the distinct parent dirs of stored photos — plus the frame-PNG
/// directories. Non-recursive: selecting a deep folder under a huge root watches
/// just that folder, never the whole tree.
#[tauri::command]
pub async fn sync_watches(event_id: Uuid, state: State<'_, AppState>) -> Result<(), String> {
    let event = state.store.load(event_id).tauri()?;
    if let Ok(mut watcher) = state.watcher.lock() {
        let mut folders: std::collections::HashSet<&Path> = std::collections::HashSet::new();
        for path in event.photos.keys() {
            if let Some(dir) = path.parent() {
                folders.insert(dir);
            }
        }
        for dir in folders {
            let _ = watcher.watch(dir);
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

/// Count direct supported images in `folder` and whether it has any (non-hidden)
/// subfolder. One `read_dir`, no decode, no recursion.
fn folder_summary(folder: &Path) -> (usize, bool) {
    let mut photo_count = 0;
    let mut has_subfolders = false;
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let hidden = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'));
                has_subfolders |= !hidden;
            } else if p.is_file() && crate::photo::loader::is_supported_image(&p) {
                photo_count += 1;
            }
        }
    }
    (photo_count, has_subfolders)
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

/// Merge a folder's freshly-scanned photos into the event's path-keyed map,
/// preserving user data:
/// - Same path + same hash → keep stored entry, refresh file metadata
/// - Same path + changed hash → keep overrides + copies, reset print_count, take new dims/hash
/// - New path → insert
/// - Stored photo under `folder` no longer on disk → remove
///
/// Identity is the path, so frontend state keyed by path (the copy-queue, the
/// `selected` preview) survives a file being edited/rotated on disk. Returns the
/// paths whose content hash changed (callers invalidate their cached previews).
fn merge_folder(event: &mut Event, folder: &Path, scanned: Vec<Photo>) -> Vec<PathBuf> {
    let mut changed = Vec::new();
    let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    for new_p in scanned {
        seen.insert(new_p.path.clone());
        match event.photos.get(&new_p.path) {
            Some(old) if old.content_hash == new_p.content_hash => {
                let old = event.photos.get_mut(&new_p.path).unwrap();
                old.size_bytes = new_p.size_bytes;
                old.created = new_p.created;
                old.modified = new_p.modified;
            }
            Some(old) => {
                changed.push(new_p.path.clone());
                let merged = Photo {
                    orientation_override: old.orientation_override,
                    // crop_override stores pixel coords for the old image's dims;
                    // clearing it (via ..new_p) prevents out-of-bounds crops if the
                    // replacement has a different resolution.
                    print_count: 0,
                    // Queued copies are user intent — keep them across a content change.
                    copies: old.copies,
                    ..new_p
                };
                event.photos.insert(merged.path.clone(), merged);
            }
            None => {
                event.photos.insert(new_p.path.clone(), new_p);
            }
        }
    }

    // Drop photos that were directly in this folder but are gone from disk. Leave
    // photos in other folders untouched.
    event
        .photos
        .retain(|path, _| path.parent() != Some(folder) || seen.contains(path));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_with(photos: Vec<Photo>) -> Event {
        let mut e = Event::new("t".into());
        for p in photos {
            e.photos.insert(p.path.clone(), p);
        }
        e
    }

    fn photo(path: &str, hash: &str) -> Photo {
        Photo {
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
    fn changed_hash_resets_count_and_is_reported() {
        let mut event = event_with(vec![photo("/f/a.jpg", "h1")]);
        let changed = merge_folder(&mut event, Path::new("/f"), vec![photo("/f/a.jpg", "h2")]);
        let merged = &event.photos[Path::new("/f/a.jpg")];
        assert_eq!(merged.print_count, 0, "content change resets print_count");
        assert_eq!(merged.content_hash, "h2");
        assert_eq!(changed, vec![PathBuf::from("/f/a.jpg")]);
    }

    #[test]
    fn same_hash_keeps_everything_and_reports_nothing() {
        let mut event = event_with(vec![photo("/f/a.jpg", "h1")]);
        let changed = merge_folder(&mut event, Path::new("/f"), vec![photo("/f/a.jpg", "h1")]);
        assert_eq!(event.photos[Path::new("/f/a.jpg")].print_count, 5);
        assert!(changed.is_empty());
    }

    #[test]
    fn removed_file_dropped_other_folders_kept() {
        let mut event = event_with(vec![photo("/f/a.jpg", "h1"), photo("/g/b.jpg", "h1")]);
        // Re-scan /f with no files → a.jpg dropped, /g untouched.
        merge_folder(&mut event, Path::new("/f"), vec![]);
        assert!(!event.photos.contains_key(Path::new("/f/a.jpg")));
        assert!(event.photos.contains_key(Path::new("/g/b.jpg")));
    }

    #[test]
    fn folder_summary_counts_direct_images() {
        let dir = std::env::temp_dir().join(format!("orenew_sum_{}", Uuid::new_v4()));
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("a.jpg"), b"x").unwrap();
        std::fs::write(dir.join("b.png"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap(); // ignored
        std::fs::write(sub.join("c.jpeg"), b"x").unwrap();

        let (count, has_subs) = folder_summary(&dir);
        assert_eq!(count, 2, "direct images only, txt excluded");
        assert!(has_subs);
        assert_eq!(folder_summary(&sub), (1, false));

        std::fs::remove_dir_all(&dir).ok();
    }
}
