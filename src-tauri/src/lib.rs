mod commands;
mod photo;
mod project;
mod preview;
mod canvas;
mod watcher;
mod auth;
mod json_store;
mod constants;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use project::persistence::EventStore;
use preview::thumbnail::ThumbnailCache;
use watcher::fs_watcher::FsWatcher;
use auth::entitlement::{Entitlement, Tier};
use auth::session::Session;
use auth::AuthState;
use uuid::Uuid;

pub struct AppState {
    pub store: EventStore,
    pub thumbs: ThumbnailCache,
    pub app_data_dir: PathBuf,
    pub watcher: Mutex<FsWatcher>,
    /// Current auth state (session + entitlement). `None` => signed out / Free.
    pub auth: Mutex<Option<AuthState>>,
    /// Framed preview cache keyed by (photo_id, preset_id).
    pub preview_cache: Arc<Mutex<HashMap<(Uuid, Uuid), Vec<u8>>>>,
}

impl AppState {
    /// Effective tier — Free unless a valid Pro/Studio entitlement is active.
    pub fn tier(&self) -> Tier {
        self.auth
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|a| a.entitlement.effective_tier()))
            .unwrap_or(Tier::Free)
    }

    /// Whether output canvases should be watermarked (Free tier only).
    pub fn watermark(&self) -> bool {
        matches!(self.tier(), Tier::Free)
    }

    /// Drop cached framed previews that use the given frame preset.
    pub fn invalidate_preview_for_preset(&self, preset_id: Uuid) {
        self.preview_cache.lock().unwrap().retain(|(_, fpid), _| *fpid != preset_id);
    }

    /// Drop cached framed previews for the given photo across all presets.
    pub fn invalidate_preview_for_photo(&self, photo_id: Uuid) {
        self.preview_cache.lock().unwrap().retain(|(pid, _), _| *pid != photo_id);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ponytail: WebView2 (Windows) eats trackpad pinch for its own content zoom,
    // so the gesture never reaches the page. `--disable-pinch` turns that off so
    // Chromium forwards pinch as wheel+ctrl events, which the gallery already maps
    // to thumbnail size. Set before any webview is created. Windows-only; no effect
    // on the WebKitGTK/macOS builds. Needs on-device verification.
    #[cfg(target_os = "windows")]
    if std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_none() {
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--disable-pinch");
    }

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();
    // Single-instance must be the FIRST plugin. With the deep-link feature it
    // routes the magnetapp:// OAuth callback to the already-running app (and
    // focuses it) instead of Windows launching a second instance/window.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }));
    }
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            use tauri::{Emitter, Manager};

            let data_dir = app.path().app_data_dir().expect("app_data_dir");
            std::fs::create_dir_all(&data_dir).expect("create app_data_dir");

            let store = EventStore::new(data_dir.join("events"))
                .expect("EventStore init");
            // Clones for the coalesced-write flush task + the window-close hook.
            // EventStore is Arc-backed, so these share the same cache/dirty set.
            let store_flush = store.clone();
            let store_close = store.clone();
            let thumbs = ThumbnailCache::new(data_dir.join("thumbs"))
                .expect("ThumbnailCache init");

            let app_handle = app.handle().clone();
            let fs_watcher = FsWatcher::new(move |path: PathBuf| {
                let _ = app_handle.emit(constants::events::FS_CHANGED, path.to_string_lossy().to_string());
            }).expect("FsWatcher init");

            // Load the cached session, then offline-verify the cached entitlement
            // token (signature + device + grace) to seed the tier. This is only a
            // fallback for the offline-launch case — the refresh loop below
            // re-validates online first and overwrites it. An invalid/expired/
            // wrong-device token yields Free until that online check succeeds.
            let initial_auth = json_store::load_json::<Session>(&data_dir.join("session.json"))
                .ok()
                .map(|session| {
                    let entitlement = auth::provision::load_cached(&data_dir)
                        .unwrap_or_else(Entitlement::free);
                    AuthState { session, entitlement }
                });

            app.manage(AppState {
                store,
                thumbs,
                app_data_dir: data_dir.clone(),
                watcher: Mutex::new(fs_watcher),
                auth: Mutex::new(initial_auth),
                preview_cache: Arc::new(Mutex::new(HashMap::new())),
            });

            // Register the magnetapp:// scheme with the OS at runtime so the
            // OAuth callback deep link reaches the app on Windows/Linux dev and
            // unpackaged builds (installers register it, but dev runs don't).
            #[cfg(desktop)]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.deep_link().register_all();
            }

            // Background task: refresh the session on startup, then retry every
            // 60s until the server responds or there's no session to refresh.
            let app_handle_bg = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                auth_refresh_loop(app_handle_bg).await;
            });

            // Background task: flush coalesced event writes to disk every 1s.
            // ponytail: 1s poll, swap to notify/condvar only if it ever shows up
            // in a profile.
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    if let Err(e) = store_flush.flush_dirty() {
                        log::warn!("event flush failed: {e:#}");
                    }
                }
            });

            // Durability backstop: flush the last edits synchronously when the
            // main window is closing, before the process exits.
            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        if let Err(e) = store_close.flush_dirty() {
                            log::warn!("event flush on close failed: {e:#}");
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::project::open_event,
            commands::project::save_event,
            commands::project::set_photo_copies,
            commands::project::set_output_folder,
            commands::project::add_batch,
            commands::project::delete_event,
            commands::project::delete_batch,
            commands::project::refresh_batch,
            commands::project::sync_watches,
            commands::gallery::get_thumbnail,
            commands::gallery::get_frame_thumbnail,
            commands::gallery::get_framed_preview,
            commands::gallery::clear_framed_preview_cache,
            commands::gallery::set_orientation_override,
            commands::gallery::clear_orientation_override,
            commands::batch::save_batch,
            commands::batch::print_photos,
            commands::faces::count_faces_in_batch,
            commands::canvas_preset::create_canvas_preset,
            commands::canvas_preset::update_canvas_preset,
            commands::canvas_preset::delete_canvas_preset,
            commands::frame_preset::create_frame_preset,
            commands::frame_preset::update_frame_preset,
            commands::frame_preset::delete_frame_preset,
            commands::auth::establish_session,
            commands::auth::get_entitlement,
            commands::auth::refresh_entitlement,
            commands::auth::disconnect_device,
            commands::auth::list_devices,
            commands::auth::current_device_hash,
            commands::auth::sign_out,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Online-first re-validation at startup: refresh the session with Supabase and
/// re-mint a fresh device-bound entitlement token (which rewrites the last-online
/// timestamp and renews the 14-day grace).
/// - First attempt is immediate (handles startup-while-online case).
/// - Retries every 60s (handles launch-offline-then-connect case).
/// - Stops after a successful re-validation, an explicit rejection, or if there's
///   no session loaded. Until it succeeds, the cached token (offline-verified at
///   startup) governs the tier.
async fn auth_refresh_loop(app: tauri::AppHandle) {
    use tauri::{Emitter, Manager};

    let mut attempts: u32 = 0;

    loop {
        if attempts > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
        attempts += 1;

        let state = app.state::<AppState>();
        let data_dir = state.app_data_dir.clone();

        let existing = { state.auth.lock().ok().and_then(|g| g.clone()) };

        let Some(existing) = existing else {
            // Nothing signed in — nothing to refresh.
            break;
        };

        // 1. Refresh the access token.
        let session = match auth::client::refresh(&existing.session.refresh_token).await {
            Ok(s) => s,
            Err(_) => {
                // Can't reach the server. If the offline grace window has lapsed,
                // evict the session so the user is treated as Free. Check on every
                // failed attempt so a grace expiry mid-session is caught promptly.
                if !existing.entitlement.is_grace_period_valid() {
                    let _ = std::fs::remove_file(data_dir.join("session.json"));
                    auth::provision::clear_cached(&data_dir);
                    if let Ok(mut guard) = state.auth.lock() {
                        *guard = None;
                    }
                    let _ = app.emit(constants::events::LICENSE_EXPIRED, ());
                    break;
                }
                // Grace still valid — stay on cached tier; retry in 60s.
                continue;
            }
        };

        // Persist the new tokens immediately after step 1 so that a failure in
        // step 2 or 3 never leaves the old (now-consumed) refresh token on disk.
        let _ = json_store::save_json(&data_dir.join("session.json"), &session);
        if let Ok(mut guard) = state.auth.lock() {
            if let Some(a) = guard.as_mut() {
                a.session = session.clone();
            }
        }

        // 2. Verify the freshly minted access token.
        if auth::jwt::verify(&session.access_token).await.is_err() {
            continue; // JWKS unreachable etc. — keep cached tier, retry.
        }

        // 3. Re-mint + verify the device-bound entitlement token.
        match auth::provision::provision(&data_dir, &session.access_token).await {
            Ok(auth::provision::Provisioned::Active(entitlement)) => {
                if let Ok(mut guard) = state.auth.lock() {
                    *guard = Some(AuthState { session, entitlement });
                }
                let _ = app.emit(constants::events::TIER_CHANGED, ());
                break; // Done for this session.
            }
            Ok(auth::provision::Provisioned::DeviceLimit(devices)) => {
                // This device lost its seat (disconnected elsewhere). Drop to Free,
                // clear the cached token, and prompt the UI to re-select a device.
                auth::provision::clear_cached(&data_dir);
                if let Ok(mut guard) = state.auth.lock() {
                    *guard = Some(AuthState { session, entitlement: Entitlement::free() });
                }
                let _ = app.emit(constants::events::DEVICE_LIMIT, devices);
                break;
            }
            Err(_) => continue, // network blip minting/verifying — retry.
        }
    }
}
