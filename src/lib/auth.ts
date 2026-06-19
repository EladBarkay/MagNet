import { invoke } from "@tauri-apps/api/core";
import type { Session } from "@supabase/supabase-js";
import { supabase } from "./supabase";
import type { AuthResult, Device } from "../types";

/// Hand a freshly obtained Supabase session to Rust (the source of truth), which
/// verifies the JWT, registers this device + mints a signed entitlement token,
/// and persists the session. Rust now owns the session, so we drop the local
/// supabase copy. Returns either the resolved entitlement or a device-limit
/// prompt (the caller then disconnects a device to finish signing in).
export async function establishFromSession(session: Session): Promise<AuthResult> {
  const result = await invoke<AuthResult>("establish_session", {
    accessToken: session.access_token,
    refreshToken: session.refresh_token,
    expiresAt: session.expires_at ?? 0,
  });
  await supabase.auth.signOut({ scope: "local" });
  return result;
}

/// Disconnect another device (freeing a seat) and complete provisioning for this
/// one. Returns the resolved entitlement, or a still-limited device list.
export async function disconnectDevice(deviceHash: string): Promise<AuthResult> {
  return invoke<AuthResult>("disconnect_device", { deviceHash });
}

/// List the devices currently registered to the subscription. `null` => signed out.
export async function listDevices(): Promise<Device[] | null> {
  return invoke<Device[] | null>("list_devices");
}

/// Hash identifying the machine the app runs on, so the UI can mark "this device".
export async function currentDeviceHash(): Promise<string> {
  return invoke<string>("current_device_hash");
}
