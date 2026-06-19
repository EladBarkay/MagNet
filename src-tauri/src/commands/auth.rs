use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::auth::client::{self, DeviceInfo};
use crate::auth::entitlement::Entitlement;
use crate::auth::provision::{self, Provisioned};
use crate::auth::session::Session;
use crate::auth::{device, jwt, AuthState};
use crate::commands::IntoTauri;
use crate::AppState;

/// Outcome of an auth operation, so the frontend can branch between "signed in"
/// and "must pick a device to disconnect first".
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthResult {
    /// Signed in at the resolved entitlement.
    Entitlement(Entitlement),
    /// The subscription's device-seat limit is reached; show the picker.
    DeviceLimit { devices: Vec<DeviceInfo> },
}

/// Called by the frontend after an interactive Supabase sign-in. Rust verifies
/// the access token, registers this device + mints a signed entitlement token,
/// caches it, and upgrades the in-memory tier. If the subscription is already at
/// its device-seat limit, returns `DeviceLimit` (the caller then disconnects a
/// device and the sign-in completes via `disconnect_device`).
#[tauri::command]
pub async fn establish_session(
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    state: State<'_, AppState>,
) -> Result<AuthResult, String> {
    // Verify the token against Supabase JWKS — never trust the frontend's word.
    let claims = jwt::verify(&access_token).await.tauri()?;

    let session = Session {
        access_token: access_token.clone(),
        refresh_token,
        expires_at,
        user_id: claims.sub,
    };

    // Persist the session immediately so a `DeviceLimit` outcome can still drive
    // the subsequent `disconnect_device` + retry using this access token.
    let session_path = state.app_data_dir.join("session.json");
    crate::json_store::save_json(&session_path, &session).tauri()?;

    match provision::provision(&state.app_data_dir, &access_token).await.tauri()? {
        Provisioned::Active(mut entitlement) => {
            entitlement.email = entitlement.email.or(claims.email);
            set_auth(&state, session, entitlement.clone());
            Ok(AuthResult::Entitlement(entitlement))
        }
        Provisioned::DeviceLimit(devices) => {
            // Keep the (verified) session in memory at Free tier so the picker's
            // disconnect call has the access token; no token cached → no paid tier.
            set_auth(&state, session, Entitlement::free());
            Ok(AuthResult::DeviceLimit { devices })
        }
    }
}

/// Disconnect another device (freeing a seat), then register THIS device and
/// complete sign-in. Driven by the device picker when sign-in hit the seat limit.
#[tauri::command]
pub async fn disconnect_device(
    device_hash: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AuthResult, String> {
    let existing = current_auth(&state);
    let Some(existing) = existing else { return Err("not signed in".into()) };
    let access_token = existing.session.access_token.clone();

    client::disconnect_device(&access_token, &device_hash).await.tauri()?;

    match provision::provision(&state.app_data_dir, &access_token).await.tauri()? {
        Provisioned::Active(entitlement) => {
            set_auth(&state, existing.session, entitlement.clone());
            let _ = app.emit(crate::constants::events::TIER_CHANGED, ());
            Ok(AuthResult::Entitlement(entitlement))
        }
        // Still at the limit (e.g. the picked device wasn't actually freed) — let
        // the UI show the refreshed list.
        Provisioned::DeviceLimit(devices) => Ok(AuthResult::DeviceLimit { devices }),
    }
}

/// List the devices currently registered to the subscription, for the Settings
/// "manage devices" view. `None` => signed out.
#[tauri::command]
pub async fn list_devices(state: State<'_, AppState>) -> Result<Option<Vec<DeviceInfo>>, String> {
    let Some(existing) = current_auth(&state) else { return Ok(None) };
    let devices = client::list_devices(&existing.session.access_token).await.tauri()?;
    Ok(Some(devices))
}

/// The hash of the machine the app is running on, so the UI can mark "this
/// device" in the picker / device list.
#[tauri::command]
pub fn current_device_hash() -> String {
    device::device_hash()
}

/// Returns the currently cached entitlement (no network call), for UI bootstrap.
/// `None` => signed out / Free.
#[tauri::command]
pub async fn get_entitlement(state: State<'_, AppState>) -> Result<Option<Entitlement>, String> {
    Ok(current_auth(&state).map(|a| a.entitlement))
}

/// On-demand entitlement refresh (e.g. the Settings refresh button): refresh the
/// access token, re-mint + re-verify the entitlement token (renewing grace).
/// `None` => signed out.
#[tauri::command]
pub async fn refresh_entitlement(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<AuthResult>, String> {
    let Some(existing) = current_auth(&state) else { return Ok(None) };

    let session = client::refresh(&existing.session.refresh_token).await.tauri()?;
    jwt::verify(&session.access_token).await.tauri()?;
    crate::json_store::save_json(&state.app_data_dir.join("session.json"), &session).tauri()?;

    match provision::provision(&state.app_data_dir, &session.access_token).await.tauri()? {
        Provisioned::Active(entitlement) => {
            set_auth(&state, session, entitlement.clone());
            let _ = app.emit(crate::constants::events::TIER_CHANGED, ());
            Ok(Some(AuthResult::Entitlement(entitlement)))
        }
        Provisioned::DeviceLimit(devices) => {
            // This device lost its seat (disconnected elsewhere). Drop to Free and
            // let the UI prompt a re-selection.
            set_auth(&state, session, Entitlement::free());
            provision::clear_cached(&state.app_data_dir);
            let _ = app.emit(crate::constants::events::TIER_CHANGED, ());
            Ok(Some(AuthResult::DeviceLimit { devices }))
        }
    }
}

/// Sign out: disconnect this device server-side (frees its seat), clear the
/// in-memory auth state, and remove the local caches.
#[tauri::command]
pub async fn sign_out(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(existing) = current_auth(&state) {
        // Best-effort: free this device's seat. Ignore failures (offline sign-out).
        let _ = client::disconnect_device(&existing.session.access_token, &device::device_hash()).await;
    }

    let _ = std::fs::remove_file(state.app_data_dir.join("session.json"));
    provision::clear_cached(&state.app_data_dir);

    if let Ok(mut guard) = state.auth.lock() {
        *guard = None;
    }
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn current_auth(state: &State<'_, AppState>) -> Option<AuthState> {
    state.auth.lock().ok().and_then(|g| g.clone())
}

fn set_auth(state: &State<'_, AppState>, session: Session, entitlement: Entitlement) {
    if let Ok(mut guard) = state.auth.lock() {
        *guard = Some(AuthState { session, entitlement });
    }
}
