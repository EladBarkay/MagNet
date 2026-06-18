import { useTranslation } from "react-i18next";
import { Entitlement, MagnetEvent } from "../types";
import { PrintIcon, SettingsIcon, TrashIcon } from "./icons";
import { tierLabel, tierColor } from "../lib/tiers";

type Props = {
  event: MagnetEvent | null;
  entitlement: Entitlement | null;
  status: string;
  totalPhotos: number;
  queuedTotal: number;
  onOpenEvent: () => void;
  onDeleteEvent: () => void;
  onExport: () => void;
  onSettings: () => void;
};

export default function Toolbar({
  event, entitlement, status, totalPhotos, queuedTotal,
  onOpenEvent, onDeleteEvent, onExport, onSettings,
}: Props) {
  const { t } = useTranslation();
  const tier = entitlement?.tier ?? "free";

  return (
    <header className="flex items-center gap-3 px-4 py-2.5 bg-neutral-800 border-b border-neutral-700 shrink-0">
      <span className="font-bold text-base tracking-tight text-white">MagNet</span>
      <div className="w-px h-4 bg-neutral-600" />
      <button onClick={onOpenEvent}
        className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 active:bg-blue-700 rounded text-sm font-medium transition-colors">
        {t("toolbar.openEvent")}
      </button>

      {event && (
        <>
          <span className="text-neutral-300 text-sm font-medium">{event.name}</span>
          <span className="text-neutral-500 text-xs">{t("common.photos", { count: totalPhotos })}</span>

          <button
            onClick={onDeleteEvent}
            title={t("toolbar.deleteEvent")}
            className="text-neutral-600 hover:text-red-400 transition-colors"
          >
            <TrashIcon />
          </button>

          <div className="ms-auto flex items-center gap-3">
            <button
              onClick={onExport}
              disabled={queuedTotal === 0}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 disabled:opacity-40 disabled:cursor-not-allowed rounded text-sm font-medium transition-colors"
              title={queuedTotal === 0 ? t("toolbar.setQuantitiesFirst") : ""}
            >
              <PrintIcon />
              {queuedTotal > 0 ? t("toolbar.exportCount", { count: queuedTotal }) : t("toolbar.export")}
            </button>
          </div>
        </>
      )}

      {status && (
        <span className={["text-xs", event ? "" : "ms-auto", "text-neutral-400"].join(" ")}>
          {status}
        </span>
      )}

      <button
        onClick={onSettings}
        title={t("toolbar.settingsLicense")}
        className={[
          "flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs transition-colors",
          event ? "" : "ms-auto",
          "text-neutral-400 hover:text-neutral-100 hover:bg-neutral-700",
        ].join(" ")}
      >
        <SettingsIcon />
        <span className={`font-semibold px-1.5 py-0.5 rounded-full text-[10px] ${tierColor(tier)}`}>
          {t(`tier.${tier}`, { defaultValue: tierLabel(tier) })}
        </span>
      </button>
    </header>
  );
}
