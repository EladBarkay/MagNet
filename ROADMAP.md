# MagNet Roadmap

## Status: v0.1.0 (feature-complete, demo-ready)

Core pipeline is working: event/batch management, gallery, framed preview, export, print, FS watcher, licensing.

---

## v1.0 — Production-Ready

### Tier 2: Feature Completeness

| # | Task | Files | Estimate |
|---|------|-------|----------|
| 2.1 | Frame preset edit UI | `FramePresetDialog.tsx`, `commands/frame_preset.rs` | 2-3h |
| 2.2 | Photo crop/rotation override UI | `PreviewPanel.tsx`, `commands/gallery.rs` | 3-4h |
| 2.3 | XMP sidecar processing (read adjustments, apply on export) | `photo/loader.rs`, `photo/batch.rs` | 4-5h |
| 2.4 | RAW format support (CR2, NEF, ARW, DNG) — embedded JPEG for gallery, demosaiced for export | `Cargo.toml` (+rawloader), `photo/loader.rs` | 5-6h |

### Tier 3: Robustness & Polish

| # | Task | Files | Estimate |
|---|------|----------|----------|
| 3.1 | Unit tests — crop module | `photo/crop.rs` | 2h |
| 3.2 | Unit tests — frame overlay | `photo/frame.rs` | 2h |
| 3.3 | Unit tests — license validator | `license/validator.rs` | 2-3h |
| 3.4 | Unit tests — canvas compositor | `canvas/compositor.rs` | 2-3h |
| 3.5 | Export error tracking + retry UI | `ExportDialog.tsx`, `commands/batch.rs` | 3h |
| 3.6 | Memory benchmark + optimization (target: <500MB for 100 photos) | `commands/batch.rs` | 2-3h |
| 3.7 | Dark mode theme toggle | `App.tsx`, `SettingsDialog.tsx`, all components | 2-3h |

### Performance Targets (v1.0 gate)

| Metric | Target |
|--------|--------|
| Export per photo | ≤ 0.1s (100 photos ≤ 10s total) |
| Framed preview | < 500ms |
| Thumbnail load | < 200ms (from disk cache) |
| Gallery scroll | 60 FPS (react-window virtual list) |
| Memory peak (100 photos) | < 500MB (rayon 4-concurrent, ~70MB/photo) |

---

## v1.1 — Cloud & Collaboration

- Multi-device sync (delta sync `magnet.json` only; photo files stay local)
- Cloud backup of event metadata + thumbnail cache
- Shared event gallery link for client feedback (read-only, star ratings)

## v1.2 — Advanced Editing

- In-app XMP editor per photo (exposure, white balance, saturation, contrast) with real-time preview
- Photo tagging & filtering (star ratings, flags, keywords; persisted to XMP)
- Batch metadata edit (apply tags/adjustments to selection)
- Preset library sharing (export/import frame & canvas presets as JSON)

## v1.3 — Ecosystem & Monetization

Licensing tiers:

| Tier | Features | Price |
|------|----------|-------|
| Free | JPG/PNG framing, 1 frame + 1 canvas preset, watermark | Free |
| Pro | RAW support, no watermark, unlimited presets, XMP editor, cloud backup | $79/yr |
| Studio | Team management, frame library, API access, white-label export | $199/yr |

Additional:
- Bundled frame library (50+ curated frames; in-app marketplace)
- Canvas preset marketplace (community presets; 70/30 revenue split)
- Print-on-demand API integrations (Printful, PrintNinja)
- Tablet companion app (iOS/Android — read-only gallery + remote preset selection)

## v2.0+ — Long-Term Vision

- AI auto-crop, smile/eye-closed detection, background blur suggestions
- Native mobile app (offline-first, quick export)
- Lightroom/Capture One plugin, Dropbox/Drive auto-sync
