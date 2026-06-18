use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use anyhow::Result;
use uuid::Uuid;
use crate::json_store::{load_json, save_json};
use crate::project::model::Event;

/// Manages reading/writing event state in `{app_data}/events/`.
/// Constructed once at app startup from `app.path().app_data_dir()` and held in Tauri state.
///
/// Keeps an in-memory cache so hot paths (`list_photos`, `get_framed_preview`,
/// save/print) don't re-read and re-parse JSON from disk on every call. The
/// disk file remains the source of truth and is always written on `save`.
#[derive(Clone)]
pub struct EventStore {
    base_dir: PathBuf,
    cache: Arc<Mutex<HashMap<Uuid, Event>>>,
}

impl EventStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir, cache: Arc::new(Mutex::new(HashMap::new())) })
    }

    fn event_path(&self, id: Uuid) -> PathBuf {
        self.base_dir.join(id.to_string()).join("magnet.json")
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

    pub fn save(&self, event: &Event) -> Result<()> {
        save_json(&self.event_path(event.id), event)?;
        self.cache.lock().unwrap().insert(event.id, event.clone());
        Ok(())
    }

    pub fn list_all(&self) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let json_path = entry.path().join("magnet.json");
            if json_path.exists() {
                match load_json::<Event>(&json_path).ok() {
                    Some(event) => events.push(event),
                    None => log::warn!("skipping malformed event at {}", json_path.display()),
                }
            }
        }
        Ok(events)
    }

    /// Find an existing event whose batch source paths overlap with `folder`.
    pub fn find_by_source_path(&self, folder: &Path) -> Result<Option<Event>> {
        for event in self.list_all()? {
            if event.batches.iter().any(|b| b.source_path == folder) {
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
        let dir = self.base_dir.join(id.to_string());
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}
