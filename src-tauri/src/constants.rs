//! Cross-cutting constants. Currently the Tauri event names, which are emitted
//! from more than one module and must stay in sync with the frontend listeners
//! in `src/constants.ts`.

/// Tauri event channel names (Rust `emit` ↔ TS `listen`).
pub mod events {
    /// A watched source/frame file changed on disk. Payload: changed path (String).
    pub const FS_CHANGED: &str = "fs-changed";
    /// Entitlement tier changed after a background refresh. Payload: ().
    pub const TIER_CHANGED: &str = "tier-changed";
    /// Offline grace lapsed; downgraded to Free. Payload: ().
    pub const LICENSE_EXPIRED: &str = "license-expired";
    /// This device lost/can't take a subscription seat; the user must disconnect
    /// one to proceed. Payload: Vec<DeviceInfo> (the currently registered devices).
    pub const DEVICE_LIMIT: &str = "device-limit";
    /// Per-canvas save progress. Payload: SaveProgress.
    pub const SAVE_PROGRESS: &str = "save-progress";
    /// Save run finished. Payload: SaveComplete.
    pub const SAVE_COMPLETE: &str = "save-complete";
    /// Per-photo face-scan progress. Payload: FaceScanProgress.
    pub const FACE_SCAN_PROGRESS: &str = "face-scan-progress";
}
