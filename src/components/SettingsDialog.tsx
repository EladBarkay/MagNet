import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { LicenseInfo } from "../types";
import { Modal } from "./ui";

type Props = {
  license: LicenseInfo | null;
  onClose: () => void;
  onLicenseChange: (license: LicenseInfo | null) => void;
};

type Step = "form" | "otp";

const TIER_LABELS: Record<string, string> = {
  free: "Free",
  pro: "Pro",
  studio: "Studio",
};

const TIER_COLORS: Record<string, string> = {
  free: "bg-neutral-700 text-neutral-300",
  pro: "bg-green-700/80 text-white",
  studio: "bg-purple-700/80 text-white",
};

export default function SettingsDialog({ license, onClose, onLicenseChange }: Props) {
  const [step, setStep] = useState<Step>("form");
  const [email, setEmail] = useState("");
  const [key, setKey] = useState("");
  const [challengeId, setChallengeId] = useState("");
  const [otp, setOtp] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const tier = license?.tier ?? "free";
  const isActivated = license !== null;

  // Listen for background revalidation results (connectivity watcher, revocation)
  useEffect(() => {
    const unsub = listen<void>("tier-changed", async () => {
      try {
        const info = await invoke<LicenseInfo | null>("get_license_info");
        onLicenseChange(info);
      } catch {}
    });
    return () => { unsub.then(fn => fn()); };
  }, [onLicenseChange]);

  // Also listen for grace period expiry
  useEffect(() => {
    const unsub = listen<void>("license-expired", () => {
      onLicenseChange(null);
    });
    return () => { unsub.then(fn => fn()); };
  }, [onLicenseChange]);

  async function sendCode() {
    if (!email.trim() || !key.trim()) {
      setError("Enter both email and license key");
      return;
    }
    setError("");
    setBusy(true);
    try {
      const id = await invoke<string>("activate_init", {
        email: email.trim(),
        key: key.trim(),
      });
      setChallengeId(id);
      setStep("otp");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function verifyOtp() {
    if (!otp.trim()) {
      setError("Enter the 6-digit code");
      return;
    }
    setError("");
    setBusy(true);
    try {
      const info = await invoke<LicenseInfo>("activate_confirm", {
        challengeId,
        otp: otp.trim(),
        email: email.trim(),
      });
      onLicenseChange(info);
      setStep("form");
      setEmail("");
      setKey("");
      setOtp("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function deactivate() {
    setBusy(true);
    try {
      await invoke("clear_license");
      onLicenseChange(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function openAccountPortal() {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl("https://magnet.app/account");
    } catch {}
  }

  return (
    <Modal onClose={onClose}>
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">Settings</h2>

        {/* License status card */}
        <div className="rounded-lg bg-neutral-800 p-3 space-y-1.5">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium uppercase tracking-wide text-neutral-400">
              License
            </span>
            <span className={`text-xs font-semibold px-2 py-0.5 rounded-full ${TIER_COLORS[tier] ?? TIER_COLORS.free}`}>
              {TIER_LABELS[tier] ?? tier}
            </span>
          </div>
          {isActivated ? (
            <div className="text-xs text-neutral-400 space-y-0.5">
              <p>{license.email}</p>
              <p className="text-neutral-500">
                {license.expires_at
                  ? `Expires ${license.expires_at}`
                  : "Active subscription"}
              </p>
            </div>
          ) : (
            <p className="text-xs text-neutral-500">
              Free tier — exported and printed canvases are watermarked.
            </p>
          )}
        </div>

        {isActivated ? (
          /* Active license: manage / deactivate */
          <div className="flex flex-col gap-2">
            <button
              onClick={openAccountPortal}
              className="text-xs text-blue-400 hover:text-blue-300 text-left"
            >
              Manage subscription ↗
            </button>
            <button
              onClick={deactivate}
              disabled={busy}
              className="text-xs text-red-400 hover:text-red-300 disabled:opacity-40 text-left"
            >
              {busy ? "Deactivating…" : "Deactivate on this device"}
            </button>
          </div>
        ) : step === "form" ? (
          /* Step 1: email + key */
          <div className="space-y-2">
            <label className="block text-xs font-medium text-neutral-400">
              Email (used at purchase)
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && sendCode()}
                placeholder="you@example.com"
                autoFocus
                className="mt-1 w-full bg-neutral-700 rounded px-2 py-1.5 text-sm text-neutral-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
            </label>
            <label className="block text-xs font-medium text-neutral-400">
              License key
              <input
                type="text"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && sendCode()}
                placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
                className="mt-1 w-full bg-neutral-700 rounded px-2 py-1.5 text-sm font-mono text-neutral-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
            </label>
          </div>
        ) : (
          /* Step 2: OTP */
          <div className="space-y-2">
            <p className="text-xs text-neutral-400">
              Check <span className="text-neutral-200">{email}</span> for a 6-digit verification code.
            </p>
            <label className="block text-xs font-medium text-neutral-400">
              Verification code
              <input
                type="text"
                inputMode="numeric"
                value={otp}
                onChange={(e) => setOtp(e.target.value.replace(/\D/g, "").slice(0, 6))}
                onKeyDown={(e) => e.key === "Enter" && verifyOtp()}
                placeholder="000000"
                maxLength={6}
                autoFocus
                className="mt-1 w-full bg-neutral-700 rounded px-2 py-1.5 text-sm font-mono tracking-widest text-neutral-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
            </label>
            <button
              onClick={() => { setStep("form"); setOtp(""); setError(""); }}
              className="text-xs text-neutral-500 hover:text-neutral-300"
            >
              ← Back / use different key
            </button>
          </div>
        )}

        {error && <p className="text-xs text-red-400">{error}</p>}

        <div className="flex justify-end gap-2 pt-1">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-neutral-400 hover:text-neutral-200"
          >
            Close
          </button>
          {!isActivated && (
            step === "form" ? (
              <button
                onClick={sendCode}
                disabled={busy}
                className="px-4 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded font-medium"
              >
                {busy ? "Sending…" : "Send verification code"}
              </button>
            ) : (
              <button
                onClick={verifyOtp}
                disabled={busy || otp.length !== 6}
                className="px-4 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded font-medium"
              >
                {busy ? "Verifying…" : "Activate"}
              </button>
            )
          )}
        </div>
      </div>
    </Modal>
  );
}
