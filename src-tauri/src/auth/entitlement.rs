use crate::json_store::{load_json, save_json};
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

/// The user's authorization state, resolved from the Supabase `entitlements`
/// table and cached to `entitlement.json` for offline use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entitlement {
    /// Email of the signed-in user (for display only). `None` for dev bypass.
    pub email: Option<String>,
    pub tier: Tier,
    /// Subscription expiry date. `None` = active subscription with no fixed end.
    pub expires_at: Option<NaiveDate>,
    /// When this entitlement was last confirmed by Supabase.
    pub last_verified: DateTime<Utc>,
}

impl Entitlement {
    /// A Free entitlement (no paid row, signed out, or expired grace).
    pub fn free() -> Self {
        Self {
            email: None,
            tier: Tier::Free,
            expires_at: None,
            last_verified: Utc::now(),
        }
    }

    /// Whether the cached entitlement is still within the offline grace window
    /// (14 days from last successful verification).
    pub fn is_grace_period_valid(&self) -> bool {
        Utc::now() - self.last_verified < Duration::days(14)
    }

    /// The tier actually in effect right now. Downgrades to Free only when the
    /// subscription `expires_at` is in the past AND the offline grace window has
    /// also lapsed.  Within the grace window the cached tier is kept because the
    /// subscription may have been renewed since the last server verification.
    pub fn effective_tier(&self) -> Tier {
        if let Some(expiry) = self.expires_at {
            if Utc::now().date_naive() > expiry && !self.is_grace_period_valid() {
                return Tier::Free;
            }
        }
        self.tier.clone()
    }
}

/// Load the cached entitlement from disk. Returns `None` if missing/unparseable.
pub fn load_cached(path: &Path) -> Option<Entitlement> {
    load_json(path).ok()
}

/// Persist the entitlement to `entitlement.json`.
pub fn save_cached(path: &Path, ent: &Entitlement) -> Result<()> {
    save_json(path, ent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grace_boundary() {
        let mut e = Entitlement::free();
        e.tier = Tier::Pro;
        e.last_verified = Utc::now() - Duration::days(13);
        assert!(e.is_grace_period_valid(), "13 days should still be valid");

        e.last_verified = Utc::now() - Duration::days(15);
        assert!(!e.is_grace_period_valid(), "15 days should have lapsed");
    }

    #[test]
    fn effective_tier_downgrades_when_expired() {
        let mut e = Entitlement::free();
        e.tier = Tier::Pro;

        e.expires_at = Some(Utc::now().date_naive() + Duration::days(1));
        assert_eq!(e.effective_tier(), Tier::Pro, "future expiry keeps tier");

        // Past expiry but still within the 14-day grace window (e.g. subscription
        // was renewed offline; we give benefit of the doubt until we can verify).
        e.expires_at = Some(Utc::now().date_naive() - Duration::days(1));
        e.last_verified = Utc::now() - Duration::days(1);
        assert_eq!(e.effective_tier(), Tier::Pro, "past expiry within grace keeps tier");

        // Past expiry AND grace lapsed — definitively downgrade.
        e.last_verified = Utc::now() - Duration::days(15);
        assert_eq!(e.effective_tier(), Tier::Free, "past expiry outside grace downgrades");

        e.expires_at = None;
        assert_eq!(e.effective_tier(), Tier::Pro, "no expiry keeps tier");
    }

    #[test]
    fn cache_round_trip() {
        let dir = std::env::temp_dir().join(format!("magnet-ent-{}", uuid::Uuid::new_v4()));
        let path = dir.join("entitlement.json");
        let mut e = Entitlement::free();
        e.email = Some("a@b.com".into());
        e.tier = Tier::Studio;
        e.expires_at = Some(NaiveDate::from_ymd_opt(2030, 1, 1).unwrap());

        save_cached(&path, &e).unwrap();
        let loaded = load_cached(&path).unwrap();
        assert_eq!(loaded.tier, Tier::Studio);
        assert_eq!(loaded.email.as_deref(), Some("a@b.com"));
        assert_eq!(loaded.expires_at, e.expires_at);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
