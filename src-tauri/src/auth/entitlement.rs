use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
    Studio,
}

/// The user's authorization state, derived from a verified, server-signed
/// entitlement token (see `entitlement_token`). Display/UI type — the trusted
/// source is the token, not this struct.
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

    /// The tier actually in effect right now. The in-memory `Entitlement` is only
    /// ever populated from a *verified* entitlement token (whose signature and
    /// `exp` were checked), so trust reduces to a single runtime guard: the 14-day
    /// offline-grace ceiling measured from `last_verified` (= the token's `iat`,
    /// i.e. the last successful online verification). This catches a session left
    /// running past the grace window without a successful re-verification.
    ///
    /// Subscription end (`expires_at`) is enforced server-side at mint time — once
    /// it lapses the server stops issuing paid tokens — so it is display-only here
    /// and deliberately does NOT factor into the effective tier.
    pub fn effective_tier(&self) -> Tier {
        if !self.is_grace_period_valid() {
            return Tier::Free;
        }
        self.tier.clone()
    }
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
    fn effective_tier_tracks_grace_only() {
        let mut e = Entitlement::free();
        e.tier = Tier::Pro;

        // Within grace → keep the tier, regardless of subscription end date
        // (subscription expiry is enforced server-side at mint, not here).
        e.last_verified = Utc::now() - Duration::days(1);
        e.expires_at = Some(Utc::now().date_naive() - Duration::days(1));
        assert_eq!(e.effective_tier(), Tier::Pro, "within grace keeps tier");

        // Grace lapsed (no successful online re-verification in 14 days) → Free.
        e.last_verified = Utc::now() - Duration::days(15);
        e.expires_at = None;
        assert_eq!(e.effective_tier(), Tier::Free, "lapsed grace downgrades to Free");
    }

    #[test]
    fn cache_round_trip() {
        let dir = std::env::temp_dir().join(format!("magnet-ent-{}", uuid::Uuid::new_v4()));
        let path = dir.join("entitlement.json");
        let mut e = Entitlement::free();
        e.email = Some("a@b.com".into());
        e.tier = Tier::Studio;
        e.expires_at = Some(NaiveDate::from_ymd_opt(2030, 1, 1).unwrap());

        crate::json_store::save_json(&path, &e).unwrap();
        let loaded: Entitlement = crate::json_store::load_json(&path).unwrap();
        assert_eq!(loaded.tier, Tier::Studio);
        assert_eq!(loaded.email.as_deref(), Some("a@b.com"));
        assert_eq!(loaded.expires_at, e.expires_at);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
