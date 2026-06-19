-- MagNet entitlements: one row per user, the single source of truth for tier.
-- Run in the Supabase SQL editor (or via the CLI) on the project.

create table public.entitlements (
  user_id    uuid primary key references auth.users(id) on delete cascade,
  tier       text not null default 'free' check (tier in ('free','pro','studio')),
  expires_at date,
  updated_at timestamptz not null default now()
);

alter table public.entitlements enable row level security;

-- Users may read ONLY their own row. There is intentionally no insert/update/
-- delete policy, so only the service role (Stripe webhook / admin) can grant a
-- paid tier.
create policy "read own entitlement" on public.entitlements
  for select using (auth.uid() = user_id);

-- Auto-create a free row whenever a new auth user signs up.
create function public.handle_new_user() returns trigger
  language plpgsql security definer as $$
begin
  insert into public.entitlements(user_id, tier) values (new.id, 'free')
  on conflict do nothing;
  return new;
end; $$;

create trigger on_auth_user_created after insert on auth.users
  for each row execute function public.handle_new_user();

-- ── Device registry (license seat enforcement) ──────────────────────────────
-- One row per machine actively using the subscription. The `issue-entitlement`
-- Edge Function inserts/updates rows (as the service role) and caps the count per
-- tier; `disconnect-device` deletes a row to free a seat. The client never writes
-- here directly.
create table public.entitlement_devices (
  user_id      uuid not null references auth.users(id) on delete cascade,
  device_hash  text not null,
  device_label text not null default '',
  first_seen   timestamptz not null default now(),
  last_seen    timestamptz not null default now(),
  primary key (user_id, device_hash)
);

alter table public.entitlement_devices enable row level security;

-- Users may READ only their own devices (so the in-app "manage devices" list
-- works). There is intentionally no insert/update/delete policy — only the
-- service role (the Edge Functions) may mutate the registry.
create policy "read own devices" on public.entitlement_devices
  for select using (auth.uid() = user_id);
