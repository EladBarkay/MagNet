mod commands;
mod photo;
mod project;
mod preview;
mod canvas;
mod watcher;
mod license;

use std::path::PathBuf;
use std::sync::Mutex;
use project::persistence::EventStore;
use preview::thumbnail::ThumbnailCache;
use watcher::fs_watcher::FsWatcher;

pub struct AppState {
    pub store: EventStore,
    pub thumbs: ThumbnailCache,
    pub app_data_dir: PathBuf,
    pub watcher: Mutex<FsWatcher>,
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
            let fs_watcher = FsWatcher::new(move |folder: PathBuf| {
                let _ = app_handle.emit("folder-changed", folder.to_string_lossy().to_string());
            }).expect("FsWatcher init");

            app.manage(AppState {
                store,
                thumbs,
                app_data_dir: data_dir,
                watcher: Mutex::new(fs_watcher),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::project::open_event,
            commands::project::create_event,
            commands::project::save_event,
            commands::project::list_events,
            commands::project::set_output_folder,
            commands::project::add_batch,
            commands::project::delete_event,
            commands::project::refresh_batch,
            commands::gallery::list_photos,
            commands::gallery::get_thumbnail,
            commands::gallery::get_framed_preview,
            commands::gallery::set_orientation_override,
            commands::gallery::set_crop_override,
            commands::batch::export_batch,
            commands::batch::print_photos,
            commands::canvas_preset::list_canvas_presets,
            commands::canvas_preset::create_canvas_preset,
            commands::canvas_preset::delete_canvas_preset,
            commands::frame_preset::list_frame_presets,
            commands::frame_preset::create_frame_preset,
            commands::frame_preset::delete_frame_preset,
            commands::license::validate_license,
            commands::license::get_license_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
