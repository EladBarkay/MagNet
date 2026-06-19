// disconnect-device: remove a device from the caller's registry, freeing a seat.
//
// Request:  POST { device_hash: string }  (Bearer access token)
// Response: 200 { ok: true }
//
// The evicted device drops to Free on its next online check; within its remaining
// offline grace window its last signed token still verifies locally.

import { admin, HttpError, json, requireCaller } from "../_shared/auth.ts";

Deno.serve(async (req) => {
  try {
    if (req.method !== "POST") throw new HttpError(405, "method not allowed");

    const sb = admin();
    const caller = await requireCaller(req, sb);

    const body = await req.json().catch(() => ({}));
    const deviceHash = String(body.device_hash ?? "").trim();
    if (!deviceHash) throw new HttpError(400, "device_hash is required");

    const { error } = await sb
      .from("entitlement_devices")
      .delete()
      .eq("user_id", caller.id)
      .eq("device_hash", deviceHash);
    if (error) throw new HttpError(500, `delete failed: ${error.message}`);

    return json({ ok: true });
  } catch (e) {
    if (e instanceof HttpError) return json({ error: e.message }, e.status);
    return json({ error: String(e) }, 500);
  }
});
