use crate::json_store::{load_json, save_json};
use crate::project::model::Event;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Manages reading/writing event state in `{app_data}/events/`.
/// Constructed once at app startup from `app.path().app_data_dir()` and held in Tauri state.
///
/// The in-memory `cache` is the source of truth. `save` updates the cache and
/// marks the event dirty; the actual disk write is **coalesced** — a background
/// task flushes dirty events on a short interval, and the window-close hook
/// flushes synchronously. This avoids rewriting the whole multi-MB `orenew.json`
/// on every single mutation (each mutation becomes an in-memory map insert).
///
/// ponytail: JSON + coalesced writes is enough for one user / a few thousand
/// photos per event. Revisit SQLite only when a real event exceeds ~10k photos
/// with perceptible lag *after* this, or a cross-event feature (analytics /
/// global search / undo log) is actually scheduled.
#[derive(Clone)]
pub struct EventStore {
    base_dir: PathBuf,
    cache: Arc<Mutex<HashMap<Uuid, Event>>>,
    dirty: Arc<Mutex<HashSet<Uuid>>>,
}

impl EventStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir)?;
        Ok(Self {
            base_dir,
            cache: Arc::new(Mutex::new(HashMap::new())),
            dirty: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    fn event_path(&self, id: Uuid) -> PathBuf {
        self.base_dir.join(id.to_string()).join("orenew.json")
    }

    pub fn load(&self, id: Uuid) -> Result<Event> {
        if let Some(event) = self.cache.lock().unwrap().get(&id).cloned() {
            return Ok(event);
        }
        let path = self.event_path(id);
        let event: Event = load_json(&path)?;
        self.cache.lock().unwrap().insert(id, event.clone());
        Ok(event)
    }

    /// Update the cache and mark the event dirty. Does **not** write to disk —
    /// the flush task / close hook persists it. Use `flush_one` for writes that
    /// must be durable immediately (money-tracking count bumps).
    pub fn save(&self, event: &Event) -> Result<()> {
        self.cache.lock().unwrap().insert(event.id, event.clone());
        self.dirty.lock().unwrap().insert(event.id);
        Ok(())
    }

    /// Write the cached event to disk via the atomic `save_json` path. No dirty
    /// bookkeeping — callers own that (they differ on drain/re-mark policy).
    fn write_event(&self, id: Uuid) -> Result<()> {
        let event = self.cache.lock().unwrap().get(&id).cloned();
        if let Some(event) = event {
            save_json(&self.event_path(id), &event)?;
        }
        Ok(())
    }

    /// Write all dirty events to disk, clearing the dirty set. Called by the
    /// background flush task and the close hook.
    pub fn flush_dirty(&self) -> Result<()> {
        let ids: Vec<Uuid> = { self.dirty.lock().unwrap().drain().collect() };
        for id in ids {
            if let Err(e) = self.write_event(id) {
                // re-mark dirty so a transient write error retries next flush
                self.dirty.lock().unwrap().insert(id);
                return Err(e);
            }
        }
        Ok(())
    }

    /// Flush a single event to disk immediately (durability-critical writes).
    pub fn flush_one(&self, id: Uuid) -> Result<()> {
        self.write_event(id)?;
        self.dirty.lock().unwrap().remove(&id);
        Ok(())
    }

    pub fn list_all(&self) -> Result<Vec<Event>> {
        // ponytail: flush before scan so dirty events read their latest state;
        // cheap since list_all is cold-path only (open/create).
        self.flush_dirty()?;
        let mut events = Vec::new();
        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let json_path = entry.path().join("orenew.json");
            if json_path.exists() {
                match load_json::<Event>(&json_path).ok() {
                    Some(event) => events.push(event),
                    None => log::warn!("skipping malformed event at {}", json_path.display()),
                }
            }
        }
        Ok(events)
    }

    /// Find an existing event that holds a photo directly inside `folder` (legacy
    /// resume path; new events resume by `root_path`).
    pub fn find_by_source_path(&self, folder: &Path) -> Result<Option<Event>> {
        for event in self.list_all()? {
            if event.photos.keys().any(|p| p.parent() == Some(folder)) {
                return Ok(Some(event));
            }
        }
        Ok(None)
    }

    /// Find an existing event whose root_path matches `folder`.
    pub fn find_by_root_path(&self, folder: &Path) -> Result<Option<Event>> {
        for event in self.list_all()? {
            if event.root_path.as_deref() == Some(folder) {
                return Ok(Some(event));
            }
        }
        Ok(None)
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.cache.lock().unwrap().remove(&id);
        self.dirty.lock().unwrap().remove(&id);
        let dir = self.base_dir.join(id.to_string());
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::model::Event;

    fn tmp_store() -> (EventStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("orenew_test_{}", Uuid::new_v4()));
        (EventStore::new(dir.clone()).unwrap(), dir)
    }

    #[test]
    fn save_is_coalesced_flush_writes() {
        let (store, dir) = tmp_store();
        let event = Event::new("e".into());
        let path = store.event_path(event.id);

        // save() must NOT touch disk — it only caches + marks dirty.
        store.save(&event).unwrap();
        assert!(!path.exists(), "save() should not write to disk");
        assert!(store.load(event.id).is_ok(), "cache serves the event");

        // flush_dirty() persists it and clears the dirty set.
        store.flush_dirty().unwrap();
        assert!(path.exists(), "flush_dirty() should write to disk");
        assert!(store.dirty.lock().unwrap().is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn flush_one_writes_immediately_and_delete_clears_dirty() {
        let (store, dir) = tmp_store();
        let event = Event::new("e".into());
        let path = store.event_path(event.id);

        store.save(&event).unwrap();
        store.flush_one(event.id).unwrap();
        assert!(path.exists(), "flush_one() should write to disk");
        assert!(!store.dirty.lock().unwrap().contains(&event.id));

        // delete() drops the dirty mark so a deleted event isn't re-flushed.
        store.save(&event).unwrap();
        store.delete(event.id).unwrap();
        assert!(!store.dirty.lock().unwrap().contains(&event.id));
        store.flush_dirty().unwrap(); // no-op, must not recreate the file
        assert!(!path.exists());

        std::fs::remove_dir_all(&dir).ok();
    }
}
