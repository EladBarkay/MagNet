// Minimal EdDSA (Ed25519) JWS signer for the entitlement token, built on the
// Web Crypto API (available in the Edge runtime). The private key lives only in
// the `ENTITLEMENT_SIGNING_KEY` secret; the matching public key is baked into the
// desktop client, which verifies these tokens offline.

function base64urlEncode(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function pemToDer(pem: string): Uint8Array<ArrayBuffer> {
  const body = pem
    .replace(/-----BEGIN [^-]+-----/, "")
    .replace(/-----END [^-]+-----/, "")
    .replace(/\s+/g, "");
  const bin = atob(body);
  // Back the array with a concrete ArrayBuffer so it satisfies the Web Crypto
  // `BufferSource` typing (which rejects the looser `ArrayBufferLike`).
  const der = new Uint8Array(new ArrayBuffer(bin.length));
  for (let i = 0; i < bin.length; i++) der[i] = bin.charCodeAt(i);
  return der;
}

let cachedKey: CryptoKey | null = null;

async function signingKey(): Promise<CryptoKey> {
  if (cachedKey) return cachedKey;
  const pem = Deno.env.get("ENTITLEMENT_SIGNING_KEY");
  if (!pem) throw new Error("ENTITLEMENT_SIGNING_KEY is not configured");
  cachedKey = await crypto.subtle.importKey(
    "pkcs8",
    pemToDer(pem),
    { name: "Ed25519" },
    false,
    ["sign"],
  );
  return cachedKey;
}

export type EntitlementClaims = {
  sub: string;
  tier: string;
  email?: string | null;
  sub_expires_at?: string | null;
  device: string;
  iat: number;
  exp: number;
};

/** Sign the claims into a compact EdDSA JWS. */
export async function signEntitlementToken(
  claims: EntitlementClaims,
): Promise<string> {
  const header = { alg: "EdDSA", typ: "JWT" };
  const enc = new TextEncoder();
  const signingInput = base64urlEncode(enc.encode(JSON.stringify(header))) +
    "." +
    base64urlEncode(enc.encode(JSON.stringify(claims)));

  const sig = await crypto.subtle.sign(
    "Ed25519",
    await signingKey(),
    enc.encode(signingInput),
  );
  return signingInput + "." + base64urlEncode(new Uint8Array(sig));
}
