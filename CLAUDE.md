# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**MagNet** is a cross-platform desktop application for event photographers to batch-apply custom decorative frames to photos for printing and magnet production. The photographer works one event at a time; each event has multiple photo batches (one per SD card dump), per-event frame PNGs, canvas presets for print/export, and a fixed output folder.

## Tech Stack

| Layer | Choice |
|---|---|
| Desktop shell | Tauri v2 |
| Frontend | React + TypeScript + Vite + Tailwind CSS |
| State | React `useState`/`useRef` in `App.tsx` (no Jotai, no external store) |
| Backend | Rust (stable) |
| Image processing | `image` crate (image-rs) — JPG, PNG, TIFF (RAW deferred to v2) |
| EXIF/XMP | `kamadak-exif` + `quick-xml` |
| Parallelism | `rayon` (CPU-bound batch), Tokio (async/IPC) |
| File watching | `notify` crate |
| Licensing | Server-issued tokens + 14-day offline grace, device-bound via `machine-uid` |

## Architecture

### Source Folder Is Read-Only

The photographer's source folder is **never modified**. All app state lives in internal storage:

```
{app_data}/
  events/{event_id}/magnet.json   # all event state
  thumbs/                          # thumbnail cache (SHA-256 keyed)
  license.json
```

When opening a folder, the app matches it against `source_path` in existing `magnet.json` files to resume, or creates a new event.

### Core Data Model

- **Event** — top-level: name, list of `PhotoBatch`es, active `FramePreset`, `CanvasPreset`s, output folder path
- **PhotoBatch** — absolute `source_path` to photographer's folder, list of `Photo`s
- **Photo** — path, EXIF orientation, user overrides (orientation, crop), `print_count`, `export_count`, `content_hash` (SHA-256 of photo + XMP bytes — resets `print_count` when it changes)
- **FramePreset** — absolute paths to landscape + portrait PNG (alpha), target ratio, crop method (center or rule-of-thirds)
- **CanvasPreset** — pixel dimensions, photos-per-canvas, DPI, grid layout (e.g. 2400×1600, 2-up)

Frames are per-event PNGs provided by the photographer (no bundled frames). Paths stored as absolute paths.

### Batch Processing Pipeline

Export/print runs canvases in parallel on a dedicated 4-thread rayon pool (memory ceiling).
Frames are prepared **once** per run via `prepare_frames()` (per-orientation placement dims,
aspect preserved, RGBA8). Per photo, `frame_photo_for_canvas()`:

1. `load_photo(path)` → decode (RGB8 for JPEG)
2. `detect_orientation(photo)` → pixel dimensions → user override
3. Orientation-aware crop ratio: landscape = preset ratio, portrait = **inverted** ratio
4. SIMD crop+resize in one pass (`fast_image_resize`, no intermediate copy)
5. `blend_rgba_over_rgb()` → in-place frame composite (no RGBA round-trip)
6. Rotate 90° if that fills the slot better (landscape photo in portrait slot)
7. Compositor centers the result in its slot — white letterbox, **never stretched**
8. `export_print_ready(framed, out_path)` → RGB JPEG q95 at 300 DPI

Errors per photo: log and skip; batch continues. Progress emitted via Tauri events.
Perf guard: `cargo test --release -- --ignored perf` asserts <100ms/photo (24MP source).
Dev profile compiles deps at opt-level 3 so `tauri dev` image work stays usable.

### Preview Pipeline

- Thumbnails (256px) generated async at batch open, cached to `{app_cache}/thumbs/{sha256}.jpg`
- Virtual list (react-window) in gallery; only visible thumbnails rendered
- Full framed preview: on-demand Rust, cached per `(photo_id, preset_id)`, returned as bytes

### Print / Export

- **Export**: canvases written to `{output_folder}/{YYYY-MM-DD_HH-MM-SS}/` — no prompt, uses event's fixed output folder
- **Print**: user sets per-photo print quantity → compositor tiles framed photos onto canvas → OS print dialog → `print_count` incremented
- **Canvas presets**: user-defined per event (e.g. "2-up 2400×1600", "4-up 3600×2400")

### File System Watcher

`notify` watches each batch's source folder + all frame PNG paths:
- New photo → auto-add, generate thumbnail
- Photo/XMP change → recompute `content_hash`; if changed: reset `print_count`, invalidate thumbnail
- Frame PNG change → invalidate framed previews using that frame, UI refreshes immediately

### Licensing

- **Free tier**: output watermarked (composited on export/print canvas). No other limits.
- **Pro / Studio tier**: no watermark; tier is server-issued.
- License cache stored in `{app_data}/license.json`, device-bound, 14-day grace period offline.
- Dev bypass: `MAGNET_DEV_EMAIL` / `MAGNET_DEV_KEY` compile-time constants activate Pro instantly.

## Folder Structure

> Reflects the actual codebase (kept in sync — do not revert to the original plan).

```
magnet/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs            # thin entry → magnet_lib::run()
│   │   ├── lib.rs             # AppState, Tauri builder, invoke_handler, license load
│   │   ├── commands/          # Thin Tauri IPC handlers
│   │   │   ├── project.rs     # open/create/save/delete event, batches, refresh_batch, sync_watches, open_in_explorer
│   │   │   ├── gallery.rs     # list_photos, get_thumbnail, get_framed_preview, overrides (preview IPC lives here)
│   │   │   ├── batch.rs       # export_batch, print_photos (watermark per tier)
│   │   │   ├── canvas_preset.rs  # list/create/update/delete_canvas_preset
│   │   │   ├── frame_preset.rs   # list/create/update/delete_frame_preset
│   │   │   └── license.rs     # activate_init, activate_confirm, activate_dev_license, get_license_info, clear_license
│   │   ├── photo/             # Core image processing — no Tauri deps, unit-tested
│   │   │   ├── loader.rs      # load_photo(), read_exif_orientation(), compute_content_hash() (content-based)
│   │   │   ├── orientation.rs # detect_orientation() → Photo::effective_orientation()
│   │   │   ├── crop.rs        # compute_crop_rect() (center + rule-of-thirds), apply_crop() [tests]
│   │   │   ├── frame.rs       # apply_frame_overlay(), apply_frame_overlay_image() [tests]
│   │   │   ├── export.rs      # export_print_ready() — RGB JPEG q95, 300 DPI JFIF
│   │   │   └── batch.rs       # frame_photo_for_canvas() (export/print per-photo path)
│   │   ├── canvas/            # compositor.rs — tile + apply_watermark() (procedural, free tier)
│   │   ├── project/           # model.rs + persistence.rs (serde_json, in-memory cache) [tests]
│   │   ├── preview/           # thumbnail.rs (256px disk cache) + framed_preview.rs (1200px)
│   │   ├── license/           # validator.rs (cache load/save, grace period), client.rs (HTTP OTP flow, revalidation), device.rs (machine-uid fingerprint)
│   │   └── watcher/           # fs_watcher.rs — notify, emits `fs-changed` with changed path
│   └── Cargo.toml
├── src/
│   ├── components/            # flat (no nested folders)
│   │   ├── Gallery.tsx        # react-window FixedSizeGrid virtual grid
│   │   ├── PhotoCard.tsx      # thumbnail tile + qty stepper (bottom overlay, default 1)
│   │   ├── PreviewPanel.tsx   # framed preview + orientation override + export/print counts
│   │   ├── ProcessDialog.tsx  # export/print config: frame+canvas preset pickers, progress bar
│   │   ├── FramePresetDialog.tsx   # create/edit frame preset
│   │   ├── CanvasPresetForm.tsx    # create/edit canvas preset (used by manager)
│   │   ├── CanvasPresetManager.tsx # list/edit/delete canvas presets
│   │   ├── SettingsDialog.tsx      # license activation (OTP flow + dev bypass), tier display
│   │   ├── EmptyState.tsx          # empty gallery placeholder
│   │   ├── icons.tsx               # SVG icon components
│   │   └── ui.tsx                  # shared Modal and primitive UI components
│   └── hooks/                 # useThumbnail.ts, useFramedPreview.ts, useFsWatcher.ts, useExportProgress.ts
└── package.json
```

### Print / Export flow (current)

Per-photo quantities are set via the qty stepper at the **bottom of each gallery card** (default 1).
All quantities live in `App.photoQueue: Record<string, number>` — a unified session-only state for
both print and export. The toolbar **Print/Export** button opens `ProcessDialog` to pick a frame
preset + canvas preset, then calls `print_photos` or `export_batch`. After completion, `print_count`
/ `export_count` on each Photo is bumped optimistically and the queue is cleared. Actual printer
submission is deferred — files go to a temp dir, OS printer dialog is not yet wired.

### Licensing (current)

Two-step server activation: email + key → server sends OTP → `activate_confirm` → `license.json`
written to `{app_data}/`. `license.json` is bound to the device via `device_id` (machine-uid);
mismatched files are rejected. 14-day offline grace period; after expiry the app falls back to Free.

Background revalidation runs at startup (retries every 60 s if unreachable). Emits `tier-changed`
or `license-expired` Tauri events when state changes.

**Dev bypass**: enter `MAGNET_DEV_EMAIL` + `MAGNET_DEV_KEY` in Settings → calls
`activate_dev_license`, skips OTP, writes a synthetic Pro `LicenseInfo` with `token = "dev-token"`.
Revalidation loop skips dev-token licenses entirely.

`AppState::tier()` gates watermarking in `export_batch`/`print_photos`. Free tier composites a
procedural diagonal-stripe watermark (no bundled asset/font).

### File watcher (current)

`FsWatcher` emits a Tauri `fs-changed` event with the changed file path. The frontend decides:
a frame-PNG path → bump a preview nonce to refetch framed previews; otherwise refresh the owning
batch via `refresh_batch` (which recomputes content hashes and resets `print_count` for changed
photos in `merge_photos`). Thumbnails bust automatically because `useThumbnail` keys on
`content_hash`. `sync_watches` re-establishes watches (batch folders + frame dirs) on event open.

## Performance Targets

| Target | Strategy |
|---|---|
| Gallery scroll <16ms | react-window virtual list; pre-cached thumbnails |
| Thumbnail <200ms | Disk-cached at batch open |
| Framed preview <500ms | On-demand Rust, cached per (photo, preset) |
| 100 photos <10s | rayon, max 4 in-flight (~70MB each decoded) |
| Memory ceiling ~500MB | Bounded concurrency in rayon pool |

