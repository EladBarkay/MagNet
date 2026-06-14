use tauri::State;

use crate::auth::entitlement::{save_cached as save_entitlement, Entitlement};
use crate::auth::session::{save_cached as save_session, Session};
use crate::auth::{client, jwt, AuthState};
use crate::commands::IntoTauri;
use crate::AppState;

/// Called by the frontend after an interactive Supabase sign-in. Rust verifies
/// the access token, fetches the entitlement, persists both caches, and upgrades
/// the in-memory tier immediately. Returns the resolved entitlement.
#[tauri::command]
pub async fn establish_session(
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    state: State<'_, AppState>,
) -> Result<Entitlement, String> {
    // Verify the token against Supabase JWKS — never trust the frontend's word.
    let claims = jwt::verify(&access_token).await.tauri()?;

    let entitlement = client::fetch_entitlement(&access_token, claims.email.clone())
        .await
        .tauri()?;

    let session = Session {
        access_token,
        refresh_token,
        expires_at,
        user_id: claims.sub,
    };

    let session_path = state.app_data_dir.join("session.json");
    let entitlement_path = state.app_data_dir.join("entitlement.json");
    save_session(&session_path, &session).tauri()?;
    save_entitlement(&entitlement_path, &entitlement).tauri()?;

    if let Ok(mut guard) = state.auth.lock() {
        *guard = Some(AuthState { session, entitlement: entitlement.clone() });
    }
    Ok(entitlement)
}

/// Returns the currently cached entitlement (no network call), for UI bootstrap.
/// `None` => signed out / Free.
#[tauri::command]
pub async fn get_entitlement(state: State<'_, AppState>) -> Result<Option<Entitlement>, String> {
    Ok(state
        .auth
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|a| a.entitlement.clone())))
}

/// Sign out: clear the in-memory auth state and remove both caches.
#[tauri::command]
pub async fn sign_out(state: State<'_, AppState>) -> Result<(), String> {
    let _ = std::fs::remove_file(state.app_data_dir.join("session.json"));
    let _ = std::fs::remove_file(state.app_data_dir.join("entitlement.json"));

    if let Ok(mut guard) = state.auth.lock() {
        *guard = None;
    }
    Ok(())
}
