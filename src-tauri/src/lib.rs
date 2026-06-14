mod commands;
mod photo;
mod project;
mod preview;
mod canvas;
mod watcher;
mod license;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use project::persistence::EventStore;
use preview::thumbnail::ThumbnailCache;
use watcher::fs_watcher::FsWatcher;
use license::validator::{LicenseInfo, Tier};
use uuid::Uuid;

pub struct AppState {
    pub store: EventStore,
    pub thumbs: ThumbnailCache,
    pub app_data_dir: PathBuf,
    pub device_id: String,
    pub watcher: Mutex<FsWatcher>,
    /// Currently active license, if any. `None` => Free tier.
    pub license: Mutex<Option<LicenseInfo>>,
    /// Framed preview cache keyed by (photo_id, preset_id).
    pub preview_cache: Arc<Mutex<HashMap<(Uuid, Uuid), Vec<u8>>>>,
}

impl AppState {
    /// Effective tier — Free unless a valid Pro/Studio license is active.
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
            std::fs::create_dir_all(&data_dir).expect("create app_data_dir");

            let device_id = license::device::get_or_create(&data_dir)
                .expect("device ID init");

            let store = EventStore::new(data_dir.join("events"))
                .expect("EventStore init");
            let thumbs = ThumbnailCache::new(data_dir.join("thumbs"))
                .expect("ThumbnailCache init");

            let app_handle = app.handle().clone();
            let fs_watcher = FsWatcher::new(move |path: PathBuf| {
                let _ = app_handle.emit("fs-changed", path.to_string_lossy().to_string());
            }).expect("FsWatcher init");

            let license_path = data_dir.join("license.json");
            let cached = license::validator::load_cached(&license_path, &device_id);

            app.manage(AppState {
                store,
                thumbs,
                app_data_dir: data_dir.clone(),
                device_id: device_id.clone(),
                watcher: Mutex::new(fs_watcher),
                license: Mutex::new(cached),
                preview_cache: Arc::new(Mutex::new(HashMap::new())),
            });

            // Background task: revalidate on startup, then retry every 60s until
            // either the server responds or there's no license to revalidate.
            let app_handle_bg = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                revalidation_loop(app_handle_bg, license_path).await;
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
            commands::gallery::clear_framed_preview_cache,
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
            commands::license::activate_init,
            commands::license::activate_confirm,
            commands::license::get_license_info,
            commands::license::clear_license,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Attempts to revalidate the cached license with the server.
/// - First attempt is immediate (handles startup-while-online case).
/// - Retries every 60s (handles launch-offline-then-connect case).
/// - Stops after a server response (success or revoked) or if there's no license.
async fn revalidation_loop(app: tauri::AppHandle, license_path: PathBuf) {
    use tauri::{Emitter, Manager};
    use license::client::RevalidateResult;
    use license::validator::save_cached;

    let mut attempts: u32 = 0;

    loop {
        if attempts > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
        attempts += 1;

        let state = app.state::<AppState>();

        let existing = {
            state.license.lock().ok().and_then(|g| g.clone())
        };

        let Some(existing) = existing else {
            // No license loaded — nothing to revalidate.
            break;
        };

        match license::client::revalidate(&existing).await {
            RevalidateResult::Ok(updated) => {
                let _ = save_cached(&license_path, &updated);
                if let Ok(mut guard) = state.license.lock() {
                    *guard = Some(updated);
                }
                let _ = app.emit("tier-changed", ());
                break; // Done for this session.
            }
            RevalidateResult::Revoked => {
                let _ = std::fs::remove_file(&license_path);
                if let Ok(mut guard) = state.license.lock() {
                    *guard = None;
                }
                let _ = app.emit("tier-changed", ());
                break;
            }
            RevalidateResult::Unreachable => {
                if attempts == 1 {
                    // Startup attempt failed — apply grace period check.
                    if !existing.is_grace_period_valid() {
                        let _ = std::fs::remove_file(&license_path);
                        if let Ok(mut guard) = state.license.lock() {
                            *guard = None;
                        }
                        let _ = app.emit("license-expired", ());
                        break;
                    }
                }
                // Stay on cached tier; connectivity watcher retries in 60s.
            }
        }
    }
}
