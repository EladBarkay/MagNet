import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Gallery from "./components/Gallery";
import Lightbox from "./components/Lightbox";
import ExportDialog from "./components/ExportDialog";
import FramePresetDialog from "./components/FramePresetDialog";
import SettingsDialog from "./components/SettingsDialog";
import DeviceManagerDialog from "./components/DeviceManagerDialog";
import CanvasPresetForm from "./components/CanvasPresetForm";
import EventConfigDialog from "./components/EventConfigDialog";
import { Modal } from "./components/ui";
import Toolbar from "./components/Toolbar";
import BatchTabs from "./components/BatchTabs";
import ActionBar from "./components/ActionBar";
import EmptyState from "./components/EmptyState";
import { useFsWatcher } from "./hooks/useFsWatcher";
import { useAuthDeepLink } from "./hooks/useAuthDeepLink";
import { useUpdater } from "./hooks/useUpdater";
import { listDevices, currentDeviceHash } from "./lib/auth";
import { reorderById } from "./lib/reorder";
import { rangeIds } from "./lib/selection";
import { EVENTS } from "./constants";
import { OrenewEvent, Orientation, Photo, PhotoBatch, FramePreset, CanvasPreset, Entitlement, AuthResult, Device } from "./types";

type ModalKind = "export" | "settings" | "eventConfig" | null;
export type SortKey = "name" | "created" | "modified" | "size";

export default function App() {
  const { t, i18n } = useTranslation();
  const [event, setEvent] = useState<OrenewEvent | null>(null);
  const [activeBatch, setActiveBatch] = useState<PhotoBatch | null>(null);
  const [selected, setSelected] = useState<Photo | null>(null);
  const [modal, setModal] = useState<ModalKind>(null);
  const [status, setStatus] = useState("");
  const [draggedBatchId, setDraggedBatchId] = useState<string | null>(null);
  const [entitlement, setEntitlement] = useState<Entitlement | null>(null);
  // Device-seat picker: shown on the seat-limit interrupt ("limit") or from
  // Settings → Manage devices ("manage"). `null` => hidden.
  const [devicePicker, setDevicePicker] = useState<{ mode: "limit" | "manage"; devices: Device[] } | null>(null);
  const [deviceHash, setDeviceHash] = useState("");
  const [frameNonce, setFrameNonce] = useState(0);
  const [editingFrame, setEditingFrame] = useState<FramePreset | null>(null);
  const [editingCanvas, setEditingCanvas] = useState<CanvasPreset | null>(null);
  // Add-preset modals stack over the Export dialog (which manages presets), so they
  // get their own state instead of sharing the single `modal` slot.
  const [addingFrame, setAddingFrame] = useState(false);
  const [addingCanvas, setAddingCanvas] = useState(false);
  // Unified per-photo queue: photoId → quantity (session-only).
  const [photoQueue, setPhotoQueue] = useState<Record<string, number>>({});
  const [cellSize, setCellSize] = useState(168);
  const [hideEmpty, setHideEmpty] = useState(false);
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortDir, setSortDir] = useState<1 | -1>(1);
  const [scanning, setScanning] = useState(false);
  const [scanProgress, setScanProgress] = useState<{ done: number; total: number } | null>(null);
  // Multi-selection in the grid; `selected` (above) is the last-clicked photo and
  // drives the preview + shift-range anchor.
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  // Full-screen review is a distinct mode from selection: a plain click opens it,
  // while Ctrl/Shift clicks only multi-select (for bulk copies) without opening it.
  const [lightboxOpen, setLightboxOpen] = useState(false);
  const anchorRef = useRef<string | null>(null);
  // Photo ids the queue has already seeded. New photos that appear later (file
  // watcher) get a default qty of 1; photos the user zeroed stay "seen" so they
  // aren't bumped back up.
  const seenIdsRef = useRef<Set<string>>(new Set());
  // Skips the persist-on-change effect for the queue we just loaded from disk.
  const skipPersistRef = useRef(false);

  function clearSelection() {
    setSelected(null);
    setSelectedIds(new Set());
    anchorRef.current = null;
  }
  const colCountRef = useRef(1);

  // Block the webview reload shortcuts — a reload wipes the in-memory event and
  // forces the user to re-open it.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "r") || e.key === "F5") {
        e.preventDefault();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // Gallery cell size via Ctrl+Plus/Minus and Ctrl+wheel (clamp 100–280, step 20).
  useEffect(() => {
    const bump = (dir: number) =>
      setCellSize((c) => Math.min(280, Math.max(100, c + dir * 20)));
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      if (["+", "=", "Add"].includes(e.key)) { e.preventDefault(); bump(1); }
      else if (["-", "_", "Subtract"].includes(e.key)) { e.preventDefault(); bump(-1); }
    };
    const onWheel = (e: WheelEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      e.preventDefault(); // suppress webview page zoom
      bump(e.deltaY < 0 ? 1 : -1);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("wheel", onWheel);
    };
  }, []);

  useEffect(() => {
    invoke<Entitlement | null>("get_entitlement")
      .then((info) => setEntitlement(info ?? null))
      .catch(() => {});
    currentDeviceHash().then(setDeviceHash).catch(() => {});
  }, []);

  // Translate an auth/provision outcome into UI state: signed in, or open the
  // device-seat picker so the user can disconnect a device to proceed.
  function handleAuthResult(result: AuthResult) {
    if (result.kind === "entitlement") {
      setEntitlement({
        email: result.email,
        tier: result.tier,
        expires_at: result.expires_at,
        last_verified: result.last_verified,
      });
      setDevicePicker(null);
    } else {
      setDevicePicker({ mode: "limit", devices: result.devices });
    }
  }

  // Background refresh: update entitlement when the background task resolves.
  useEffect(() => {
    const unsub = listen<void>(EVENTS.TIER_CHANGED, async () => {
      try {
        const info = await invoke<Entitlement | null>("get_entitlement");
        setEntitlement(info ?? null);
      } catch {}
    });
    const unsub2 = listen<void>(EVENTS.LICENSE_EXPIRED, () => setEntitlement(null));
    // This device lost its seat (disconnected elsewhere): refresh tier (now Free)
    // and prompt a re-selection.
    const unsub3 = listen<Device[]>(EVENTS.DEVICE_LIMIT, async (e) => {
      try {
        const info = await invoke<Entitlement | null>("get_entitlement");
        setEntitlement(info ?? null);
      } catch {}
      setDevicePicker({ mode: "limit", devices: e.payload });
    });
    return () => { unsub.then(fn => fn()); unsub2.then(fn => fn()); unsub3.then(fn => fn()); };
  }, []);

  // Completes OAuth sign-in when the orenew://auth-callback deep link arrives.
  useAuthDeepLink(handleAuthResult);

  // Best-effort signed update check on startup.
  useUpdater();

  useFsWatcher(event, activeBatch, {
    onEvent: setEvent,
    onActiveBatch: (b) => {
      setActiveBatch(b);
      // Refresh the previewed photo too, so its new content_hash flows to the
      // preview/thumbnail and they re-fetch after an on-disk edit/rotation.
      setSelected((prev) => (prev ? (b.photos.find((p) => p.id === prev.id) ?? prev) : prev));
    },
    onFrameChanged: () => setFrameNonce((n) => n + 1),
  });

  // Seed the global copy-queue from each photo's persisted `copies` (across all
  // batches), so a reopened event restores the last values instead of resetting to 1.
  function seedQueueFromEvent(evt: OrenewEvent): Record<string, number> {
    const q: Record<string, number> = {};
    const all: string[] = [];
    for (const b of evt.batches) {
      for (const p of b.photos) {
        all.push(p.id);
        if (p.copies > 0) q[p.id] = p.copies;
      }
    }
    seenIdsRef.current = new Set(all);
    skipPersistRef.current = true; // don't write the freshly-loaded values back
    return q;
  }

  // Seed qty=1 for photos that appear after the batch was opened (added on disk
  // and picked up by the watcher). Runs after refresh updates activeBatch.
  useEffect(() => {
    if (!activeBatch) return;
    const newIds = activeBatch.photos.filter((p) => !seenIdsRef.current.has(p.id));
    if (newIds.length === 0) return;
    setPhotoQueue((prev) => {
      const next = { ...prev };
      for (const p of newIds) next[p.id] = 1;
      return next;
    });
    for (const p of newIds) seenIdsRef.current.add(p.id);
  }, [activeBatch]);

  // Persist queued copies (debounced) so they survive close/reopen. Skips the run
  // right after a load (seedQueueFromEvent) so we don't echo disk values back.
  useEffect(() => {
    if (!event) return;
    if (skipPersistRef.current) { skipPersistRef.current = false; return; }
    const id = event.id;
    const timer = setTimeout(() => {
      invoke("set_photo_copies", { eventId: id, copies: photoQueue }).catch(() => {});
    }, 400);
    return () => clearTimeout(timer);
  }, [photoQueue, event?.id]);

  async function openEvent() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const folder = await open({ directory: true, multiple: false });
      if (!folder) return;
      setStatus(t("app.loading"));
      const evt = await invoke<OrenewEvent>("open_event", { path: folder });
      setEvent(evt);
      setActiveBatch(evt.batches[0] ?? null);
      clearSelection();
      setPhotoQueue(seedQueueFromEvent(evt));
      invoke("sync_watches", { eventId: evt.id }).catch(() => {});
      setStatus("");
    } catch (e) {
      setStatus(t("app.error", { message: String(e) }));
    }
  }

  async function deleteEvent() {
    if (!event) return;
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    const yes = await confirm(
      t("app.deleteEventConfirm", { name: event.name }),
      { title: t("app.deleteEventTitle"), kind: "warning" }
    );
    if (!yes) return;
    try {
      await invoke("delete_event", { eventId: event.id });
      setEvent(null);
      setActiveBatch(null);
      setSelected(null);
    } catch (e) {
      setStatus(t("app.error", { message: String(e) }));
    }
  }

  async function deleteBatch(batch: PhotoBatch) {
    if (!event) return;
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    const yes = await confirm(
      t("app.removeBatchConfirm", { name: batch.name }),
      { title: t("app.removeBatchTitle"), kind: "warning" }
    );
    if (!yes) return;
    try {
      const updated = await invoke<OrenewEvent>("delete_batch", { eventId: event.id, batchId: batch.id });
      updateEvent(updated);
      if (activeBatch?.id === batch.id) {
        setActiveBatch(updated.batches[0] ?? null);
        setSelected(null);
      }
    } catch (e) {
      setStatus(t("app.error", { message: String(e) }));
    }
  }

  function reorderBatch(targetId: string) {
    if (!event) return;
    const batches = reorderById(event.batches, draggedBatchId, targetId);
    if (!batches) return;
    const updated = { ...event, batches };
    setEvent(updated);
    invoke("save_event", { event: updated }).catch(() => {});
  }

  async function deleteCanvasPreset(preset: CanvasPreset) {
    if (!event) return;
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    const yes = await confirm(
      t("app.deleteCanvasConfirm", { name: preset.name }),
      { title: t("app.deleteCanvasTitle"), kind: "warning" }
    );
    if (!yes) return;
    try {
      await invoke("delete_canvas_preset", { eventId: event.id, presetId: preset.id });
      updateEvent({
        ...event,
        canvas_presets: event.canvas_presets.filter((p) => p.id !== preset.id),
      });
    } catch (e) {
      setStatus(t("app.error", { message: String(e) }));
    }
  }

  async function deleteFramePreset(preset: FramePreset) {
    if (!event) return;
    const { confirm } = await import("@tauri-apps/plugin-dialog");
    const yes = await confirm(
      t("app.deleteFrameConfirm", { name: preset.name }),
      { title: t("app.deleteFrameTitle"), kind: "warning" }
    );
    if (!yes) return;
    try {
      await invoke("delete_frame_preset", { eventId: event.id, presetId: preset.id });
      const remaining = event.frame_presets.filter((p) => p.id !== preset.id);
      updateEvent({
        ...event,
        frame_presets: remaining,
        active_frame_preset_id:
          event.active_frame_preset_id === preset.id
            ? remaining[0]?.id ?? null
            : event.active_frame_preset_id,
      });
    } catch (e) {
      setStatus(t("app.error", { message: String(e) }));
    }
  }

  async function addBatch() {
    if (!event) return;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const folder = await open({
        directory: true,
        multiple: false,
        defaultPath: event.root_path ?? undefined,
      });
      if (!folder) return;
      setStatus(t("app.loadingBatch"));
      const updated = await invoke<OrenewEvent>("add_batch", { eventId: event.id, folder });
      updateEvent(updated);
      const newBatch = updated.batches[updated.batches.length - 1];
      if (newBatch) setActiveBatch(newBatch); // seenIdsRef effect seeds its photos to qty 1
      clearSelection();
      setStatus("");
    } catch (e) {
      setStatus(t("app.error", { message: String(e) }));
    }
  }

  function updateEvent(updated: OrenewEvent) {
    setEvent(updated);
    if (activeBatch) {
      const refreshed = updated.batches.find((b) => b.id === activeBatch.id);
      if (refreshed) setActiveBatch(refreshed);
    }
  }

  const totalPhotos = event?.batches.reduce((n, b) => n + b.photos.length, 0) ?? 0;
  const photos = activeBatch?.photos ?? [];
  // When "hide empty" is on, drop photos queued for 0 copies from the grid, then
  // sort by the chosen key/direction (name uses path order, which the backend
  // already produces, so the default is a no-op).
  const filteredPhotos = hideEmpty ? photos.filter((p) => (photoQueue[p.id] ?? 0) > 0) : photos;
  const visiblePhotos = [...filteredPhotos].sort((a, b) => {
    const c =
      sortKey === "name" ? a.path.localeCompare(b.path) :
      sortKey === "size" ? a.size_bytes - b.size_bytes :
      sortKey === "created" ? a.created - b.created :
      a.modified - b.modified;
    return c * sortDir;
  });

  // Batch actions act on the selection, or the whole batch when nothing is selected.
  const targetIds = selectedIds.size > 0 ? [...selectedIds] : photos.map((p) => p.id);
  // Process/totals only count the targeted photos.
  const effectiveQueue = selectedIds.size > 0
    ? Object.fromEntries(Object.entries(photoQueue).filter(([id]) => selectedIds.has(id)))
    : photoQueue;
  const queuedTotal = Object.values(effectiveQueue).reduce((s, q) => s + q, 0);

  // Export indicator: which batches contribute to the (effective) queue + a few
  // thumbnails. The queue is global, so a queued photo can live in any batch —
  // make that scope visible next to the Export button.
  const queuedIds = Object.keys(effectiveQueue).filter((id) => (effectiveQueue[id] ?? 0) > 0);
  const idToPhoto = new Map<string, Photo>();
  const idToBatch = new Map<string, string>();
  for (const b of event?.batches ?? []) {
    for (const p of b.photos) { idToPhoto.set(p.id, p); idToBatch.set(p.id, b.name); }
  }
  const exportBatchCount = new Set(queuedIds.map((id) => idToBatch.get(id)).filter(Boolean)).size;
  const exportThumbs = queuedIds
    .slice(0, 3)
    .map((id) => idToPhoto.get(id))
    .filter((p): p is Photo => !!p)
    .map((p) => ({ path: p.path, hash: p.content_hash }));

  // Suggest per-photo export quantities = number of faces detected. Heavy, so
  // it runs on user click (not automatically) with a live progress count. Scans
  // only the selected photos (or the whole batch if none selected) and merges the
  // result into the queue, leaving non-scanned photos' quantities untouched.
  async function scanFaces() {
    if (!event || !activeBatch) return;
    const ids = selectedIds.size > 0 ? [...selectedIds] : null;
    setScanning(true);
    setScanProgress(null);
    // Progress is shown next to the Suggest-copies button in the gallery sub-bar,
    // not in the app-level status line.
    const unsub = await listen<{ done: number; total: number }>(
      EVENTS.FACE_SCAN_PROGRESS,
      (e) => setScanProgress({ done: e.payload.done, total: e.payload.total })
    );
    try {
      const counts = await invoke<Record<string, number>>("count_faces_in_batch", {
        eventId: event.id,
        batchId: activeBatch.id,
        photoIds: ids,
      });
      // Positive counts only (matches adjustQty's "no zero entries" rule); a
      // scanned photo with 0 faces falls through to qty 0 → dimmed card.
      setPhotoQueue((prev) => {
        const next = { ...prev };
        for (const [id, n] of Object.entries(counts)) {
          if (n > 0) next[id] = n;
          else delete next[id];
        }
        return next;
      });
    } catch (e) {
      setStatus(t("app.error", { message: String(e) }));
    } finally {
      unsub();
      setScanning(false);
      setScanProgress(null);
    }
  }

  // Click selects (plain = replace, ctrl = toggle, shift = range from anchor);
  // `selected` tracks the last click for the preview + shift anchor.
  function handlePhotoClick(photo: Photo, e: React.MouseEvent) {
    const id = photo.id;
    setSelected(photo);
    if (e.shiftKey && anchorRef.current) {
      setSelectedIds(new Set(rangeIds(visiblePhotos, anchorRef.current, id)));
    } else if (e.ctrlKey || e.metaKey) {
      setSelectedIds((prev) => {
        const next = new Set(prev);
        next.has(id) ? next.delete(id) : next.add(id);
        return next;
      });
      anchorRef.current = id;
    } else {
      // Plain click selects just this photo. Double-click opens the full-screen
      // review (see handlePhotoDoubleClick) — single click no longer opens it.
      setSelectedIds(new Set([id]));
      anchorRef.current = id;
    }
  }

  function handlePhotoDoubleClick(photo: Photo) {
    setSelected(photo);
    setLightboxOpen(true);
  }

  // Ctrl/Cmd+A selects every photo currently shown in the grid.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && (e.key === "a" || e.key === "A")) {
        if (!activeBatch) return;
        // Don't hijack select-all inside text fields.
        const t = e.target as HTMLElement | null;
        if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
        e.preventDefault();
        setSelectedIds(new Set(visiblePhotos.map((p) => p.id)));
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [activeBatch, visiblePhotos]);

  // Ctrl+Tab / Ctrl+Shift+Tab cycle through batches (wrap around). Selection +
  // queue persist (combine across batches), same as clicking a tab.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.key !== "Tab") return;
      if (!event || event.batches.length < 2) return;
      e.preventDefault();
      const n = event.batches.length;
      const idx = event.batches.findIndex((b) => b.id === activeBatch?.id);
      const nextIdx = e.shiftKey ? (idx - 1 + n) % n : (idx + 1) % n;
      setActiveBatch(event.batches[nextIdx]);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [event, activeBatch]);

  // Derive uniform qty across the targeted photos — 0 if empty or mixed values.
  const targetQtys = targetIds.map((id) => photoQueue[id] ?? 0);
  const allQty = targetQtys.length > 0 && targetQtys.every((q) => q === targetQtys[0])
    ? targetQtys[0]
    : 0;

  // Keyboard navigation through photos. Works over the grid and in the lightbox;
  // with nothing selected, the first arrow selects the first card and goes from there.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const isArrow = ["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"].includes(e.key);
      if (isArrow) {
        const target = e.target as HTMLElement | null;
        if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) return;
        if (visiblePhotos.length === 0) return;
        e.preventDefault();
        const idx = selected ? visiblePhotos.findIndex((p) => p.id === selected.id) : -1;
        const select = (photo: Photo) => {
          setSelected(photo);
          if (!lightboxOpen) setSelectedIds(new Set([photo.id]));
          anchorRef.current = photo.id;
        };
        if (idx < 0) { select(visiblePhotos[0]); return; }
        const cols = colCountRef.current;
        const clamp = (n: number) => Math.min(visiblePhotos.length - 1, Math.max(0, n));
        // react-window's grid is laid out left-to-right regardless of `dir`, so only
        // the lightbox filmstrip (a dir-aware flex row) needs the RTL left/right flip.
        const rtl = lightboxOpen && i18n.dir() === "rtl";
        const fwd = rtl ? -1 : 1; // ArrowRight delta
        let next = idx;
        if (e.key === "ArrowRight") next = clamp(idx + fwd);
        else if (e.key === "ArrowLeft") next = clamp(idx - fwd);
        else if (e.key === "ArrowDown") next = lightboxOpen ? idx : clamp(idx + cols);
        else if (e.key === "ArrowUp") next = lightboxOpen ? idx : clamp(idx - cols);
        select(visiblePhotos[next]);
        return;
      }
      if (e.key === "Enter" && selected) setLightboxOpen(true);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [selected, visiblePhotos, lightboxOpen]);

  // Escape: close the lightbox, else clear the selection. Kept separate from the
  // arrow/Enter nav effect above (which is gated on `selected`) so Esc still works
  // after Ctrl+A — that sets selectedIds without setting `selected`.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) return;
      if (lightboxOpen) setLightboxOpen(false);
      else clearSelection();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [lightboxOpen]);

  // Grid cell size, shared by the BatchTabs −/+ buttons and the Ctrl+wheel /
  // Ctrl+± handlers above (same clamp 100–280, step 20).
  const zoomCell = (dir: 1 | -1) =>
    setCellSize((c) => Math.min(280, Math.max(100, c + dir * 20)));

  function adjustQty(photoId: string, delta: number) {
    setPhotoQueue((prev) => {
      const next = Math.max(0, (prev[photoId] ?? 0) + delta);
      const updated = { ...prev };
      if (next === 0) delete updated[photoId];
      else updated[photoId] = next;
      return updated;
    });
  }

  // Set the queued qty for the targeted photos (selection, or whole batch when
  // nothing is selected); other photos' quantities are left untouched.
  function handleSetAllQty(qty: number) {
    if (!activeBatch) return;
    setPhotoQueue((prev) => {
      const next = { ...prev };
      for (const id of targetIds) {
        if (qty <= 0) delete next[id];
        else next[id] = qty;
      }
      return next;
    });
  }

  // Set (or clear, with `null`) a photo's orientation override and mirror the
  // change into event/activeBatch/selected state.
  async function setOrientation(photoId: string, orientation: Orientation | null) {
    if (!event) return;
    try {
      if (orientation) {
        await invoke("set_orientation_override", { eventId: event.id, photoId, orientation });
      } else {
        await invoke("clear_orientation_override", { eventId: event.id, photoId });
      }
      const updatePhoto = (p: Photo): Photo =>
        p.id === photoId ? { ...p, orientation_override: orientation } : p;
      const updatedEvent = {
        ...event,
        batches: event.batches.map((b) => ({ ...b, photos: b.photos.map(updatePhoto) })),
      };
      setEvent(updatedEvent);
      if (activeBatch) {
        const refreshedBatch = updatedEvent.batches.find((b) => b.id === activeBatch.id);
        if (refreshedBatch) setActiveBatch(refreshedBatch);
      }
      setSelected((prev) => (prev?.id === photoId ? updatePhoto(prev) : prev));
      // Orientation changes the crop ratio/frame, so force the preview to refetch.
      setFrameNonce((n) => n + 1);
    } catch (e) {
      setStatus(t("app.error", { message: String(e) }));
    }
  }

  // After exporting: optimistically bump counts for processed photos and clear
  // only those from the queue (`queue` is the effective, possibly selection-scoped set).
  function handleExported(destination: "print" | "save", queue: Record<string, number>) {
    const bump = (p: Photo): Photo => {
      const qty = queue[p.id] ?? 0;
      if (!qty) return p;
      return destination === "print"
        ? { ...p, print_count: p.print_count + qty }
        : { ...p, save_count: p.save_count + qty };
    };
    setEvent((prev) =>
      prev
        ? { ...prev, batches: prev.batches.map((b) => ({ ...b, photos: b.photos.map(bump) })) }
        : prev
    );
    setActiveBatch((prev) => (prev ? { ...prev, photos: prev.photos.map(bump) } : prev));
    setPhotoQueue((prev) => {
      const next = { ...prev };
      for (const id of Object.keys(queue)) delete next[id];
      return next;
    });
  }

  // Lightbox prev/next over the currently visible photos.
  const selIdx = selected ? visiblePhotos.findIndex((p) => p.id === selected.id) : -1;
  function goAdjacent(dir: -1 | 1) {
    if (selIdx < 0) return;
    const next = visiblePhotos[selIdx + dir];
    if (!next) return;
    setSelected(next);
    setSelectedIds(new Set([next.id]));
    anchorRef.current = next.id;
  }

  return (
    <div className="flex flex-col h-screen bg-neutral-950 text-neutral-100 select-none">
      <Toolbar
        event={event}
        entitlement={entitlement}
        status={status}
        totalPhotos={totalPhotos}
        onOpenEvent={openEvent}
        onConfigureEvent={() => setModal("eventConfig")}
        onDeleteEvent={deleteEvent}
        onSettings={() => setModal("settings")}
      />

      {event ? (
        <>
          <BatchTabs
            event={event}
            activeBatch={activeBatch}
            draggedBatchId={draggedBatchId}
            setDraggedBatchId={setDraggedBatchId}
            onAddBatch={addBatch}
            onSelectBatch={(b) => { setActiveBatch(b); /* keep selection + photoQueue; counts combine across batches */ }}
            onDeleteBatch={deleteBatch}
            onReorderBatch={reorderBatch}
            hideEmpty={hideEmpty}
            onToggleHideEmpty={() => setHideEmpty((v) => !v)}
            cellSize={cellSize}
            onZoom={zoomCell}
            sortKey={sortKey}
            sortDir={sortDir}
            onSortKey={setSortKey}
            onToggleSortDir={() => setSortDir((d) => (d === 1 ? -1 : 1))}
          />

          <div className="flex flex-1 overflow-hidden">
            <Gallery
              photos={visiblePhotos}
              selectedId={selected?.id ?? null}
              selectedIds={selectedIds}
              onPhotoClick={handlePhotoClick}
              onPhotoDoubleClick={handlePhotoDoubleClick}
              onBackgroundClick={clearSelection}
              photoQueue={photoQueue}
              onQtyDelta={adjustQty}
              cellSize={cellSize}
              onColCountChange={(n) => { colCountRef.current = n; }}
            />
          </div>

          {activeBatch && activeBatch.photos.length > 0 && (
            <ActionBar
              queuedTotal={queuedTotal}
              visibleCount={visiblePhotos.length}
              selectedCount={selectedIds.size}
              allQty={allQty}
              scanning={scanning}
              scanProgress={scanProgress}
              exportBatchCount={exportBatchCount}
              exportThumbs={exportThumbs}
              onSetAllQty={handleSetAllQty}
              onScanFaces={scanFaces}
              onClearSelection={clearSelection}
              onExport={() => setModal("export")}
            />
          )}

          {lightboxOpen && selected && (
            <Lightbox
              event={event}
              photo={selected}
              onClose={() => setLightboxOpen(false)}
              frameNonce={frameNonce}
              onOrientationOverride={(id, o) => setOrientation(id, o)}
              onClearOrientationOverride={(id) => setOrientation(id, null)}
              onPrev={() => goAdjacent(-1)}
              onNext={() => goAdjacent(1)}
              hasPrev={selIdx > 0}
              hasNext={selIdx >= 0 && selIdx < visiblePhotos.length - 1}
              qty={photoQueue[selected.id] ?? 0}
              onQtyDelta={adjustQty}
              photos={visiblePhotos}
              onJump={(p) => { setSelected(p); setSelectedIds(new Set([p.id])); anchorRef.current = p.id; }}
            />
          )}
        </>
      ) : (
        <EmptyState onOpen={openEvent} />
      )}

      {/* Modals */}
      {modal === "export" && event && (
        <ExportDialog
          event={event}
          photoQueue={effectiveQueue}
          onClose={() => setModal(null)}
          onEventUpdate={updateEvent}
          onExported={handleExported}
          onAddFrame={() => setAddingFrame(true)}
          onEditFrame={setEditingFrame}
          onDeleteFrame={deleteFramePreset}
          onAddCanvas={() => setAddingCanvas(true)}
          onEditCanvas={setEditingCanvas}
          onDeleteCanvas={deleteCanvasPreset}
        />
      )}
      {addingFrame && event && (
        <FramePresetDialog
          event={event}
          onCreated={(updatedEvent) => {
            updateEvent(updatedEvent);
            invoke("sync_watches", { eventId: updatedEvent.id }).catch(() => {});
            setAddingFrame(false);
          }}
          onClose={() => setAddingFrame(false)}
        />
      )}
      {modal === "eventConfig" && event && (
        <EventConfigDialog
          event={event}
          onClose={() => setModal(null)}
          onEventUpdate={updateEvent}
          onAddFrame={() => setAddingFrame(true)}
          onEditFrame={setEditingFrame}
          onDeleteFrame={deleteFramePreset}
          onAddCanvas={() => setAddingCanvas(true)}
          onEditCanvas={setEditingCanvas}
          onDeleteCanvas={deleteCanvasPreset}
        />
      )}
      {modal === "settings" && (
        <SettingsDialog
          entitlement={entitlement}
          onClose={() => setModal(null)}
          onEntitlementChange={setEntitlement}
          onAuthResult={handleAuthResult}
          onManageDevices={async () => {
            try {
              const list = await listDevices();
              setModal(null);
              setDevicePicker({ mode: "manage", devices: list ?? [] });
            } catch {}
          }}
        />
      )}
      {devicePicker && (
        <DeviceManagerDialog
          mode={devicePicker.mode}
          initialDevices={devicePicker.devices}
          currentHash={deviceHash}
          onResolved={setEntitlement}
          onClose={() => setDevicePicker(null)}
        />
      )}
      {addingCanvas && event && (
        <Modal onClose={() => setAddingCanvas(false)}>
          <CanvasPresetForm
            event={event}
            onCreated={(_preset, updatedEvent) => { updateEvent(updatedEvent); setAddingCanvas(false); }}
            onCancel={() => setAddingCanvas(false)}
          />
        </Modal>
      )}
      {editingCanvas && event && (
        <Modal onClose={() => setEditingCanvas(null)}>
          <CanvasPresetForm
            event={event}
            editing={editingCanvas}
            onCreated={(_preset, updatedEvent) => { updateEvent(updatedEvent); setEditingCanvas(null); }}
            onCancel={() => setEditingCanvas(null)}
          />
        </Modal>
      )}
      {editingFrame && event && (
        <FramePresetDialog
          event={event}
          editing={editingFrame}
          onCreated={(updatedEvent) => {
            updateEvent(updatedEvent);
            invoke("sync_watches", { eventId: updatedEvent.id }).catch(() => {});
            setFrameNonce((n) => n + 1);
            setEditingFrame(null);
          }}
          onClose={() => setEditingFrame(null)}
        />
      )}
    </div>
  );
}
