use serde::{Deserialize, Serialize};

/// A Supabase auth session. The refresh token is the long-lived secret used to
/// mint new access tokens; it is per-user, not device-bound.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix timestamp (seconds) at which `access_token` expires.
    pub expires_at: i64,
    /// Supabase user id (`sub` claim of the verified JWT).
    pub user_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_round_trip() {
        let dir = std::env::temp_dir().join(format!("magnet-sess-{}", uuid::Uuid::new_v4()));
        let path = dir.join("session.json");
        let s = Session {
            access_token: "at".into(),
            refresh_token: "rt".into(),
            expires_at: 1_900_000_000,
            user_id: "uid".into(),
        };
        crate::json_store::save_json(&path, &s).unwrap();
        let loaded: Session = crate::json_store::load_json(&path).unwrap();
        assert_eq!(loaded.refresh_token, "rt");
        assert_eq!(loaded.user_id, "uid");
        assert_eq!(loaded.expires_at, 1_900_000_000);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
