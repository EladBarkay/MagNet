use anyhow::Result;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Studio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub email: String,
    pub tier: Tier,
    /// Subscription expiry date. `None` = active subscription with no fixed end.
    pub expires_at: Option<NaiveDate>,
    /// Opaque server-issued token. Sent on every revalidation call.
    pub token: String,
    /// Device ID this license was activated on. Used to reject copied license files.
    pub device_id: String,
    /// When this cache entry was last confirmed by the server.
    pub cached_at: DateTime<Utc>,
}

impl LicenseInfo {
    /// Whether the cached tier is still within the offline grace window (14 days).
    pub fn is_grace_period_valid(&self) -> bool {
        Utc::now() - self.cached_at < Duration::days(14)
    }
}

/// Load and validate `license.json` from disk.
///
/// Returns `None` if:
/// - File is missing or unparseable
/// - `device_id` in file doesn't match `current_device_id` (copy-paste attack)
pub fn load_cached(path: &Path, current_device_id: &str) -> Option<LicenseInfo> {
    let data = std::fs::read_to_string(path).ok()?;
    let info: LicenseInfo = serde_json::from_str(&data).ok()?;
    if info.device_id != current_device_id {
        return None;
    }
    Some(info)
}

/// Persist `LicenseInfo` to `license.json`.
pub fn save_cached(path: &Path, info: &LicenseInfo) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(info)?;
    std::fs::write(path, data)?;
    Ok(())
}
