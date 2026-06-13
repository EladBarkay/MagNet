use anyhow::{bail, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::license::validator::{LicenseInfo, Tier};

const API_BASE: &str = "https://api.magnet.app/v1";
const TIMEOUT: Duration = Duration::from_secs(8);

fn http() -> Result<Client> {
    Ok(Client::builder().timeout(TIMEOUT).build()?)
}

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Serialize)]
struct ActivateInitReq<'a> {
    key: &'a str,
    email: &'a str,
    device_id: &'a str,
    app_version: &'a str,
}

#[derive(Deserialize)]
struct ActivateInitRes {
    ok: bool,
    challenge_id: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ActivateConfirmReq<'a> {
    challenge_id: &'a str,
    otp: &'a str,
    device_id: &'a str,
}

#[derive(Serialize)]
struct RevalidateReq<'a> {
    device_id: &'a str,
    token: &'a str,
}

#[derive(Serialize)]
struct DeactivateReq<'a> {
    device_id: &'a str,
    token: &'a str,
}

#[derive(Deserialize)]
struct LicenseRes {
    ok: bool,
    tier: Option<String>,
    expires_at: Option<String>,
    token: Option<String>,
    error: Option<String>,
}

// ── Result type for revalidation ─────────────────────────────────────────────

pub enum RevalidateResult {
    /// Server confirmed the license; returns updated info.
    Ok(LicenseInfo),
    /// Server explicitly rejected (revoked, expired, unknown device).
    Revoked,
    /// Network error or timeout — server not reachable.
    Unreachable,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Step 1 of activation: validate email + key, trigger OTP email.
/// Returns `challenge_id` to pass to `activate_confirm`.
pub async fn activate_init(
    key: &str,
    email: &str,
    device_id: &str,
    app_version: &str,
) -> Result<String> {
    let res: ActivateInitRes = http()?
        .post(format!("{API_BASE}/activate/init"))
        .json(&ActivateInitReq { key, email, device_id, app_version })
        .send()
        .await?
        .json()
        .await?;

    if res.ok {
        res.challenge_id.ok_or_else(|| anyhow::anyhow!("server returned no challenge_id"))
    } else {
        bail!(res.error.unwrap_or_else(|| "activation failed".into()))
    }
}

/// Step 2 of activation: verify OTP. Returns full `LicenseInfo` on success.
pub async fn activate_confirm(
    challenge_id: &str,
    otp: &str,
    device_id: &str,
    email: &str,
) -> Result<LicenseInfo> {
    let res: LicenseRes = http()?
        .post(format!("{API_BASE}/activate/confirm"))
        .json(&ActivateConfirmReq { challenge_id, otp, device_id })
        .send()
        .await?
        .json()
        .await?;

    if !res.ok {
        bail!(res.error.unwrap_or_else(|| "verification failed".into()))
    }
    parse_license(res, email, device_id)
}

/// Check whether the current license is still valid. Never panics — returns
/// `Unreachable` on any network error so the caller can apply grace-period logic.
pub async fn revalidate(info: &LicenseInfo) -> RevalidateResult {
    let result = http()
        .and_then(|c| Ok(c.post(format!("{API_BASE}/revalidate"))
            .json(&RevalidateReq { device_id: &info.device_id, token: &info.token })))
        .map(|r| async move { r.send().await });

    let response = match result {
        Ok(fut) => match fut.await {
            Ok(r) => r,
            Err(_) => return RevalidateResult::Unreachable,
        },
        Err(_) => return RevalidateResult::Unreachable,
    };

    let res: LicenseRes = match response.json().await {
        Ok(r) => r,
        Err(_) => return RevalidateResult::Unreachable,
    };

    if !res.ok {
        return RevalidateResult::Revoked;
    }

    match parse_license(res, &info.email, &info.device_id) {
        Ok(updated) => RevalidateResult::Ok(updated),
        Err(_) => RevalidateResult::Revoked,
    }
}

/// Signal to the server that this device is deactivating (frees a seat).
/// Fire-and-forget on error — if it fails, the seat will remain used until
/// an admin deactivates it from the account portal.
pub async fn deactivate(device_id: &str, token: &str) {
    if let Ok(c) = http() {
        let _ = c.post(format!("{API_BASE}/deactivate"))
            .json(&DeactivateReq { device_id, token })
            .send()
            .await;
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_license(res: LicenseRes, email: &str, device_id: &str) -> Result<LicenseInfo> {
    let tier = match res.tier.as_deref() {
        Some("pro") => Tier::Pro,
        Some("studio") => Tier::Studio,
        Some(other) => bail!("unknown tier: {other}"),
        None => bail!("missing tier in response"),
    };
    let token = res.token.ok_or_else(|| anyhow::anyhow!("missing token in response"))?;
    let expires_at = res.expires_at.and_then(|s| {
        chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()
    });

    Ok(LicenseInfo {
        email: email.to_string(),
        tier,
        expires_at,
        token,
        device_id: device_id.to_string(),
        cached_at: chrono::Utc::now(),
    })
}
