/** Tauri event channel names. Must stay in sync with Rust `constants::events`
 *  (src-tauri/src/constants.rs). */
export const EVENTS = {
  FS_CHANGED: "fs-changed",
  TIER_CHANGED: "tier-changed",
  LICENSE_EXPIRED: "license-expired",
  EXPORT_PROGRESS: "export-progress",
  EXPORT_COMPLETE: "export-complete",
  FACE_SCAN_PROGRESS: "face-scan-progress",
} as const;
