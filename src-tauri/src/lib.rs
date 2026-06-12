mod commands;
mod photo;
mod project;
mod preview;
mod canvas;
mod watcher;
mod license;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use project::persistence::EventStore;
use preview::thumbnail::ThumbnailCache;
use watcher::fs_watcher::FsWatcher;
use license::validator::{LicenseInfo, Tier};

pub struct AppState {
    pub store: EventStore,
    pub thumbs: ThumbnailCache,
    pub app_data_dir: PathBuf,
    pub watcher: Mutex<FsWatcher>,
    /// Currently active license, if any. `None` => Free tier.
    pub license: Mutex<Option<LicenseInfo>>,
}

impl AppState {
    /// Effective tier — Free unless a non-expired Pro license is active.
    pub fn tier(&self) -> Tier {
        self.license
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|l| l.tier.clone()))
            .unwrap_or(Tier::Free)
    }

    /// Whether output canvases should be watermarked (Free tier only).
    pub fn watermark(&self) -> bool {
        matches!(self.tier(), Tier::Free)
    }

    /// Load license.json from disk into a validated, non-expired LicenseInfo.
    pub fn load_license(app_data_dir: &Path) -> Option<LicenseInfo> {
        let path = app_data_dir.join("license.json");
        let data = std::fs::read_to_string(path).ok()?;
        let info: LicenseInfo = serde_json::from_str(&data).ok()?;
        if info.expiry < chrono::Local::now().date_naive() {
            return None;
        }
        Some(info)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            use tauri::{Emitter, Manager};
            let data_dir = app.path().app_data_dir().expect("app_data_dir");
            let store = EventStore::new(data_dir.join("events"))
                .expect("EventStore init");
            let thumbs = ThumbnailCache::new(data_dir.join("thumbs"))
                .expect("ThumbnailCache init");

            let app_handle = app.handle().clone();
            let fs_watcher = FsWatcher::new(move |path: PathBuf| {
                let _ = app_handle.emit("fs-changed", path.to_string_lossy().to_string());
            }).expect("FsWatcher init");

            let license = AppState::load_license(&data_dir);

            app.manage(AppState {
                store,
                thumbs,
                app_data_dir: data_dir,
                watcher: Mutex::new(fs_watcher),
                license: Mutex::new(license),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::project::open_event,
            commands::project::save_event,
            commands::project::set_output_folder,
            commands::project::add_batch,
            commands::project::delete_event,
            commands::project::delete_batch,
            commands::project::refresh_batch,
            commands::project::sync_watches,
            commands::gallery::get_thumbnail,
            commands::gallery::get_framed_preview,
            commands::gallery::set_orientation_override,
            commands::gallery::set_crop_override,
            commands::batch::export_batch,
            commands::batch::print_photos,
            commands::canvas_preset::create_canvas_preset,
            commands::canvas_preset::update_canvas_preset,
            commands::canvas_preset::delete_canvas_preset,
            commands::frame_preset::create_frame_preset,
            commands::frame_preset::update_frame_preset,
            commands::frame_preset::delete_frame_preset,
            commands::license::validate_license,
            commands::license::get_license_info,
            commands::license::clear_license,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
