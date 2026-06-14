# MagNet

**Batch photo framing for event photographers — Tauri v2 desktop app (Windows / macOS / Linux)**

> v0.1.0 — feature-complete demo build

MagNet lets photographers drag in an SD card dump, pick a decorative frame PNG per event, and export or print composite canvases (magnets, 2-up prints, etc.) in one click. Every source photo is read-only; all state lives in an internal JSON store. The heavy image work runs in Rust with SIMD resize and a bounded rayon pool.

---

## Tech Stack

| Layer | Choice |
|---|---|
| Desktop shell | Tauri v2 |
| Frontend | React 19 + TypeScript + Vite + Tailwind CSS |
| Backend | Rust (stable) |
| Image processing | `image` crate — JPEG, PNG, TIFF |
| SIMD crop+resize | `fast_image_resize` |
| EXIF / XMP | `kamadak-exif` + `quick-xml` |
| Parallelism | `rayon` (CPU-bound batch), `tokio` (async IPC) |
| File watching | `notify` crate |
| Licensing | Server-issued tokens, 14-day offline grace period, HMAC-SHA256 device binding |

---

## Prerequisites

### All platforms
- **Rust** (stable toolchain via [rustup](https://rustup.rs)): `rustup update stable`
- **Node.js ≥ 18** + npm

### Windows
- **MSVC Build Tools** — install *Desktop development with C++* workload from the [Visual Studio installer](https://visualstudio.microsoft.com/visual-cpp-build-tools/) or via: `winget install Microsoft.VisualStudio.2022.BuildTools`
- **WebView2** — pre-installed on Windows 10/11; otherwise download from Microsoft
- **Faster linking** (recommended): `rustup component add llvm-tools`
  - Enables `rust-lld` + `/DEBUG:FASTLINK` configured in `src-tauri/.cargo/config.toml`

### macOS
- Xcode Command Line Tools: `xcode-select --install`

### Linux
- WebKit2GTK + OpenSSL dev headers. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#linux) for your distro.

---

## Getting Started

```bash
git clone <repo-url>
cd MagNet
npm install
npm run tauri dev
```

The first build compiles ~500 Rust crates — expect **5–10 minutes**. Subsequent builds are incremental (seconds for frontend changes, 10–30 s for Rust changes).

The dev profile deliberately compiles dependencies at `opt-level 3` while your own code stays at `opt-level 1`, keeping JPEG/resize operations usable during development without sacrificing rebuild speed.

---

## Project Structure

```
MagNet/
├── src/                        # React frontend
│   ├── App.tsx                 # Root component — all UI state lives here
│   ├── types.ts                # TypeScript mirrors of the Rust data model
│   ├── components/             # UI components (flat, no subfolders)
│   │   ├── Toolbar.tsx         # Top bar: open event, batch-wide qty stepper, print/export
│   │   ├── Sidebar.tsx         # Event tree: batches + frame presets + canvas presets
│   │   ├── Gallery.tsx         # react-window virtual grid of photo thumbnails
│   │   ├── PhotoCard.tsx       # Thumbnail tile + qty stepper (bottom overlay)
│   │   ├── PreviewPanel.tsx    # Framed preview + orientation override + export/print counts
│   │   ├── ExportDialog.tsx    # Export config, per-photo qty, progress bar
│   │   ├── PrintConfirmDialog.tsx  # Frame + canvas preset pickers → print
│   │   ├── FramePresetDialog.tsx   # Create / edit frame preset
│   │   ├── CanvasPresetManager.tsx # List / edit / delete canvas presets
│   │   └── SettingsDialog.tsx      # License key entry + tier display
│   ├── hooks/
│   │   ├── useThumbnail.ts         # Fetch + cache 256px thumbnail (keyed on content_hash)
│   │   ├── useFramedPreview.ts     # Fetch 1200px framed preview on demand
│   │   ├── useFsWatcher.ts         # Listen for `fs-changed` Tauri events
│   │   └── useExportProgress.ts    # Subscribe to `export-progress` Tauri events
│   └── lib/
│       └── paths.ts            # basename(), batchDisplayPath() helpers
│
├── src-tauri/
│   ├── src/
│   │   ├── main.rs             # Entry point → magnet_lib::run()
│   │   ├── lib.rs              # AppState, Tauri builder, invoke_handler, startup license load
│   │   ├── commands/           # Thin Tauri IPC handlers — no business logic
│   │   │   ├── project.rs      # open/create/save/delete event, batches, refresh_batch, sync_watches
│   │   │   ├── gallery.rs      # list_photos, get_thumbnail, get_framed_preview, orientation/crop overrides
│   │   │   ├── batch.rs        # export_batch, print_photos (watermark per tier)
│   │   │   ├── canvas_preset.rs
│   │   │   ├── frame_preset.rs
│   │   │   └── license.rs      # activate_init, activate_confirm, activate_dev_license, get_license_info, clear_license
│   │   ├── photo/              # Core image engine — zero Tauri deps, fully unit-tested
│   │   │   ├── loader.rs       # load_photo(), read_exif_orientation(), compute_content_hash()
│   │   │   ├── orientation.rs  # detect_orientation() — pixel dims + user override
│   │   │   ├── crop.rs         # compute_crop_rect() (center + rule-of-thirds), apply_crop()
│   │   │   ├── frame.rs        # apply_frame_overlay() — RGBA alpha-composite over RGB
│   │   │   ├── export.rs       # export_print_ready() — JPEG q95 at 300 DPI
│   │   │   └── batch.rs        # frame_photo_for_canvas() — per-photo export/print path
│   │   ├── canvas/
│   │   │   └── compositor.rs   # Tile framed photos onto canvas + apply_watermark() (Free tier)
│   │   ├── project/
│   │   │   ├── model.rs        # Event, PhotoBatch, Photo, FramePreset, CanvasPreset structs
│   │   │   └── persistence.rs  # serde_json load/save with in-memory cache
│   │   ├── preview/
│   │   │   ├── thumbnail.rs    # 256px disk cache at {app_cache}/thumbs/{sha256}.jpg
│   │   │   └── framed_preview.rs  # On-demand 1200px Rust renderer
│   │   ├── license/
│   │   │   ├── validator.rs    # LicenseInfo cache (load/save), device-ID binding, grace period check
│   │   │   ├── client.rs       # HTTP activation (activate_init/confirm, revalidate, deactivate)
│   │   │   └── device.rs       # Hardware-stable device fingerprint via machine-uid
│   │   └── watcher/
│   │       └── fs_watcher.rs   # notify watcher → emits `fs-changed` Tauri event
│   ├── .cargo/config.toml      # Windows: rust-lld linker + /DEBUG:FASTLINK
│   ├── Cargo.toml
│   ├── build.rs
│   └── tauri.conf.json
│
├── CLAUDE.md                   # Architecture reference for Claude Code
├── ROADMAP.md                  # Release plan and feature backlog
└── package.json
```

---

## Architecture

### Source folder is read-only

Photos are **never written or modified**. All app state is stored internally:

```
{app_data}/
  events/{event_id}/magnet.json   # event metadata, presets, print counts
  thumbs/{sha256}.jpg             # thumbnail cache
  license.json
```

When you open a folder, the app matches it against `source_path` in existing `magnet.json` files to resume an event, or creates a new one automatically.

### Data model

```
Event
  ├── name, root_path, output_folder
  ├── frame_presets[]     ← per-event PNGs, not bundled assets
  ├── canvas_presets[]    ← "2-up 2400×1600", "4-up 3600×2400", etc.
  └── batches[]
        └── PhotoBatch
              └── photos[]
                    ├── path, width, height
                    ├── exif_orientation, orientation_override
                    ├── content_hash   ← SHA-256(photo bytes + XMP); resets print_count on change
                    ├── print_count
                    └── export_count
```

`FramePreset` stores absolute paths to two PNGs (landscape + portrait orientation variants), the target aspect ratio, and the crop method (center or rule-of-thirds).

`CanvasPreset` stores pixel dimensions, DPI, photos-per-canvas, and grid layout (rows × columns).

### Export pipeline (per photo)

1. `load_photo()` — decode to RGB8
2. `detect_orientation()` — pixel dimensions + optional user override
3. Compute crop ratio: landscape = preset ratio, portrait = inverted ratio
4. SIMD crop + resize in one pass (`fast_image_resize`, no intermediate buffer)
5. `blend_rgba_over_rgb()` — alpha-composite frame PNG over photo, in-place
6. Rotate 90° if that better fills the canvas slot
7. Compositor centers the result — white letterbox padding, never stretched
8. `export_print_ready()` — JPEG q95, 300 DPI JFIF

Batch runs on a **4-thread rayon pool** (memory ceiling ~400 MB for 24 MP photos). Per-photo errors are logged and skipped; the rest of the batch continues. Progress is emitted via Tauri events.

### Preview cache

- **Thumbnails** (256 px): generated async when a batch is opened; stored on disk keyed by `content_hash`. React hook `useThumbnail` invalidates automatically when the hash changes.
- **Framed previews** (1200 px): generated on demand; stored in `AppState.preview_cache` as `HashMap<(photo_id, preset_id), Vec<u8>>`. Invalidated on orientation/crop overrides, frame preset updates/deletes, and when the frame PNG changes on disk.

### File watcher

`FsWatcher` uses the `notify` crate to watch each batch's source folder and all frame PNG paths. On change, it emits a `fs-changed` Tauri event with the file path. The frontend routes it:

- Frame PNG path → clears the Rust preview cache for that preset + bumps a nonce to force re-fetch
- Any other path → calls `refresh_batch` IPC, which recomputes `content_hash` values and resets `print_count` for changed photos

### Licensing

Activation flow: email + license key → server sends OTP → OTP confirmed → `license.json` written to `{app_data}/`.

- **Free tier**: output canvases get a procedural diagonal-stripe watermark composited at export/print time. No other limits.
- **Pro / Studio tiers**: no watermark; tier is server-issued and cached locally.

The cached license is bound to the device via a hardware fingerprint (`machine-uid`). If the device ID in `license.json` doesn't match, the file is rejected. Offline use is allowed for **14 days** after the last successful server revalidation; after that the license is cleared and the app falls back to Free.

Background revalidation runs at startup (and retries every 60 s if the server was unreachable). A `tier-changed` or `license-expired` Tauri event is emitted when the result differs from the cached state.

---

## Available Scripts

| Command | Description |
|---|---|
| `npm run tauri dev` | Start dev server + Rust hot-rebuild (main workflow) |
| `npm run dev` | Frontend only (Vite, no Rust) |
| `npm run build` | TypeScript type-check + production frontend bundle |
| `npm run tauri build` | Production build — creates platform installers |

**Rust tests** (run from `src-tauri/`):

```bash
cargo test                                         # unit tests: crop, model, license
cargo test --release -- --ignored perf             # perf guard: asserts <100ms/photo
```

---

## Environment Variables

| Variable | Purpose | Default |
|---|---|---|
| `MAGNET_LICENSE_SECRET` | HMAC secret baked into binary at `cargo build`. **Change before any production build.** | `"dev-secret-change-in-prod"` |
| `MAGNET_DEV_EMAIL` | Developer bypass email — activates Pro instantly, no OTP, no server call. | `eladb1231@gmail.com` |
| `MAGNET_DEV_KEY` | Developer bypass key — pair with `MAGNET_DEV_EMAIL` in Settings to activate. | `DEV-MAGNET-PRO` |

---

## Production Build

```bash
npm run tauri build
```

Installers are written to `src-tauri/target/release/bundle/`. The release profile uses `opt-level 3`, `lto = "thin"`, `codegen-units = 1`, `panic = "abort"`, and `strip = true`.
