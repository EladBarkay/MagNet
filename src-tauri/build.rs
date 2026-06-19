fn main() {
    tauri_build::build();

    // Load the repo-root .env so backend and frontend share one source of truth
    // for SUPABASE_URL / SUPABASE_ANON_KEY (the frontend reads the same file via
    // Vite). A real shell env var still wins — dotenvy does not overwrite.
    dotenvy::from_path("../.env").ok();
    println!("cargo:rerun-if-changed=../.env");

    // Supabase project config baked in at compile time so `env!()` resolves in
    // any build. The anon key is public (safe to ship). We always re-emit via
    // rustc-env because dotenvy only populates this build script's environment,
    // not the rustc process that compiles the crate's `env!()`.
    let url = std::env::var("SUPABASE_URL")
        .unwrap_or_else(|_| "https://YOUR_PROJECT_REF.supabase.co".to_string());
    println!("cargo:rustc-env=SUPABASE_URL={url}");
    let anon_key = std::env::var("SUPABASE_ANON_KEY")
        .unwrap_or_else(|_| "YOUR_SUPABASE_ANON_KEY".to_string());
    println!("cargo:rustc-env=SUPABASE_ANON_KEY={anon_key}");

    // Ed25519 public key (SPKI PEM) used to verify the server-signed entitlement
    // tokens offline. The matching private key lives ONLY in the Supabase Edge
    // Function secret `ENTITLEMENT_SIGNING_KEY` — never in this repo. A missing
    // key compiles fine but every token then fails verification (→ Free tier),
    // which is the safe default for builds without licensing configured.
    //
    // `cargo:rustc-env` is line-based, so a multi-line PEM must have its newlines
    // escaped to the two-char sequence `\n`; the runtime verifier decodes them
    // back before parsing the key.
    let ent_key = std::env::var("ENTITLEMENT_PUBLIC_KEY")
        .unwrap_or_else(|_| "-----BEGIN PUBLIC KEY-----\nUNCONFIGURED\n-----END PUBLIC KEY-----".to_string())
        .replace('\r', "")
        .replace('\n', "\\n");
    println!("cargo:rustc-env=ENTITLEMENT_PUBLIC_KEY={ent_key}");

    println!("cargo:rerun-if-env-changed=SUPABASE_URL");
    println!("cargo:rerun-if-env-changed=SUPABASE_ANON_KEY");
    println!("cargo:rerun-if-env-changed=ENTITLEMENT_PUBLIC_KEY");
}
