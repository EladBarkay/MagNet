use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use crate::auth::session::Session;

const SUPABASE_URL: &str = env!("SUPABASE_URL");
const SUPABASE_ANON_KEY: &str = env!("SUPABASE_ANON_KEY");
const TIMEOUT: Duration = Duration::from_secs(8);

fn http() -> Result<Client> {
    Ok(Client::builder().timeout(TIMEOUT).build()?)
}

// ── token refresh ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TokenRes {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    user: TokenUser,
}

#[derive(Deserialize)]
struct TokenUser {
    id: String,
}

/// Exchange a refresh token for a fresh session via Supabase Auth.
pub async fn refresh(refresh_token: &str) -> Result<Session> {
    let res = http()?
        .post(format!("{SUPABASE_URL}/auth/v1/token?grant_type=refresh_token"))
        .header("apikey", SUPABASE_ANON_KEY)
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(anyhow!("token refresh failed: {}", res.status()));
    }

    let body: TokenRes = res.json().await?;
    Ok(Session {
        access_token: body.access_token,
        refresh_token: body.refresh_token,
        expires_at: body.expires_at,
        user_id: body.user.id,
    })
}

// ── entitlement token (Edge Function: issue-entitlement) ──────────────────────

/// A device occupying a seat on the user's subscription, for the device picker.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct DeviceInfo {
    pub device_hash: String,
    pub device_label: String,
    pub last_seen: Option<String>,
}

/// Outcome of asking the server to mint an entitlement token for this device.
pub enum MintOutcome {
    /// A signed entitlement token (compact EdDSA JWS).
    Token(String),
    /// The subscription is at its device-seat limit; the user must disconnect one
    /// of these devices before this one can be registered.
    DeviceLimitReached(Vec<DeviceInfo>),
}

#[derive(Deserialize)]
struct IssueOk {
    token: String,
}

#[derive(Deserialize)]
struct DeviceLimitBody {
    devices: Vec<DeviceInfo>,
}

fn functions_url(name: &str) -> String {
    format!("{SUPABASE_URL}/functions/v1/{name}")
}

/// Ask the server to register `device_hash` (under the subscription's seat limit)
/// and mint a signed entitlement token for it. A `409` means the seat limit is
/// reached and the response lists the current devices instead.
pub async fn issue_entitlement_token(
    access_token: &str,
    device_hash: &str,
    device_label: &str,
) -> Result<MintOutcome> {
    let res = http()?
        .post(functions_url("issue-entitlement"))
        .header("apikey", SUPABASE_ANON_KEY)
        .bearer_auth(access_token)
        .json(&serde_json::json!({
            "device_hash": device_hash,
            "device_label": device_label,
        }))
        .send()
        .await?;

    if res.status() == reqwest::StatusCode::CONFLICT {
        let body: DeviceLimitBody = res.json().await?;
        return Ok(MintOutcome::DeviceLimitReached(body.devices));
    }
    if !res.status().is_success() {
        return Err(anyhow!("issue-entitlement failed: {}", res.status()));
    }
    let body: IssueOk = res.json().await?;
    Ok(MintOutcome::Token(body.token))
}

/// Remove a device from the subscription's registry, freeing a seat. The evicted
/// device drops to Free on its next online check (or when its grace lapses).
pub async fn disconnect_device(access_token: &str, device_hash: &str) -> Result<()> {
    let res = http()?
        .post(functions_url("disconnect-device"))
        .header("apikey", SUPABASE_ANON_KEY)
        .bearer_auth(access_token)
        .json(&serde_json::json!({ "device_hash": device_hash }))
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(anyhow!("disconnect-device failed: {}", res.status()));
    }
    Ok(())
}

/// List the caller's registered devices straight from PostgREST (RLS restricts
/// the read to the caller's own rows), for the Settings "manage devices" view.
pub async fn list_devices(access_token: &str) -> Result<Vec<DeviceInfo>> {
    let res = http()?
        .get(format!(
            "{SUPABASE_URL}/rest/v1/entitlement_devices?select=device_hash,device_label,last_seen"
        ))
        .header("apikey", SUPABASE_ANON_KEY)
        .bearer_auth(access_token)
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(anyhow!("list devices failed: {}", res.status()));
    }
    Ok(res.json().await?)
}
