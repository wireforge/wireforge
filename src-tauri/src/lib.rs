//! wireforge backend library.
//!
//! Module boundaries follow the documented architecture. Most modules
//! currently define their trait contract only; implementations land in
//! later phases, so dead code is allowed during scaffolding.
#![allow(dead_code)]

pub mod canonical;
pub mod error;
pub mod id;
pub mod model;

mod commands;

pub mod agent_surface;
pub mod collection_store;
pub mod http_engine;
pub mod importer;
pub mod secret_resolver;
pub mod theme_store;
pub mod variable_resolver;
pub mod vcs;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::send_request
        ])
        .run(tauri::generate_context!())
        .expect("error while running wireforge application");
}
