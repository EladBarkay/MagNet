# Supabase setup

MagNet uses Supabase for identity (sign-in) and authorization (tier). The app
treats **Rust as the source of truth**: the frontend only drives interactive
sign-in, then hands the session to Rust, which verifies the JWT against Supabase's
public JWKS and then mints a device-bound, **signed entitlement token** via the
`issue-entitlement` Edge Function (which reads the `entitlements` table
server-side). The token is verified offline on the client before its tier is
trusted.

This is a one-time, out-of-repo project configuration. Do it once per Supabase
project (e.g. a `staging` and a `prod` project).

## 1. Auth providers

In **Authentication → Providers**, enable:

- **Email** (email + password).
- **Google** — create an OAuth client in
  [Google Cloud Console](https://console.cloud.google.com/apis/credentials).
- **Facebook** — create an app in [Meta for Developers](https://developers.facebook.com/).

> **Authorized redirect URI for Google/Meta is Supabase's own callback:**
> `https://<project-ref>.supabase.co/auth/v1/callback`.
> Google/Meta never see the `magnetapp://` custom scheme — that is only the final
> Supabase→app hop (configured in step 2). Paste each provider's client id +
> secret into Supabase.

## 2. Redirect allow-list

In **Authentication → URL Configuration → Redirect URLs**, add:

```
magnetapp://auth-callback
```

Supabase permits non-HTTP schemes here for native/desktop apps. This is where the
PKCE OAuth flow returns control to the app via `tauri-plugin-deep-link`.

## 3. JWT signing keys (asymmetric)

In **Authentication → JWT Keys** (or **Settings → JWT**), enable **asymmetric
signing keys** (ES256 / RS256). Rust verifies access tokens against the public
JWKS at `https://<project-ref>.supabase.co/auth/v1/.well-known/jwks.json`, so no
JWT secret is embedded in the binary.

## 4. Schema + RLS

Run [`docs/supabase/entitlements.sql`](supabase/entitlements.sql) in the SQL
editor. It creates `public.entitlements`, enables Row-Level Security so each user
can read only their own row, and adds a trigger that auto-creates a `free` row on
signup. Only the **service role** (your Stripe webhook / admin tooling) can write
a paid tier — there is no client-writable policy.

The same file also creates `public.entitlement_devices`, the per-user device
registry used for seat enforcement. Users can **read** their own devices (for the
in-app "manage devices" list); only the service role (the Edge Functions below)
may insert/update/delete rows.

To grant someone Pro/Studio manually (e.g. for testing), run as service role:

```sql
update public.entitlements
  set tier = 'pro', expires_at = null, updated_at = now()
  where user_id = '<the-user-uuid>';
```

## 5. Config values

The anon key is public and safe to ship. Both the Rust binary and the frontend
read these Supabase vars from a single repo-root `.env` (see `.env.example`):

```
SUPABASE_URL=https://<project-ref>.supabase.co
SUPABASE_ANON_KEY=<anon-key>
```

(The `.env` also holds `ENTITLEMENT_PUBLIC_KEY` — the entitlement-token verifier
key — set up in §6.)

`src-tauri/build.rs` loads this `.env` via `dotenvy` and bakes the values in at
`cargo build`; Vite exposes them to the frontend via
`envPrefix: ["VITE_", "SUPABASE_"]`. A real exported shell env var overrides the
`.env` on either side.

## 6. Entitlement token signing key + Edge Functions

Tier is delivered to the client as a **server-signed, device-bound entitlement
token** (EdDSA JWS), verified offline against a baked-in public key. This is what
makes the offline cache tamper-resistant and binds a seat to a machine.

1. **Generate an Ed25519 keypair** (one per project):

   ```bash
   openssl genpkey -algorithm ed25519 -out ent_priv.pem
   openssl pkey -in ent_priv.pem -pubout -out ent_pub.pem
   ```

2. **Public key → client.** Put the contents of `ent_pub.pem` into the repo-root
   `.env` as `ENTITLEMENT_PUBLIC_KEY` (one line, `\n` between PEM lines — see
   `.env.example`). It is baked into the binary by `build.rs`. Public — safe to ship.

3. **Private key → server secret.** Never commit it. Set it as the Edge Function
   secret:

   ```bash
   supabase secrets set ENTITLEMENT_SIGNING_KEY="$(cat ent_priv.pem)"
   ```

4. **Deploy the functions** (`supabase/functions/`):

   ```bash
   supabase functions deploy issue-entitlement
   supabase functions deploy disconnect-device
   ```

   - `issue-entitlement` — registers the caller's device (subject to the per-tier
     seat limit in `_shared/auth.ts`: Free uncapped, Pro 2, Studio 5) and returns a
     signed token. At the limit it returns `409 { error: "device_limit_reached",
     devices }` and the app shows the device picker.
   - `disconnect-device` — deletes a device row, freeing a seat.

   Both use the platform-provided `SUPABASE_URL` / `SUPABASE_SERVICE_ROLE_KEY` and
   authenticate the caller's access token.

## 7. Google / OAuth sign-in

The desktop OAuth flow opens the system browser at the Supabase authorize URL,
which redirects to Google and back to the app via the custom deep link
`magnetapp://auth-callback` (PKCE; `useAuthDeepLink` exchanges the code). The
app registers the `magnetapp://` scheme with the OS at runtime
(`lib.rs` → `deep_link().register_all()`), so the callback reaches dev builds too.

For sign-in to actually complete, configure the Supabase project:

1. **Auth → Providers → Google**: enable it; paste a Google Cloud OAuth client
   ID + secret.
2. **Google Cloud Console** → that OAuth client → *Authorized redirect URIs* →
   add `https://<project-ref>.supabase.co/auth/v1/callback`.
3. **Auth → URL Configuration → Redirect URLs**: add `magnetapp://auth-callback`.
   If this is missing, the authorize page stalls instead of redirecting to
   Google (the `redirect_to` is rejected).

## Dev / QA at a paid tier

There is no compile-time tier bypass. To test paid behavior, sign in with a real
account and grant it a tier as the service role (see the `update` snippet in §4);
every build — dev and release — goes through real sign-in and server validation.
