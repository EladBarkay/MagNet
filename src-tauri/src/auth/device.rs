//! Device identity for license seat enforcement. The entitlement token is bound
//! to `device_hash()`, so copying the on-disk caches to another machine yields a
//! token whose `device` claim no longer matches → verification fails → Free tier.

use sha2::{Digest, Sha256};

/// Salt mixed into the machine id so the stored/transmitted value can't be
/// reversed to the raw hardware id and isn't comparable across apps.
const DEVICE_SALT: &str = "magnet-device-binding-v1";

/// Stable, non-reversible per-machine identifier (hex SHA-256 of the salted
/// `machine-uid`). Falls back to a constant marker if the OS id is unavailable,
/// so the app still functions (it just shares one logical "seat").
pub fn device_hash() -> String {
    let raw = machine_uid::get().unwrap_or_else(|_| "magnet-unknown-machine".to_string());
    let mut hasher = Sha256::new();
    hasher.update(DEVICE_SALT.as_bytes());
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Human-friendly label shown in the "manage devices" picker, e.g.
/// "Elad's MacBook · macOS". Best-effort — never fails.
pub fn device_label() -> String {
    let name = whoami::fallible::hostname().unwrap_or_else(|_| whoami::devicename());
    format!("{name} · {}", whoami::platform())
}
