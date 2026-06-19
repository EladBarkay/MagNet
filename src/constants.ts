/** Tauri event channel names. Must stay in sync with Rust `constants::events`
 *  (src-tauri/src/constants.rs). */
export const EVENTS = {
  FS_CHANGED: "fs-changed",
  TIER_CHANGED: "tier-changed",
  LICENSE_EXPIRED: "license-expired",
  DEVICE_LIMIT: "device-limit",
  SAVE_PROGRESS: "save-progress",
  SAVE_COMPLETE: "save-complete",
  FACE_SCAN_PROGRESS: "face-scan-progress",
} as const;
