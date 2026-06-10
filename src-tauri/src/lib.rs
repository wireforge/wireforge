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
pub mod environments;
pub mod http_engine;
pub mod importer;
pub mod postman;
pub mod secret_resolver;
pub mod theme_store;
pub mod variable_resolver;
pub mod vcs;
pub mod workspace;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::send_request,
            commands::open_workspace,
            commands::create_request,
            commands::create_folder,
            commands::rename_node,
            commands::delete_node,
            commands::move_node,
            commands::duplicate_request,
            commands::load_request_file,
            commands::save_request_file,
            commands::import_preview,
            commands::import_apply,
            commands::list_environments,
            commands::create_environment,
            commands::load_environment,
            commands::save_environment,
            commands::resolve_preview,
            commands::secret_status,
            commands::set_secret,
            commands::delete_secret
        ])
        .run(tauri::generate_context!())
        .expect("error while running wireforge application");
}
