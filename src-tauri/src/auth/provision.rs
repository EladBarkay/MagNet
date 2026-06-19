//! Orchestrates minting, verifying, and caching the device-bound entitlement
//! token. Shared by the IPC commands (`commands::auth`) and the startup refresh
//! loop (`lib.rs`) so the mint → verify → persist sequence lives in one place.

use anyhow::Result;
use std::path::Path;

use crate::auth::client::{self, DeviceInfo, MintOutcome};
use crate::auth::entitlement::Entitlement;
use crate::auth::{device, entitlement_token};
use crate::json_store;

/// On-disk filename for the signed entitlement token (the JWS, stored as a JSON
/// string for atomic writes). Replaces the old trusted plaintext `entitlement.json`.
pub const TOKEN_FILE: &str = "entitlement.token";

/// Result of trying to provision this device with a fresh entitlement token.
pub enum Provisioned {
    /// Token minted, verified, and cached — here is the resolved entitlement.
    Active(Entitlement),
    /// The subscription is at its seat limit; the user must disconnect one of
    /// these devices before this one can be registered. Nothing was cached.
    DeviceLimit(Vec<DeviceInfo>),
}

/// Mint a token for *this* device, verify it locally, and (on success) cache it.
pub async fn provision(data_dir: &Path, access_token: &str) -> Result<Provisioned> {
    let device_hash = device::device_hash();
    let device_label = device::device_label();

    match client::issue_entitlement_token(access_token, &device_hash, &device_label).await? {
        MintOutcome::Token(token) => {
            // Verify before trusting — defends against a compromised/misconfigured
            // server and confirms the device binding round-trips.
            let entitlement = entitlement_token::verify(&token, &device_hash)?;
            json_store::save_json(&data_dir.join(TOKEN_FILE), &token)?;
            Ok(Provisioned::Active(entitlement))
        }
        MintOutcome::DeviceLimitReached(devices) => Ok(Provisioned::DeviceLimit(devices)),
    }
}

/// Load and offline-verify the cached entitlement token (signature + device +
/// grace). `None` if absent, invalid, expired, or bound to another device — all
/// of which mean "fall back to Free until an online re-validation succeeds".
pub fn load_cached(data_dir: &Path) -> Option<Entitlement> {
    let token: String = json_store::load_json(&data_dir.join(TOKEN_FILE)).ok()?;
    entitlement_token::verify(&token, &device::device_hash()).ok()
}

/// Remove the cached token (e.g. on sign-out).
pub fn clear_cached(data_dir: &Path) {
    let _ = std::fs::remove_file(data_dir.join(TOKEN_FILE));
}
