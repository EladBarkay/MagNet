export type Orientation = "landscape" | "portrait";

export type CropRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type Photo = {
  id: string;
  path: string;
  width: number;
  height: number;
  exif_orientation: Orientation | null;
  orientation_override: Orientation | null;
  crop_override: CropRect | null;
  print_count: number;
  save_count: number;
  content_hash: string;
  size_bytes: number;
  created: number;
  modified: number;
};

export type PhotoBatch = {
  id: string;
  name: string;
  source_path: string;
  photos: Photo[];
};

export type FramePreset = {
  id: string;
  name: string;
  landscape_frame_path: string;
  portrait_frame_path: string;
  target_ratio_w: number;
  target_ratio_h: number;
};

export type CanvasPreset = {
  id: string;
  name: string;
  canvas_width_px: number;
  canvas_height_px: number;
  photos_per_canvas: number;
  dpi: number;
  cols: number;
  rows: number;
};

export type Tier = "free" | "pro" | "studio";

export type Entitlement = {
  email: string | null;
  tier: Tier;
  expires_at: string | null; // ISO date or null (active subscription, no fixed end)
  last_verified: string; // ISO datetime
};

// A machine registered to the subscription (occupies a seat).
export type Device = {
  device_hash: string;
  device_label: string;
  last_seen: string | null;
};

// Result of an auth/provision call: either signed in, or the subscription is at
// its device-seat limit and the user must disconnect one to proceed.
export type AuthResult =
  | ({ kind: "entitlement" } & Entitlement)
  | { kind: "device_limit"; devices: Device[] };

export type MagnetEvent = {
  id: string;
  name: string;
  root_path: string | null;
  batches: PhotoBatch[];
  frame_presets: FramePreset[];
  canvas_presets: CanvasPreset[];
  output_folder: string | null;
  active_frame_preset_id: string | null;
  active_canvas_preset_id: string | null;
};
