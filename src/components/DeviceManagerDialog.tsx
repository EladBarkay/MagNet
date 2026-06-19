import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AuthResult, Device, Entitlement } from "../types";
import { disconnectDevice, listDevices } from "../lib/auth";
import { useAsyncForm } from "../hooks/useAsyncForm";
import { Modal } from "./ui";

type Props = {
  /** "limit": sign-in is blocked until a seat is freed. "manage": review devices. */
  mode: "limit" | "manage";
  initialDevices: Device[];
  /** Hash of the machine this app instance runs on, to mark "this device". */
  currentHash: string;
  /** Called when provisioning resolves to a signed-in entitlement. */
  onResolved: (entitlement: Entitlement) => void;
  onClose: () => void;
};

export default function DeviceManagerDialog({
  mode,
  initialDevices,
  currentHash,
  onResolved,
  onClose,
}: Props) {
  const { t } = useTranslation();
  const [devices, setDevices] = useState<Device[]>(initialDevices);
  const { error, loading: busy, run } = useAsyncForm();

  function handleResult(r: AuthResult) {
    if (r.kind === "entitlement") {
      onResolved(r);
      if (mode === "limit") onClose();
    } else {
      setDevices(r.devices);
    }
  }

  function disconnect(hash: string) {
    run(async () => {
      const result = await disconnectDevice(hash);
      handleResult(result);
      if (mode === "manage" && result.kind === "entitlement") {
        // Refresh the list so the freed seat disappears.
        const list = await listDevices();
        setDevices(list ?? []);
      }
    });
  }

  return (
    <Modal onClose={onClose}>
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">
          {mode === "limit" ? t("devices.limitTitle") : t("devices.manageTitle")}
        </h2>
        <p className="text-xs text-neutral-400">
          {mode === "limit" ? t("devices.limitHelp") : t("devices.manageHelp")}
        </p>

        <ul className="space-y-2">
          {devices.map((d) => {
            const isCurrent = d.device_hash === currentHash;
            return (
              <li
                key={d.device_hash}
                className="flex items-center justify-between gap-3 rounded-lg bg-neutral-800 px-3 py-2"
              >
                <div className="min-w-0">
                  <p className="truncate text-sm text-neutral-100">
                    {d.device_label || t("devices.unknownDevice")}
                    {isCurrent && (
                      <span className="ms-2 text-[10px] uppercase tracking-wide text-accent">
                        {t("devices.thisDevice")}
                      </span>
                    )}
                  </p>
                  {d.last_seen && (
                    <p className="text-xs text-neutral-500">
                      {t("devices.lastActive", { date: new Date(d.last_seen).toLocaleString() })}
                    </p>
                  )}
                </div>
                {/* Disconnecting "this device" would just re-register it (a seat
                    frees, then we re-mint for the same machine) — use Sign out
                    instead, so hide the action here. */}
                {!isCurrent && (
                  <button
                    onClick={() => disconnect(d.device_hash)}
                    disabled={busy}
                    className="shrink-0 text-xs text-red-400 hover:text-red-300 disabled:opacity-40"
                  >
                    {t("devices.disconnect")}
                  </button>
                )}
              </li>
            );
          })}
          {devices.length === 0 && (
            <li className="text-xs text-neutral-500">{t("devices.none")}</li>
          )}
        </ul>

        {error && <p className="text-xs text-red-400">{error}</p>}

        <div className="flex justify-end pt-1">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-neutral-400 hover:text-neutral-200"
          >
            {mode === "limit" ? t("common.cancel") : t("common.close")}
          </button>
        </div>
      </div>
    </Modal>
  );
}
