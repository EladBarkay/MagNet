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
    /// Per-canvas export progress. Payload: ExportProgress.
    pub const EXPORT_PROGRESS: &str = "export-progress";
    /// Export run finished. Payload: ExportComplete.
    pub const EXPORT_COMPLETE: &str = "export-complete";
}
