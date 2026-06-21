import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { check } from "@tauri-apps/plugin-updater";

// ponytail: best-effort silent update check once at startup. No custom UI — a
// native confirm to install, then ask the user to relaunch (avoids pulling in
// plugin-process just to call relaunch()). Add a progress bar / auto-relaunch
// only if users ask for it.
export function useUpdater() {
  const { t } = useTranslation();
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const update = await check();
        if (!update || cancelled) return;
        if (!window.confirm(t("updater.available", { version: update.version }))) return;
        await update.downloadAndInstall();
        window.alert(t("updater.installed"));
      } catch {
        // Offline, or updater not configured (placeholder endpoint): ignore.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [t]);
}
