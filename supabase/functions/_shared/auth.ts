// Shared helpers for the entitlement Edge Functions: a service-role Supabase
// client (bypasses RLS) and resolution of the caller from their access token.

import { createClient, type SupabaseClient } from "npm:@supabase/supabase-js@2";

export const TOKEN_TTL_SECONDS = 14 * 24 * 60 * 60; // 14-day offline grace ceiling

// Per-tier device-seat limits. Free is effectively uncapped (watermarked output
// removes any incentive to share); paid tiers are capped to curb credential
// sharing.
export const SEAT_LIMITS: Record<string, number> = {
  free: Number.MAX_SAFE_INTEGER,
  pro: 2,
  studio: 5,
};

export function admin(): SupabaseClient {
  return createClient(
    Deno.env.get("SUPABASE_URL")!,
    Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!,
    { auth: { persistSession: false } },
  );
}

export type Caller = { id: string; email: string | null };

/** Validate the request's bearer token and return the authenticated user. */
export async function requireCaller(
  req: Request,
  sb: SupabaseClient,
): Promise<Caller> {
  const authz = req.headers.get("Authorization") ?? "";
  const token = authz.replace(/^Bearer\s+/i, "");
  if (!token) throw new HttpError(401, "missing access token");

  const { data, error } = await sb.auth.getUser(token);
  if (error || !data.user) throw new HttpError(401, "invalid access token");
  return { id: data.user.id, email: data.user.email ?? null };
}

export class HttpError extends Error {
  constructor(public status: number, message: string) {
    super(message);
  }
}

export function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
