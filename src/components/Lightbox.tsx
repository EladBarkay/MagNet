import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useThumbnail } from "../hooks/useThumbnail";
import { useFramedPreview } from "../hooks/useFramedPreview";
import { MagnetEvent, Orientation, Photo } from "../types";
import { basename } from "../lib/paths";
import { QtyButton } from "./ui";

type Props = {
  event: MagnetEvent;
  photo: Photo;
  onClose: () => void;
  /** Bumped when a frame PNG changes on disk, to force preview refetch. */
  frameNonce: number;
  onOrientationOverride: (photoId: string, orientation: Orientation) => void;
  onClearOrientationOverride: (photoId: string) => void;
  onPrev: () => void;
  onNext: () => void;
  hasPrev: boolean;
  hasNext: boolean;
  qty: number;
  onQtyDelta: (photoId: string, delta: number) => void;
  /** The photos being reviewed (the visible grid) — drives the filmstrip. */
  photos: Photo[];
  /** Jump straight to a photo (filmstrip click). */
  onJump: (photo: Photo) => void;
};

/**
 * Full-screen photo review. Replaces the old docked preview panel: gives the
 * framed preview the whole window, with prev/next nav and the same controls
 * (orientation override, frame, copies, save/print counts). Arrow keys + Esc are
 * handled by the parent (App) so they keep working over the grid too.
 */
export default function Lightbox({
  event, photo, onClose, frameNonce,
  onOrientationOverride, onClearOrientationOverride,
  onPrev, onNext, hasPrev, hasNext, qty, onQtyDelta,
  photos, onJump,
}: Props) {
  const { t } = useTranslation();
  const filename = basename(photo.path);
  const thumb = useThumbnail(photo.path, photo.content_hash);
  const [previewFrameId, setPreviewFrameId] = useState<string | null>(event.active_frame_preset_id);

  useEffect(() => {
    setPreviewFrameId(event.active_frame_preset_id);
  }, [event.active_frame_preset_id]);

  const framedSrc = useFramedPreview(event.id, photo.id, previewFrameId, frameNonce, photo.content_hash);
  const displaySrc = framedSrc ?? thumb;

  // Warm the framed preview for the ±2 neighbors so left/right nav feels instant
  // (backend caches per (photo_id, preset_id)). Filmstrip shows a wider window.
  const currentIndex = photos.findIndex((p) => p.id === photo.id);
  const prefetchIds = [-2, -1, 1, 2]
    .map((d) => photos[currentIndex + d])
    .filter((p): p is Photo => !!p);
  const stripStart = Math.max(0, currentIndex - 3);
  const stripPhotos = photos.slice(stripStart, currentIndex + 4);
  const naturalOrientation = inferOrientation(photo);
  const orientation = photo.orientation_override ?? naturalOrientation;

  function handleOrientClick(o: Orientation) {
    if (o === naturalOrientation && photo.orientation_override !== null) onClearOrientationOverride(photo.id);
    else onOrientationOverride(photo.id, o);
  }

  return (
    <div className="fixed inset-0 z-40 flex flex-col bg-black/90 backdrop-blur-sm">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 shrink-0">
        <span className="text-sm font-medium text-neutral-300 truncate">{filename}</span>
        <button
          onClick={onClose}
          className="ms-2 text-neutral-400 hover:text-white text-2xl leading-none shrink-0"
          aria-label={t("preview.closePreview")}
        >
          ×
        </button>
      </div>

      {/* Hidden prefetchers for neighbor framed previews (render nothing). */}
      {prefetchIds.map((p) => (
        <PreviewPrefetch key={p.id} eventId={event.id} photoId={p.id} presetId={previewFrameId} frameNonce={frameNonce} hash={p.content_hash} />
      ))}

      {/* Image + nav arrows. Clicking the empty margin beside the image closes. */}
      <div className="flex-1 flex items-center gap-2 px-2 min-h-0">
        <NavBtn dir="prev" onClick={onPrev} disabled={!hasPrev} />
        <div className="flex-1 h-full flex items-center justify-center min-h-0" onClick={onClose}>
          {displaySrc ? (
            <img
              src={displaySrc}
              alt={filename}
              onClick={(e) => e.stopPropagation()}
              className="max-w-full max-h-full object-contain rounded-lg shadow-2xl"
              draggable={false}
            />
          ) : (
            <div className="w-80 aspect-[4/3] bg-neutral-800 rounded-lg animate-pulse" />
          )}
        </div>
        <NavBtn dir="next" onClick={onNext} disabled={!hasNext} />
      </div>

      {/* Filmstrip: neighbors around the current photo, so the user can see what's
          left/right and jump directly. */}
      {stripPhotos.length > 1 && (
        <div className="shrink-0 flex items-center justify-center gap-1.5 px-4 py-2 overflow-x-auto">
          {stripPhotos.map((p) => (
            <FilmstripThumb
              key={p.id}
              photo={p}
              active={p.id === photo.id}
              onClick={() => onJump(p)}
            />
          ))}
        </div>
      )}

      {/* Controls strip */}
      <div className="shrink-0 flex flex-wrap items-center justify-center gap-x-6 gap-y-2 px-4 py-3 text-xs text-neutral-400">
        {/* Copies */}
        <div className="flex items-center gap-2">
          <span>{t("galleryToolbar.copies")}</span>
          <div className="flex items-center gap-0.5 rounded-full bg-white/10 px-0.5 py-0.5">
            <QtyButton size="sm" label="−" onClick={() => onQtyDelta(photo.id, -1)} disabled={qty <= 0} />
            <span className="min-w-[20px] text-center text-sm font-semibold text-neutral-100 tabular-nums">{qty}</span>
            <QtyButton size="sm" label="+" onClick={() => onQtyDelta(photo.id, 1)} />
          </div>
        </div>

        {/* Orientation */}
        <div className="flex items-center gap-2">
          <span>{photo.orientation_override ? t("preview.orientationOverride") : t("preview.orientation")}</span>
          <div className="flex gap-1">
            <OrientBtn label={t("preview.landscapeShort")} title={t("preview.landscape")} active={orientation === "landscape"} onClick={() => handleOrientClick("landscape")} />
            <OrientBtn label={t("preview.portraitShort")} title={t("preview.portrait")} active={orientation === "portrait"} onClick={() => handleOrientClick("portrait")} />
          </div>
        </div>

        {/* Frame */}
        {event.frame_presets.length > 0 && (
          <div className="flex items-center gap-2">
            <span>{t("preview.frame")}</span>
            <select
              value={previewFrameId ?? ""}
              onChange={(e) => setPreviewFrameId(e.target.value || null)}
              className="text-xs bg-neutral-800 text-neutral-200 rounded px-1.5 py-0.5 border border-neutral-700 max-w-[140px] truncate"
            >
              <option value="">{t("preview.none")}</option>
              {event.frame_presets.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </div>
        )}

        {/* Dimensions + counts */}
        <span className="text-neutral-500">{photo.width} × {photo.height}</span>
        <span className={photo.save_count > 0 ? "font-semibold text-accent" : "text-neutral-600"}>
          {photo.save_count === 0 ? t("preview.notSaved") : `⬇${photo.save_count}`}
        </span>
        <span className={photo.print_count > 0 ? "font-semibold text-green-400" : "text-neutral-600"}>
          {photo.print_count === 0 ? t("preview.notPrinted") : `×${photo.print_count}`}
        </span>
      </div>
    </div>
  );
}

function NavBtn({ dir, onClick, disabled }: { dir: "prev" | "next"; onClick: () => void; disabled: boolean }) {
  // Chevron points by visual direction; under RTL the start/end flip is handled by
  // the parent's prev/next wiring, so keep the glyphs purely visual here.
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className="shrink-0 w-12 h-12 flex items-center justify-center rounded-full text-2xl text-neutral-300 bg-white/5 hover:bg-white/15 disabled:opacity-20 disabled:cursor-default transition-colors"
      aria-label={dir}
    >
      {dir === "prev" ? "‹" : "›"}
    </button>
  );
}

function OrientBtn({ label, title, active, onClick }: { label: string; title: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={[
        "w-6 h-6 text-[10px] font-semibold rounded transition-colors",
        active ? "bg-accent text-accent-fg" : "bg-neutral-800 hover:bg-neutral-700 text-neutral-400",
      ].join(" ")}
    >
      {label}
    </button>
  );
}

// Warms the framed-preview cache for one photo, renders nothing. Mounted for the
// current photo's neighbors so navigating to them hits the cache.
function PreviewPrefetch({ eventId, photoId, presetId, frameNonce, hash }: {
  eventId: string; photoId: string; presetId: string | null; frameNonce: number; hash: string;
}) {
  useFramedPreview(eventId, photoId, presetId, frameNonce, hash);
  return null;
}

function FilmstripThumb({ photo, active, onClick }: { photo: Photo; active: boolean; onClick: () => void }) {
  const thumb = useThumbnail(photo.path, photo.content_hash);
  return (
    <button
      onClick={onClick}
      className={[
        "shrink-0 w-14 h-14 rounded-md overflow-hidden transition-all bg-neutral-800",
        active ? "ring-2 ring-accent" : "opacity-60 hover:opacity-100",
      ].join(" ")}
      aria-current={active}
    >
      {thumb && <img src={thumb} alt="" className="w-full h-full object-cover" draggable={false} />}
    </button>
  );
}

function inferOrientation(photo: Photo): Orientation {
  return photo.width >= photo.height ? "landscape" : "portrait";
}
