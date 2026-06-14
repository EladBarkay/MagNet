use tauri::State;
use crate::license::{client, validator::{LicenseInfo, save_cached}};
use crate::AppState;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Step 1 — validate email + key, send OTP to the user's email.
/// Returns an opaque `challenge_id` to pass to `activate_confirm`.
#[tauri::command]
pub async fn activate_init(
    email: String,
    key: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    client::activate_init(&key, &email, &state.device_id, APP_VERSION)
        .await
        .map_err(|e| e.to_string())
}

/// Step 2 — verify the OTP. On success, persists the license and upgrades the
/// in-memory tier immediately.
#[tauri::command]
pub async fn activate_confirm(
    challenge_id: String,
    otp: String,
    email: String,
    state: State<'_, AppState>,
) -> Result<LicenseInfo, String> {
    let info = client::activate_confirm(&challenge_id, &otp, &state.device_id, &email)
        .await
        .map_err(|e| e.to_string())?;

    let path = state.app_data_dir.join("license.json");
    save_cached(&path, &info).map_err(|e| e.to_string())?;

    if let Ok(mut guard) = state.license.lock() {
        *guard = Some(info.clone());
    }
    Ok(info)
}

/// Returns the currently cached license info (no network call).
#[tauri::command]
pub async fn get_license_info(
    state: State<'_, AppState>,
) -> Result<Option<LicenseInfo>, String> {
    Ok(state.license.lock().ok().and_then(|g| g.clone()))
}

/// Deactivate this device (frees a seat) and remove the local license cache.
#[tauri::command]
pub async fn clear_license(state: State<'_, AppState>) -> Result<(), String> {
    let token_and_device = state.license.lock().ok().and_then(|g| {
        g.as_ref().map(|l| (l.token.clone(), l.device_id.clone()))
    });

    // Fire-and-forget deactivation call — don't block on network failure.
    if let Some((token, device_id)) = token_and_device {
        tokio::spawn(async move {
            client::deactivate(&device_id, &token).await;
        });
    }

    let path = state.app_data_dir.join("license.json");
    let _ = std::fs::remove_file(&path);

    if let Ok(mut guard) = state.license.lock() {
        *guard = None;
    }
    Ok(())
}
