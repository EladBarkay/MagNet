use std::collections::HashSet;
use std::path::{Path, PathBuf};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use anyhow::Result;

/// Watches directories and calls `on_folder_changed(parent_dir)` whenever a file
/// inside is created, modified, or deleted. Deduplicates to one call per folder per event.
pub struct FsWatcher {
    watcher: RecommendedWatcher,
}

impl FsWatcher {
    pub fn new<F>(on_folder_changed: F) -> Result<Self>
    where
        F: Fn(PathBuf) + Send + 'static,
    {
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let mut folders = HashSet::new();
                for path in event.paths {
                    if let Some(parent) = path.parent() {
                        folders.insert(parent.to_path_buf());
                    }
                }
                for folder in folders {
                    on_folder_changed(folder);
                }
            }
        })?;
        Ok(Self { watcher })
    }

    pub fn watch(&mut self, path: &Path) -> Result<()> {
        self.watcher.watch(path, RecursiveMode::NonRecursive)?;
        Ok(())
    }

    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        let _ = self.watcher.unwatch(path);
        Ok(())
    }
}
