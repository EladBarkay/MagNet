import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Entitlement } from "../types";
import { supabase } from "../lib/supabase";
import { establishFromSession } from "../lib/auth";
import { tierLabel, tierColor } from "../lib/tiers";
import { EVENTS } from "../constants";
import { Modal } from "./ui";
import { useAsyncForm } from "../hooks/useAsyncForm";

type Props = {
  entitlement: Entitlement | null;
  onClose: () => void;
  onEntitlementChange: (entitlement: Entitlement | null) => void;
};

const BUY_URL = "https://magnet.app/pricing";

export default function SettingsDialog({ entitlement, onClose, onEntitlementChange }: Props) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const { error, setError, loading: busy, run } = useAsyncForm();

  const tier = entitlement?.tier ?? "free";
  const isSignedIn = entitlement !== null && !!entitlement.email;

  // Background refresh resolved a new tier — refetch the cached entitlement.
  useEffect(() => {
    const unsub = listen<void>(EVENTS.TIER_CHANGED, async () => {
      try {
        const info = await invoke<Entitlement | null>("get_entitlement");
        onEntitlementChange(info ?? null);
      } catch {}
    });
    const unsub2 = listen<void>(EVENTS.LICENSE_EXPIRED, () => onEntitlementChange(null));
    return () => { unsub.then((fn) => fn()); unsub2.then((fn) => fn()); };
  }, [onEntitlementChange]);

  async function signInPassword() {
    if (!email.trim() || !password) {
      setError("Enter your email and password");
      return;
    }
    await run(async () => {
      const { data, error } = await supabase.auth.signInWithPassword({
        email: email.trim(),
        password,
      });
      if (error || !data.session) throw new Error(error?.message ?? "Sign-in failed");
      const ent = await establishFromSession(data.session);
      onEntitlementChange(ent);
      setEmail("");
      setPassword("");
    });
  }

  async function signInOAuth(provider: "google" | "facebook") {
    await run(async () => {
      const { data, error } = await supabase.auth.signInWithOAuth({
        provider,
        options: {
          redirectTo: "magnet://auth-callback",
          skipBrowserRedirect: true,
        },
      });
      if (error || !data.url) throw new Error(error?.message ?? "Could not start sign-in");
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

  async function openBuyPage() {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(BUY_URL);
    } catch {}
  }

  return (
    <Modal onClose={onClose}>
      <div className="space-y-4">
        <h2 className="text-base font-semibold text-neutral-100">Settings</h2>

        {/* Tier status card */}
        <div className="rounded-lg bg-neutral-800 p-3 space-y-1.5">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium uppercase tracking-wide text-neutral-400">
              License
            </span>
            <span className={`text-xs font-semibold px-2 py-0.5 rounded-full ${tierColor(tier)}`}>
              {tierLabel(tier)}
            </span>
          </div>
          {isSignedIn ? (
            <div className="text-xs text-neutral-400 space-y-0.5">
              <p>{entitlement!.email}</p>
              <p className="text-neutral-500">
                {tier === "free"
                  ? "No active subscription"
                  : entitlement!.expires_at
                  ? `Expires ${entitlement!.expires_at}`
                  : "Active subscription"}
              </p>
            </div>
          ) : (
            <p className="text-xs text-neutral-500">
              Free tier — exported and printed canvases are watermarked.
            </p>
          )}
        </div>

        {isSignedIn ? (
          /* Signed in: manage / sign out */
          <div className="flex flex-col gap-2">
            {tier === "free" && (
              <button
                onClick={openBuyPage}
                className="text-xs text-blue-400 hover:text-blue-300 text-left"
              >
                Buy a license ↗
              </button>
            )}
            <button
              onClick={signOut}
              disabled={busy}
              className="text-xs text-red-400 hover:text-red-300 disabled:opacity-40 text-left"
            >
              {busy ? "Signing out…" : "Sign out"}
            </button>
          </div>
        ) : (
          /* Signed out: sign-in form */
          <div className="space-y-3">
            <div className="space-y-2">
              <label className="block text-xs font-medium text-neutral-400">
                Email
                <input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && signInPassword()}
                  placeholder="you@example.com"
                  autoFocus
                  className="mt-1 w-full bg-neutral-700 rounded px-2 py-1.5 text-sm text-neutral-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
                />
              </label>
              <label className="block text-xs font-medium text-neutral-400">
                Password
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && signInPassword()}
                  placeholder="••••••••"
                  className="mt-1 w-full bg-neutral-700 rounded px-2 py-1.5 text-sm text-neutral-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
                />
              </label>
            </div>

            <div className="flex items-center gap-2">
              <div className="h-px flex-1 bg-neutral-700" />
              <span className="text-[10px] uppercase tracking-wide text-neutral-500">or</span>
              <div className="h-px flex-1 bg-neutral-700" />
            </div>

            <div className="flex flex-col gap-2">
              <button
                onClick={() => signInOAuth("google")}
                disabled={busy}
                className="w-full px-3 py-1.5 text-sm bg-neutral-700 hover:bg-neutral-600 disabled:opacity-40 rounded font-medium"
              >
                Continue with Google
              </button>
              <button
                onClick={() => signInOAuth("facebook")}
                disabled={busy}
                className="w-full px-3 py-1.5 text-sm bg-neutral-700 hover:bg-neutral-600 disabled:opacity-40 rounded font-medium"
              >
                Continue with Facebook
              </button>
            </div>

            <p className="text-xs text-neutral-500">
              No account?{" "}
              <button onClick={openBuyPage} className="text-blue-400 hover:text-blue-300">
                Create one / buy a license ↗
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
            Close
          </button>
          {!isSignedIn && (
            <button
              onClick={signInPassword}
              disabled={busy}
              className="px-4 py-1.5 text-sm bg-blue-600 hover:bg-blue-500 disabled:opacity-40 rounded font-medium"
            >
              {busy ? "Signing in…" : "Sign in"}
            </button>
          )}
        </div>
      </div>
    </Modal>
  );
}
