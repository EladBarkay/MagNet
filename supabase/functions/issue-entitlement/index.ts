// issue-entitlement: register the caller's device (subject to the per-tier seat
// limit) and return a signed, device-bound entitlement token for offline use.
//
// Request:  POST { device_hash: string, device_label?: string }  (Bearer access token)
// Response: 200 { token }                       — minted
//           409 { error: "device_limit_reached", devices: [...] }
//
// Secrets/env: ENTITLEMENT_SIGNING_KEY (Ed25519 PKCS8 PEM), plus the platform's
// SUPABASE_URL / SUPABASE_SERVICE_ROLE_KEY.

import {
  admin,
  HttpError,
  json,
  requireCaller,
  SEAT_LIMITS,
  TOKEN_TTL_SECONDS,
} from "../_shared/auth.ts";
import { signEntitlementToken } from "../_shared/jwt.ts";

Deno.serve(async (req) => {
  try {
    if (req.method !== "POST") throw new HttpError(405, "method not allowed");

    const sb = admin();
    const caller = await requireCaller(req, sb);

    const body = await req.json().catch(() => ({}));
    const deviceHash = String(body.device_hash ?? "").trim();
    const deviceLabel = String(body.device_label ?? "").slice(0, 200);
    if (!deviceHash) throw new HttpError(400, "device_hash is required");

    // Resolve the caller's entitlement (service role bypasses RLS). No row => free.
    const { data: ent } = await sb
      .from("entitlements")
      .select("tier, expires_at")
      .eq("user_id", caller.id)
      .maybeSingle();

    const tier = ent?.tier ?? "free";
    const subExpiresAt: string | null = ent?.expires_at ?? null;

    // Current devices for seat accounting.
    const { data: devices } = await sb
      .from("entitlement_devices")
      .select("device_hash, device_label, last_seen")
      .eq("user_id", caller.id);

    const known = (devices ?? []).some((d) => d.device_hash === deviceHash);
    const limit = SEAT_LIMITS[tier] ?? SEAT_LIMITS.free;

    if (!known && (devices ?? []).length >= limit) {
      // At the seat limit — caller must disconnect a device first.
      return json(
        { error: "device_limit_reached", devices: devices ?? [] },
        409,
      );
    }

    // Register (or refresh) this device.
    const now = new Date().toISOString();
    const { error: upsertErr } = await sb
      .from("entitlement_devices")
      .upsert(
        {
          user_id: caller.id,
          device_hash: deviceHash,
          device_label: deviceLabel,
          last_seen: now,
        },
        { onConflict: "user_id,device_hash" },
      );
    if (upsertErr) {
      throw new HttpError(500, `device upsert failed: ${upsertErr.message}`);
    }

    const iat = Math.floor(Date.now() / 1000);
    const token = await signEntitlementToken({
      sub: caller.id,
      tier,
      email: caller.email,
      sub_expires_at: subExpiresAt,
      device: deviceHash,
      iat,
      exp: iat + TOKEN_TTL_SECONDS,
    });

    return json({ token });
  } catch (e) {
    if (e instanceof HttpError) return json({ error: e.message }, e.status);
    return json({ error: String(e) }, 500);
  }
});
