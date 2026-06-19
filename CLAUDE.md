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
| EXIF | `kamadak-exif` |
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
- **Photo** — path, EXIF orientation, user overrides (orientation, crop), `print_count`, `save_count`, `content_hash` (SHA-256 of photo + XMP bytes — resets `print_count` when it changes; `save_count` persists)
- **FramePreset** — absolute paths to landscape + portrait PNG (alpha), target ratio (crop is always centered)
- **CanvasPreset** — pixel dimensions, photos-per-canvas, DPI, grid layout (e.g. 2400×1600, 2-up)

Frames are per-event PNGs provided by the photographer (no bundled frames). Paths stored as absolute paths.

### Batch Processing Pipeline

Export/print runs canvases in parallel on a dedicated 4-thread rayon pool (memory ceiling).
Frames are prepared **once** per run via `prepare_frames()` (per-orientation placement dims,
aspect preserved, RGBA8). Per photo, `frame_photo_for_canvas()`:

1. `load_photo(path)` → decode (RGB8 for JPEG)
2. `detect_orientation(photo)` → pixel dimensions → user override
3. Orientation-aware crop ratio: landscape = preset ratio, portrait = **inverted** ratio (always centered)
4. `imageops::crop_and_resize()` — SIMD crop+resize in one pass (`fast_image_resize`), with a `crop_imm`+`resize_exact` fallback in the same fn
5. `imageops::overlay_frame()` → frame composite (in-place RGB8 fast path, else `image::imageops::overlay`)
6. Rotate 90° if that fills the slot better (landscape photo in portrait slot)
7. Compositor centers the result in its slot — white letterbox, **never stretched**
8. `write_print_ready(framed, out_path)` → RGB JPEG q95 at 300 DPI

Errors per photo: log and skip; batch continues. Progress emitted via Tauri events.
Perf guard: `cargo test --release -- --ignored perf` asserts <100ms/photo (24MP source).
Dev profile compiles deps at opt-level 3 so `tauri dev` image work stays usable.

### Preview Pipeline

- Thumbnails (256px) generated async at batch open, cached to `{app_cache}/thumbs/{sha256}.jpg`
- Virtual list (react-window) in gallery; only visible thumbnails rendered
- Full framed preview: on-demand Rust, cached per `(photo_id, preset_id)` (preset `None` → keyed under nil UUID, returns the raw full photo), returned as bytes

### Print / Export

- **Export**: canvases written to `{output_folder}/{YYYY-MM-DD_HH-MM-SS}/` — no prompt, uses event's fixed output folder
- **Print**: user sets per-photo print quantity → compositor tiles framed photos onto canvas → OS print dialog → `print_count` incremented
- **Canvas presets**: user-defined per event (e.g. "2-up 2400×1600", "4-up 3600×2400")

### File System Watcher

`notify` watches each batch's source folder + all frame PNG paths:
- New photo → auto-add, generate thumbnail
- Photo/XMP change → recompute `content_hash`; if changed: reset `print_count`, invalidate thumbnail
- Frame PNG change → invalidate framed previews using that frame, UI refreshes immediately

### Auth & Entitlements (Supabase)

- **Free tier**: output watermarked (composited on export/print canvas). No other limits.
- **Pro / Studio tier**: no watermark; tier comes from a Supabase `entitlements` row.
- Identity via Supabase Auth (email+password, Google, Facebook). Tier is the
  `entitlements` row for the signed-in user — no license keys.
- Session cache (`{app_data}/session.json`, refresh token) + entitlement cache
  (`{app_data}/entitlement.json`), 14-day offline grace from last verification.
- Dev bypass: `MAGNET_DEV_TIER=pro|studio` compile-time env seeds that tier, no sign-in.

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
│   │   │   ├── batch.rs       # save_batch, print_photos (watermark per tier)
│   │   │   ├── canvas_preset.rs  # list/create/update/delete_canvas_preset
│   │   │   ├── frame_preset.rs   # list/create/update/delete_frame_preset
│   │   │   └── auth.rs        # establish_session, get_entitlement, sign_out
│   │   ├── photo/             # Core image processing — no Tauri deps, unit-tested
│   │   │   ├── loader.rs      # load_photo(), read_exif_orientation(), compute_content_hash() (content-based)
│   │   │   ├── orientation.rs # detect_orientation() → Photo::effective_orientation()
│   │   │   ├── crop.rs        # compute_crop_rect() (always centered), apply_crop() [tests]
│   │   │   ├── imageops.rs    # crop_and_resize() + overlay_frame() — fast path + simple fallback per fn [tests]
│   │   │   ├── frame.rs       # apply_frame_overlay() — load frame PNG + overlay_frame() (preview path) [tests]
│   │   │   ├── encode.rs      # write_print_ready() — RGB JPEG q95, 300 DPI JFIF
│   │   │   └── batch.rs       # frame_photo_for_canvas() (save/print per-photo path)
│   │   ├── canvas/            # compositor.rs — tile + apply_watermark() (procedural, free tier)
│   │   ├── project/           # model.rs + persistence.rs (serde_json, in-memory cache) [tests]
│   │   ├── preview/           # thumbnail.rs (256px disk cache) + framed_preview.rs (1200px; preset=None → raw full photo, no crop/frame)
│   │   ├── auth/              # entitlement.rs (Tier, cache, grace + expiry), session.rs (session.json), jwt.rs (Supabase JWKS verify), client.rs (token refresh + entitlement fetch)
│   │   └── watcher/           # fs_watcher.rs — notify, emits `fs-changed` with changed path
│   └── Cargo.toml
├── src/
│   ├── components/            # flat (no nested folders)
│   │   ├── Toolbar.tsx        # slim top band: logo, event name, open/delete, settings/tier
│   │   ├── BatchTabs.tsx      # horizontal batch tab strip (drag-reorder) + view controls (grid size, hide-empty)
│   │   ├── ActionBar.tsx      # sticky bottom band: queued totals + Export; swaps to bulk controls on selection
│   │   ├── Gallery.tsx        # react-window FixedSizeGrid virtual grid (full-width)
│   │   ├── PhotoCard.tsx      # thumbnail tile + qty stepper (bottom overlay, default 1)
│   │   ├── Lightbox.tsx       # full-screen framed preview + prev/next + orientation/frame/copies/counts
│   │   ├── ExportDialog.tsx   # print/save config: frame+canvas preset pick + manage (add/edit/delete), sticky defaults
│   │   ├── FramePresetDialog.tsx   # create/edit frame preset
│   │   ├── CanvasPresetForm.tsx    # create/edit canvas preset (used inside ExportDialog manage)
│   │   ├── SettingsDialog.tsx      # Supabase sign-in (email/password + Google/Facebook), tier display, sign out
│   │   ├── EmptyState.tsx          # empty gallery placeholder
│   │   ├── icons.tsx               # SVG icon components
│   │   └── ui.tsx                  # shared Modal and primitive UI components
│   ├── hooks/                 # useThumbnail.ts, useFramedPreview.ts, useFsWatcher.ts, useSaveProgress.ts, useAuthDeepLink.ts
│   ├── lib/                   # supabase.ts (client), auth.ts (establishFromSession → Rust)
│   ├── i18n.ts                # i18next setup (en + he), LANGS, setLanguage(), syncs <html lang/dir>
│   └── locales/              # en.ts (source-of-truth dictionary) + he.ts (Hebrew, RTL)
└── package.json
```

### Export flow (current)

Per-photo quantities are set via the qty stepper at the **bottom of each gallery card** (default 1).
All quantities live in `App.photoQueue: Record<string, number>` — a unified session-only state for
both print and save. The toolbar **Export** button opens `ExportDialog` to pick a frame
preset + canvas preset and a destination (Print or Save to path), then calls `print_photos` or
`save_batch`. After completion, `print_count` / `save_count` on each Photo is bumped optimistically
and the queue is cleared. Actual printer
submission is deferred — files go to a temp dir, OS printer dialog is not yet wired.

### Auth & Entitlements (current)

**Clean split**: the frontend (`supabase-js`) only drives interactive sign-in;
**Rust is the source of truth for tier**. After email/password or OAuth sign-in,
the frontend hands `{access_token, refresh_token, expires_at}` to Rust via
`establish_session`. Rust verifies the JWT against Supabase JWKS (`auth/jwt.rs`,
asymmetric keys — no secret in the binary), fetches the `entitlements` row over
PostgREST + RLS (`auth/client.rs::fetch_entitlement`, Bearer token → caller's row
only), and persists `session.json` + `entitlement.json` in `{app_data}/`.

OAuth uses PKCE and returns via the custom deep link `magnet://auth-callback`
(`tauri-plugin-deep-link`, handled by `useAuthDeepLink`); Google/Meta only ever
see Supabase's HTTPS callback, never the custom scheme.

`auth_refresh_loop` (in `lib.rs`) runs at startup: refresh access token → verify →
re-fetch entitlement → save → emit `tier-changed`. Retries every 60 s while
offline; if the 14-day grace (from `entitlement.last_verified`) has lapsed it
clears caches and emits `license-expired`. `Entitlement::effective_tier()` also
downgrades to Free once `expires_at` passes.

**Dev bypass**: compile with `MAGNET_DEV_TIER=pro` (or `studio`) → `lib.rs` seeds
a synthetic `AuthState` (sentinel refresh token `dev-bypass`); the refresh loop
skips it. No sign-in, no network.

`AppState::tier()` gates watermarking in `save_batch`/`print_photos`. Free tier
composites a procedural diagonal-stripe watermark (no bundled asset/font).

Supabase project config lives in a single repo-root `.env`
(`SUPABASE_URL`, `SUPABASE_ANON_KEY`) read by both sides: `build.rs` loads it via
`dotenvy` and bakes the values in with `env!()`; Vite exposes them to the client
through `envPrefix: ["VITE_", "SUPABASE_"]`. A real shell env var overrides the
`.env`. See `docs/supabase.md` for project setup + the SQL migration.

### File watcher (current)

`FsWatcher` emits a Tauri `fs-changed` event with the changed file path. The frontend decides:
a frame-PNG path → bump a preview nonce to refetch framed previews; otherwise refresh the owning
batch via `refresh_batch` (which recomputes content hashes and resets `print_count` for changed
photos in `merge_photos`). Thumbnails bust automatically because `useThumbnail` keys on
`content_hash`. `sync_watches` re-establishes watches (batch folders + frame dirs) on event open.

### Internationalization (current)

UI is fully localized via **react-i18next** (`src/i18n.ts`), shipping **English**
and **Hebrew**. All user-facing strings live in `src/locales/{en,he}.ts` (en is the
source-of-truth shape; he carries Hebrew CLDR plural categories one/two/many/other).
Components pull strings with `useTranslation()` → `t("area.key", { count, ...vars })`;
counts use i18next plurals, variables use `{{interpolation}}`.

The language switcher is a dropdown in `SettingsDialog`; the choice persists in
`localStorage` (`magnet.lang`). `i18n.ts` keeps `<html lang/dir>` in sync on init and
on every `languageChanged`, so **Hebrew switches the whole app to RTL** via the native
`dir="rtl"` attribute. Layout flips rely on **Tailwind logical utilities** (`ms-`/`me-`,
`ps-`/`pe-`, `border-s`/`border-e`, `text-start`, `start-`/`end-`) instead of physical
left/right classes; the gallery "hide empty" toggle uses `rtl:-translate-x-*` and the
preview divider drag mirrors its delta when `dir==="rtl"`. To add a language: create
`src/locales/<code>.ts` and add an entry to `LANGS` in `src/i18n.ts`.

## Performance Targets

| Target | Strategy |
|---|---|
| Gallery scroll <16ms | react-window virtual list; pre-cached thumbnails |
| Thumbnail <200ms | Disk-cached at batch open |
| Framed preview <500ms | On-demand Rust, cached per (photo, preset) |
| 100 photos <10s | rayon, max 4 in-flight (~70MB each decoded) |
| Memory ceiling ~500MB | Bounded concurrency in rayon pool |

