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

/// Compile-time dev bypass: `MAGNET_DEV_TIER=pro` (or `studio`) seeds a synthetic
/// entitlement so the app runs at that tier without a real sign-in.
const DEV_TIER: Option<&str> = option_env!("MAGNET_DEV_TIER");

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

/// Build the synthetic auth state for the compile-time dev bypass, if configured.
fn dev_auth_state() -> Option<AuthState> {
    let tier = match DEV_TIER {
        Some("pro") => Tier::Pro,
        Some("studio") => Tier::Studio,
        _ => return None,
    };
    Some(AuthState {
        session: Session {
            access_token: String::new(),
            refresh_token: AuthState::DEV_REFRESH_TOKEN.to_string(),
            expires_at: 0,
            user_id: "dev".to_string(),
        },
        entitlement: Entitlement {
            email: Some("dev@magnet.app".to_string()),
            tier,
            expires_at: None,
            last_verified: chrono::Utc::now(),
        },
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            use tauri::{Emitter, Manager};

            let data_dir = app.path().app_data_dir().expect("app_data_dir");
            std::fs::create_dir_all(&data_dir).expect("create app_data_dir");

            let store = EventStore::new(data_dir.join("events"))
                .expect("EventStore init");
            let thumbs = ThumbnailCache::new(data_dir.join("thumbs"))
                .expect("ThumbnailCache init");

            let app_handle = app.handle().clone();
            let fs_watcher = FsWatcher::new(move |path: PathBuf| {
                let _ = app_handle.emit(constants::events::FS_CHANGED, path.to_string_lossy().to_string());
            }).expect("FsWatcher init");

            // Dev bypass takes precedence; otherwise load cached session + entitlement.
            let initial_auth = dev_auth_state().or_else(|| {
                let session = auth::session::load_cached(&data_dir.join("session.json"))?;
                let entitlement =
                    auth::entitlement::load_cached(&data_dir.join("entitlement.json"))
                        .unwrap_or_else(Entitlement::free);
                Some(AuthState { session, entitlement })
            });

            app.manage(AppState {
                store,
                thumbs,
                app_data_dir: data_dir.clone(),
                watcher: Mutex::new(fs_watcher),
                auth: Mutex::new(initial_auth),
                preview_cache: Arc::new(Mutex::new(HashMap::new())),
            });

            // Background task: refresh the session on startup, then retry every
            // 60s until the server responds or there's no session to refresh.
            let app_handle_bg = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                auth_refresh_loop(app_handle_bg).await;
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
            commands::gallery::clear_orientation_override,
            commands::batch::export_batch,
            commands::batch::print_photos,
            commands::canvas_preset::create_canvas_preset,
            commands::canvas_preset::update_canvas_preset,
            commands::canvas_preset::delete_canvas_preset,
            commands::frame_preset::create_frame_preset,
            commands::frame_preset::update_frame_preset,
            commands::frame_preset::delete_frame_preset,
            commands::project::open_in_explorer,
            commands::auth::establish_session,
            commands::auth::get_entitlement,
            commands::auth::sign_out,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Refreshes the cached session with Supabase and re-fetches the entitlement.
/// - First attempt is immediate (handles startup-while-online case).
/// - Retries every 60s (handles launch-offline-then-connect case).
/// - Stops after a successful refresh, an explicit rejection, or if there's no
///   session (or the dev bypass) loaded.
async fn auth_refresh_loop(app: tauri::AppHandle) {
    use tauri::{Emitter, Manager};
    use auth::entitlement::save_cached as save_entitlement;
    use auth::session::save_cached as save_session;

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

        // Dev bypass: never hits the network.
        if existing.is_dev() {
            break;
        }

        // 1. Refresh the access token.
        let session = match auth::client::refresh(&existing.session.refresh_token).await {
            Ok(s) => s,
            Err(_) => {
                // Unreachable or rejected. On the startup attempt, apply the
                // offline grace check; if it's lapsed, fall back to Free.
                if attempts == 1 && !existing.entitlement.is_grace_period_valid() {
                    let _ = std::fs::remove_file(data_dir.join("session.json"));
                    let _ = std::fs::remove_file(data_dir.join("entitlement.json"));
                    if let Ok(mut guard) = state.auth.lock() {
                        *guard = None;
                    }
                    let _ = app.emit(constants::events::LICENSE_EXPIRED, ());
                    break;
                }
                // Stay on cached tier; retry in 60s.
                continue;
            }
        };

        // 2. Verify the freshly minted access token.
        let claims = match auth::jwt::verify(&session.access_token).await {
            Ok(c) => c,
            Err(_) => continue, // JWKS unreachable etc. — keep cached tier, retry.
        };

        // 3. Re-fetch the entitlement.
        let entitlement =
            match auth::client::fetch_entitlement(&session.access_token, claims.email.clone()).await
            {
                Ok(e) => e,
                Err(_) => continue,
            };

        let _ = save_session(&data_dir.join("session.json"), &session);
        let _ = save_entitlement(&data_dir.join("entitlement.json"), &entitlement);
        if let Ok(mut guard) = state.auth.lock() {
            *guard = Some(AuthState { session, entitlement });
        }
        let _ = app.emit(constants::events::TIER_CHANGED, ());
        break; // Done for this session.
    }
}
