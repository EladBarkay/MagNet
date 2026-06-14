pub mod project;
pub mod gallery;
pub mod batch;
pub mod canvas_preset;
pub mod frame_preset;
pub mod auth;

/// Convert any `Result<T, E: Display>` into the `Result<T, String>` that Tauri IPC
/// handlers must return — replaces the repeated `.map_err(|e| e.to_string())`.
pub trait IntoTauri<T> {
    fn tauri(self) -> Result<T, String>;
}

impl<T, E: std::fmt::Display> IntoTauri<T> for Result<T, E> {
    fn tauri(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}
