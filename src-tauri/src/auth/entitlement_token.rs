//! Server-signed entitlement token (EdDSA JWS) — the tamper-resistant
//! replacement for the old trusted plaintext `entitlement.json`.
//!
//! A Supabase Edge Function signs a small set of claims with a private Ed25519
//! key that never leaves the server; the client verifies the signature offline
//! against the public key baked in at build time (`ENTITLEMENT_PUBLIC_KEY`). A
//! tier is only ever honored if it came from a valid, unexpired, device-matched
//! signature, so editing the file or copying it to another machine yields Free.

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, NaiveDate, Utc};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

use crate::auth::entitlement::{Entitlement, Tier};

/// Ed25519 public key (SPKI PEM) baked in by `build.rs`, with newlines escaped to
/// the literal two-char sequence `\n` (decoded in [`public_key`]).
const ENTITLEMENT_PUBLIC_KEY: &str = env!("ENTITLEMENT_PUBLIC_KEY");

/// Claims carried by the entitlement token. `exp` is the offline-grace ceiling
/// (`iat + 14d`, server-set); `device` binds the token to one machine.
#[derive(Debug, Deserialize)]
struct EntitlementClaims {
    /// Supabase user id.
    #[allow(dead_code)]
    sub: String,
    tier: String,
    #[serde(default)]
    email: Option<String>,
    /// Subscription end (ISO date) — display only, never used to extend grace.
    #[serde(default)]
    sub_expires_at: Option<String>,
    /// Device hash this token is bound to.
    device: String,
    /// Issued-at = timestamp of the last successful online verification.
    iat: i64,
    /// Signature validity ceiling = `iat + 14d` (the offline grace window).
    #[allow(dead_code)]
    exp: i64,
}

fn public_key() -> Result<DecodingKey> {
    let pem = ENTITLEMENT_PUBLIC_KEY.replace("\\n", "\n");
    DecodingKey::from_ed_pem(pem.as_bytes())
        .map_err(|e| anyhow!("entitlement public key is not a valid Ed25519 PEM: {e}"))
}

/// Verify a token against the baked-in public key and the local device.
pub fn verify(token: &str, device_hash: &str) -> Result<Entitlement> {
    verify_with_key(token, device_hash, &public_key()?)
}

/// Pure verification against a provided key (no baked-in key, no clock skew of
/// its own beyond `jsonwebtoken`'s `exp` check). Testable.
fn verify_with_key(token: &str, device_hash: &str, key: &DecodingKey) -> Result<Entitlement> {
    let mut validation = Validation::new(Algorithm::EdDSA);
    // This is our own token (not a Supabase access token): no issuer/aud to gate,
    // but `exp` MUST be present and in the future — that is the grace ceiling.
    validation.required_spec_claims.clear();
    validation.set_required_spec_claims(&["exp"]);
    validation.validate_aud = false;

    let claims = decode::<EntitlementClaims>(token, key, &validation)
        .map_err(|e| anyhow!("entitlement token verification failed: {e}"))?
        .claims;

    // Reject a token minted for a different machine (copied caches).
    if claims.device != device_hash {
        bail!("entitlement token is bound to a different device");
    }

    let tier = match claims.tier.as_str() {
        "pro" => Tier::Pro,
        "studio" => Tier::Studio,
        _ => Tier::Free,
    };
    let expires_at = claims.sub_expires_at.as_deref().and_then(parse_date);
    // `iat` is the server's last-verified timestamp; the in-memory `Entitlement`
    // re-checks the 14-day grace against it for long-running sessions.
    let last_verified = DateTime::<Utc>::from_timestamp(claims.iat, 0).unwrap_or_else(Utc::now);

    Ok(Entitlement { email: claims.email, tier, expires_at, last_verified })
}

/// Accept either a plain date ("2025-12-31") or an RFC3339 timestamp.
fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .or_else(|| DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.date_naive()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;

    // Throwaway Ed25519 keypair generated for these tests only (never used in
    // prod). Mirrors the test-key pattern in `jwt.rs`.
    const TEST_PRIV_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIAC+hJawRoelB9pWGxngEvqyHdJp9BpHaEKZq0oYrSpR\n-----END PRIVATE KEY-----";
    const TEST_PUB_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAZ1hV73mTUnlfFsGh43/zUg7DpKCOuB315yA/56/yjMg=\n-----END PUBLIC KEY-----";
    // A second, unrelated keypair's public key — used for the wrong-key test.
    const OTHER_PUB_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAx3+FLUHzKI8Id7spgI2te87v2sHxIVQFga0MgGxNsTA=\n-----END PUBLIC KEY-----";

    const DEVICE: &str = "device-abc";

    #[derive(Serialize)]
    struct TestClaims {
        sub: String,
        tier: String,
        device: String,
        iat: i64,
        exp: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        sub_expires_at: Option<String>,
    }

    fn now() -> i64 {
        Utc::now().timestamp()
    }

    fn sign(tier: &str, device: &str, exp: i64) -> String {
        let header = Header::new(Algorithm::EdDSA);
        let claims = TestClaims {
            sub: "user-1".into(),
            tier: tier.into(),
            device: device.into(),
            iat: now(),
            exp,
            sub_expires_at: None,
        };
        let key = EncodingKey::from_ed_pem(TEST_PRIV_PEM.as_bytes()).unwrap();
        encode(&header, &claims, &key).unwrap()
    }

    fn key(pem: &str) -> DecodingKey {
        DecodingKey::from_ed_pem(pem.as_bytes()).unwrap()
    }

    #[test]
    fn valid_token_yields_tier() {
        let token = sign("studio", DEVICE, now() + 1000);
        let ent = verify_with_key(&token, DEVICE, &key(TEST_PUB_PEM)).unwrap();
        assert_eq!(ent.tier, Tier::Studio);
    }

    #[test]
    fn wrong_device_rejected() {
        let token = sign("pro", DEVICE, now() + 1000);
        assert!(verify_with_key(&token, "different-device", &key(TEST_PUB_PEM)).is_err());
    }

    #[test]
    fn expired_token_rejected() {
        // Well past jsonwebtoken's default 60s exp leeway.
        let token = sign("pro", DEVICE, now() - 3600);
        assert!(verify_with_key(&token, DEVICE, &key(TEST_PUB_PEM)).is_err());
    }

    #[test]
    fn wrong_key_rejected() {
        // A token signed by our test key must NOT verify against another key. The
        // signature covers the whole payload, so this also proves a tampered tier
        // (which invalidates the signature) is rejected.
        let token = sign("studio", DEVICE, now() + 1000);
        assert!(verify_with_key(&token, DEVICE, &key(OTHER_PUB_PEM)).is_err());
    }
}
