import { useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { AuthResult, Entitlement } from "../types";
import { supabase } from "../lib/supabase";
import { establishFromSession } from "../lib/auth";
import { tierLabel, tierColor } from "../lib/tiers";
import { LANGS, setLanguage, type LangCode } from "../i18n";
import { Modal } from "./ui";
import { RefreshIcon } from "./icons";
import { useAsyncForm } from "../hooks/useAsyncForm";

type Props = {
  entitlement: Entitlement | null;
  onClose: () => void;
  onEntitlementChange: (entitlement: Entitlement | null) => void;
  /** Handle an auth/provision outcome (signed in, or device-limit prompt). */
  onAuthResult: (result: AuthResult) => void;
  /** Open the "manage devices" picker. */
  onManageDevices: () => void;
};

const BUY_URL = "https://magnet.app/pricing";

export default function SettingsDialog({
  entitlement,
  onClose,
  onEntitlementChange,
  onAuthResult,
  onManageDevices,
}: Props) {
  const { t, i18n } = useTranslation();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const { error, setError, loading: busy, run } = useAsyncForm();

  const tier = entitlement?.tier ?? "free";
  const isSignedIn = entitlement !== null && !!entitlement.email;

  async function signInPassword() {
    if (!email.trim() || !password) {
      setError(t("settings.enterCredentials"));
      return;
    }
    await run(async () => {
      const { data, error } = await supabase.auth.signInWithPassword({
        email: email.trim(),
        password,
      });
      if (error || !data.session) throw new Error(error?.message ?? t("settings.signInFailed"));
      const result = await establishFromSession(data.session);
      onAuthResult(result);
      setEmail("");
      setPassword("");
    });
  }

  async function signInOAuth(provider: "google" | "facebook") {
    await run(async () => {
      const { data, error } = await supabase.auth.signInWithOAuth({
        provider,
        options: {
          redirectTo: "magnetapp://auth-callback",
          skipBrowserRedirect: true,
        },
      });
      if (error || !data.url) throw new Error(error?.message ?? t("settings.couldNotStartSignIn"));
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(data.url);
      // The deep-link handler (useAuthDeepLink) completes sign-in on callback.
    });
  }

  async function signOut() {
    await run(async () => {
      await invoke("sign_out");
      onEntitlementChange(null);
    });
  }

  async function refreshLicense() {
    await run(async () => {
      const result = await invoke<AuthResult | null>("refresh_entitlement");
      if (!result) onEntitlementChange(null);
      else onAuthResult(result);
    });
  }

  async function openBuyPage() {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(BUY_URL);
    } catch {}
  }

  return (
    <Modal onClose={onClose}>
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">{t("settings.title")}</h2>

        {/* Language */}
        <div className="flex items-center justify-between gap-2">
          <span className="text-xs font-medium text-neutral-400">{t("settings.language")}</span>
          <select
            value={i18n.language}
            onChange={(e) => setLanguage(e.target.value as LangCode)}
            className="text-sm bg-neutral-700 text-neutral-100 rounded px-2 py-1 border border-neutral-600 focus:outline-none focus:ring-1 focus:ring-accent"
          >
            {LANGS.map((l) => (
              <option key={l.code} value={l.code}>{l.label}</option>
            ))}
          </select>
        </div>

        {/* Tier status card */}
        <div className="rounded-lg bg-neutral-800 p-3 space-y-1.5">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium uppercase tracking-wide text-neutral-400">
              {t("settings.license")}
            </span>
            <div className="flex items-center gap-1.5">
              <span className={`text-xs font-semibold px-2 py-0.5 rounded-full ${tierColor(tier)}`}>
                {t(`tier.${tier}`, { defaultValue: tierLabel(tier) })}
              </span>
              <button
                onClick={refreshLicense}
                disabled={busy}
                title={t("settings.refreshLicense")}
                className="text-neutral-500 hover:text-neutral-200 disabled:opacity-40 disabled:cursor-wait transition-colors"
              >
                <RefreshIcon className={`w-3.5 h-3.5 ${busy ? "animate-spin" : ""}`} />
              </button>
            </div>
          </div>
          {isSignedIn ? (
            <div className="text-xs text-neutral-400 space-y-0.5">
              <p>{entitlement!.email}</p>
              <p className="text-neutral-500">
                {tier === "free"
                  ? t("settings.noSubscription")
                  : entitlement!.expires_at
                  ? t("settings.expires", { date: entitlement!.expires_at })
                  : t("settings.activeSubscription")}
              </p>
            </div>
          ) : (
            <p className="text-xs text-neutral-500">
              {t("settings.freeTierNote")}
            </p>
          )}
        </div>

        {isSignedIn ? (
          /* Signed in: manage / sign out */
          <div className="flex flex-col gap-2">
            {tier === "free" && (
              <button
                onClick={openBuyPage}
                className="text-xs text-accent hover:text-accent-hover text-start"
              >
                {t("settings.buyLicense")}
              </button>
            )}
            {tier !== "free" && (
              <button
                onClick={onManageDevices}
                className="text-xs text-accent hover:text-accent-hover text-start"
              >
                {t("devices.manageLink")}
              </button>
            )}
            <button
              onClick={signOut}
              disabled={busy}
              className="text-xs text-red-400 hover:text-red-300 disabled:opacity-40 text-start"
            >
              {busy ? t("settings.signingOut") : t("settings.signOut")}
            </button>
          </div>
        ) : (
          /* Signed out: sign-in form */
          <div className="space-y-3">
            <div className="space-y-2">
              <label className="block text-xs font-medium text-neutral-400">
                {t("settings.email")}
                <input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && signInPassword()}
                  placeholder={t("settings.emailPlaceholder")}
                  autoFocus
                  className="mt-1 w-full bg-neutral-700 rounded px-2 py-1.5 text-sm text-neutral-100 focus:outline-none focus:ring-1 focus:ring-accent"
                />
              </label>
              <label className="block text-xs font-medium text-neutral-400">
                {t("settings.password")}
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && signInPassword()}
                  placeholder={t("settings.passwordPlaceholder")}
                  className="mt-1 w-full bg-neutral-700 rounded px-2 py-1.5 text-sm text-neutral-100 focus:outline-none focus:ring-1 focus:ring-accent"
                />
              </label>
            </div>

            <div className="flex items-center gap-2">
              <div className="h-px flex-1 bg-neutral-700" />
              <span className="text-[10px] uppercase tracking-wide text-neutral-500">{t("settings.or")}</span>
              <div className="h-px flex-1 bg-neutral-700" />
            </div>

            <div className="flex flex-col gap-2">
              <button
                onClick={() => signInOAuth("google")}
                disabled={busy}
                className="w-full px-3 py-1.5 text-sm bg-neutral-700 hover:bg-neutral-600 disabled:opacity-40 rounded font-medium"
              >
                {t("settings.continueGoogle")}
              </button>
              <button
                onClick={() => signInOAuth("facebook")}
                disabled={busy}
                className="w-full px-3 py-1.5 text-sm bg-neutral-700 hover:bg-neutral-600 disabled:opacity-40 rounded font-medium"
              >
                {t("settings.continueFacebook")}
              </button>
            </div>

            <p className="text-xs text-neutral-500">
              {t("settings.noAccount")}{" "}
              <button onClick={openBuyPage} className="text-accent hover:text-accent-hover">
                {t("settings.createAccount")}
              </button>
            </p>
          </div>
        )}

        {error && <p className="text-xs text-red-400">{error}</p>}

        <div className="flex justify-end gap-2 pt-1">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-neutral-400 hover:text-neutral-200"
          >
            {t("common.close")}
          </button>
          {!isSignedIn && (
            <button
              onClick={signInPassword}
              disabled={busy}
              className="px-4 py-1.5 text-sm bg-accent hover:bg-accent-hover disabled:opacity-40 rounded font-medium"
            >
              {busy ? t("settings.signingIn") : t("settings.signIn")}
            </button>
          )}
        </div>
      </div>
    </Modal>
  );
}
