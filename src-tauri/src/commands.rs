//! Tauri command surface — a thin layer over the backend modules.
//!
//! Commands are added as their backing modules are implemented. For v0.1 this
//! is just an info probe used by the shell to confirm the IPC boundary works.

#[tauri::command]
pub fn app_info() -> String {
    format!("wireforge {}", env!("CARGO_PKG_VERSION"))
}
