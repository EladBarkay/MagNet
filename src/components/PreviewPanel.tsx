import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useThumbnail } from "../hooks/useThumbnail";
import { useFramedPreview } from "../hooks/useFramedPreview";
import { MagnetEvent, Orientation, Photo } from "../types";
import { basename } from "../lib/paths";

type Props = {
  event: MagnetEvent;
  photo: Photo;
  onClose: () => void;
  /** Bumped when a frame PNG changes on disk, to force preview refetch. */
  frameNonce: number;
  onOrientationOverride: (photoId: string, orientation: Orientation) => void;
  onClearOrientationOverride: (photoId: string) => void;
  width?: number;
};

export default function PreviewPanel({ event, photo, onClose, frameNonce, onOrientationOverride, onClearOrientationOverride, width }: Props) {
  const { t } = useTranslation();
  const filename = basename(photo.path);
  const thumb = useThumbnail(photo.path, photo.content_hash);
  const [previewFrameId, setPreviewFrameId] = useState<string | null>(
    event.active_frame_preset_id
  );

  // Sync when the event's active frame changes (e.g. user adds/removes a preset).
  useEffect(() => {
    setPreviewFrameId(event.active_frame_preset_id);
  }, [event.active_frame_preset_id]);

  const framedSrc = useFramedPreview(
    event.id,
    photo.id,
    previewFrameId,
    frameNonce,
    photo.content_hash
  );

  const displaySrc = framedSrc ?? thumb;
  const naturalOrientation = inferOrientation(photo);
  const orientation = photo.orientation_override ?? naturalOrientation;

  function handleOrientClick(o: Orientation) {
    if (o === naturalOrientation && photo.orientation_override !== null) {
      onClearOrientationOverride(photo.id);
    } else {
      onOrientationOverride(photo.id, o);
    }
  }

  return (
    <aside
      style={{ width: width ?? 288 }}
      className="flex flex-col bg-neutral-800 border-s border-neutral-700 overflow-hidden shrink-0"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-neutral-700">
        <span className="text-xs font-medium text-neutral-300 truncate">{filename}</span>
        <button
          onClick={onClose}
          className="ms-2 text-neutral-500 hover:text-neutral-200 text-lg leading-none flex-shrink-0"
          aria-label={t("preview.closePreview")}
        >
          ×
        </button>
      </div>

      {/* Image */}
      <div className="flex-1 flex items-center justify-center bg-neutral-900 p-3 overflow-hidden">
        {displaySrc ? (
          <img
            src={displaySrc}
            alt={filename}
            className="max-w-full max-h-full object-contain rounded shadow-lg"
            draggable={false}
          />
        ) : (
          <div className="w-full aspect-square bg-neutral-800 rounded animate-pulse" />
        )}
      </div>

      {/* Metadata + controls */}
      <div className="px-3 py-3 border-t border-neutral-700 space-y-2 text-xs text-neutral-400">
        <Row label={t("preview.dimensions")} value={`${photo.width} × ${photo.height}`} />

        {/* Orientation override */}
        <div className="flex items-center justify-between">
          <span>{photo.orientation_override ? t("preview.orientationOverride") : t("preview.orientation")}</span>
          <div className="flex gap-1">
            <OrientBtn
              label={t("preview.landscapeShort")}
              title={t("preview.landscape")}
              active={orientation === "landscape"}
              onClick={() => handleOrientClick("landscape")}
            />
            <OrientBtn
              label={t("preview.portraitShort")}
              title={t("preview.portrait")}
              active={orientation === "portrait"}
              onClick={() => handleOrientClick("portrait")}
            />
          </div>
        </div>

        {event.frame_presets.length > 0 && (
          <div className="flex items-center justify-between gap-2">
            <span>{t("preview.frame")}</span>
            <select
              value={previewFrameId ?? ""}
              onChange={(e) => setPreviewFrameId(e.target.value || null)}
              className="text-xs bg-neutral-700 text-neutral-200 rounded px-1.5 py-0.5 border border-neutral-600 max-w-[140px] truncate"
            >
              <option value="">{t("preview.none")}</option>
              {event.frame_presets.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </div>
        )}
        <div className="flex items-center justify-between">
          <span>{t("preview.saved")}</span>
          <span
            className={
              photo.save_count > 0
                ? "font-semibold text-blue-400"
                : "text-neutral-600"
            }
          >
            {photo.save_count === 0 ? t("preview.notSaved") : `⬇${photo.save_count}`}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span>{t("preview.printed")}</span>
          <span
            className={
              photo.print_count > 0
                ? "font-semibold text-green-400"
                : "text-neutral-600"
            }
          >
            {photo.print_count === 0 ? t("preview.notPrinted") : `×${photo.print_count}`}
          </span>
        </div>
      </div>
    </aside>
  );
}

function OrientBtn({
  label, title, active, onClick,
}: { label: string; title: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={[
        "w-6 h-6 text-[10px] font-semibold rounded transition-colors",
        active
          ? "bg-blue-600 text-white"
          : "bg-neutral-700 hover:bg-neutral-600 text-neutral-400",
      ].join(" ")}
    >
      {label}
    </button>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between">
      <span>{label}</span>
      <span className="text-neutral-200">{value}</span>
    </div>
  );
}

function inferOrientation(photo: Photo): Orientation {
  return photo.width >= photo.height ? "landscape" : "portrait";
}

