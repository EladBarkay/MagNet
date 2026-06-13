use anyhow::Result;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::Path;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

// App-specific salt so the derived ID can't be cross-correlated with raw OS IDs.
// Override at build time with MAGNET_DEVICE_SECRET env var.
const DEVICE_SECRET: &str = match option_env!("MAGNET_DEVICE_SECRET") {
    Some(s) => s,
    None => "magnet-device-fingerprint-v1",
};

#[derive(Serialize, Deserialize)]
struct DeviceFile {
    device_id: String,
}

/// Returns a stable, machine-bound device ID.
///
/// Strategy (in order of preference):
/// 1. OS machine ID (Windows MachineGuid / Linux /etc/machine-id / macOS IOPlatformUUID),
///    HMAC-SHA256'd with DEVICE_SECRET → 64-char hex string.
///    Survives app reinstalls; unique per OS installation.
/// 2. Fallback: UUID stored in `{app_data}/device.json`.
///    Changes on reinstall but is better than nothing if the OS ID is unavailable.
pub fn get_or_create(app_data: &Path) -> Result<String> {
    if let Ok(os_id) = machine_uid::get() {
        return Ok(hmac_device_id(&os_id));
    }

    // Fallback path — only reached if machine-uid fails (rare on supported OSes).
    let path = app_data.join("device.json");
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(f) = serde_json::from_str::<DeviceFile>(&data) {
            if !f.device_id.is_empty() {
                return Ok(f.device_id);
            }
        }
    }

    let id = Uuid::new_v4().to_string();
    std::fs::create_dir_all(app_data)?;
    let data = serde_json::to_string_pretty(&DeviceFile { device_id: id.clone() })?;
    std::fs::write(path, data)?;
    Ok(id)
}

fn hmac_device_id(os_id: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(DEVICE_SECRET.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(os_id.as_bytes());
    let bytes = mac.finalize().into_bytes();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
