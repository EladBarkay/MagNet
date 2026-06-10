use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use uuid::Uuid;
use crate::project::model::Event;

/// Manages reading/writing event state in `{app_data}/events/`.
/// Constructed once at app startup from `app.path().app_data_dir()` and held in Tauri state.
#[derive(Clone)]
pub struct EventStore {
    base_dir: PathBuf,
}

impl EventStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    fn event_path(&self, id: Uuid) -> PathBuf {
        self.base_dir.join(id.to_string()).join("magnet.json")
    }

    pub fn load(&self, id: Uuid) -> Result<Event> {
        let path = self.event_path(id);
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&data).context("deserializing event")
    }

    pub fn save(&self, event: &Event) -> Result<()> {
        let path = self.event_path(event.id);
        std::fs::create_dir_all(path.parent().unwrap())?;
        let data = serde_json::to_string_pretty(event).context("serializing event")?;
        std::fs::write(&path, data)
            .with_context(|| format!("writing {}", path.display()))
    }

    pub fn list_all(&self) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let json_path = entry.path().join("magnet.json");
            if json_path.exists() {
                match std::fs::read_to_string(&json_path)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                {
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
        let dir = self.base_dir.join(id.to_string());
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}
