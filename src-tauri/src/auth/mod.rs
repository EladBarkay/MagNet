pub mod session;
pub mod jwt;
pub mod entitlement;
pub mod entitlement_token;
pub mod device;
pub mod client;
pub mod provision;

/// Bundled auth state held in memory: the active Supabase session plus the
/// resolved entitlement. `None` in `AppState.auth` => Free tier.
///
/// The entitlement here is always derived from a *verified* entitlement token
/// (see `entitlement_token`), so the in-memory tier is never attacker-supplied.
#[derive(Debug, Clone)]
pub struct AuthState {
    pub session: session::Session,
    pub entitlement: entitlement::Entitlement,
}
