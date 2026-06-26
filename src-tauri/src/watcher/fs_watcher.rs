use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Watches directories and calls `on_changed(path)` for each file that is
/// created, modified, or deleted. Emits the full file path so the consumer can
/// distinguish batch photos from frame PNGs. Deduplicates per event.
pub struct FsWatcher {
    watcher: RecommendedWatcher,
}

impl FsWatcher {
    pub fn new<F>(on_changed: F) -> Result<Self>
    where
        F: Fn(PathBuf) + Send + 'static,
    {
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let mut seen = HashSet::new();
                for path in event.paths {
                    if seen.insert(path.clone()) {
                        on_changed(path);
                    }
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
