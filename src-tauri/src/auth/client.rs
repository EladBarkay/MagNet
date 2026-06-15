use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use crate::auth::entitlement::{Entitlement, Tier};
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

// ── entitlement fetch ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EntitlementRow {
    tier: String,
    expires_at: Option<String>,
}

/// Fetch the caller's entitlement row from PostgREST. RLS guarantees the access
/// token can only read its own row. No row => Free.
///
/// `email` is taken from the verified JWT and stored for display.
pub async fn fetch_entitlement(access_token: &str, email: Option<String>) -> Result<Entitlement> {
    let res = http()?
        .get(format!(
            "{SUPABASE_URL}/rest/v1/entitlements?select=tier,expires_at"
        ))
        .header("apikey", SUPABASE_ANON_KEY)
        .bearer_auth(access_token)
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(anyhow!("entitlement fetch failed: {}", res.status()));
    }

    let rows: Vec<EntitlementRow> = res.json().await?;
    let Some(row) = rows.into_iter().next() else {
        // No row yet — treat as Free.
        return Ok(Entitlement {
            email,
            tier: Tier::Free,
            expires_at: None,
            last_verified: chrono::Utc::now(),
        });
    };

    let tier = match row.tier.as_str() {
        "pro" => Tier::Pro,
        "studio" => Tier::Studio,
        _ => Tier::Free,
    };
    let expires_at = row.expires_at.and_then(|s| {
        // Supabase may return a plain date ("2025-12-31") or a full timestamptz
        // ("2025-12-31T00:00:00+00:00").  Silently ignoring a parse error here
        // would make an expired subscription appear perpetual, so try both.
        chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d")
            .or_else(|_| {
                chrono::DateTime::parse_from_rfc3339(&s).map(|dt| dt.date_naive())
            })
            .ok()
    });

    Ok(Entitlement {
        email,
        tier,
        expires_at,
        last_verified: chrono::Utc::now(),
    })
}
