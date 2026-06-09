//! Tauri command surface — a thin layer over the backend modules.

use crate::error::WfResult;
use crate::http_engine::{HttpEngine, ReqwestEngine};
use crate::model::{RequestFile, UnifiedRequest, UnifiedResponse};
use crate::workspace::{self, Node};
use std::path::Path;

#[tauri::command]
pub fn app_info() -> String {
    format!("wireforge {}", env!("CARGO_PKG_VERSION"))
}

#[tauri::command]
pub async fn send_request(request: UnifiedRequest) -> WfResult<UnifiedResponse> {
    ReqwestEngine::new().send(request).await
}

#[tauri::command]
pub fn open_workspace(root: String) -> WfResult<Vec<Node>> {
    workspace::load_tree(Path::new(&root))
}

#[tauri::command]
pub fn create_request(root: String, folder: String, name: String) -> WfResult<String> {
    workspace::create_request(Path::new(&root), &folder, &name)
}

#[tauri::command]
pub fn create_folder(root: String, parent: String, name: String) -> WfResult<String> {
    workspace::create_folder(Path::new(&root), &parent, &name)
}

#[tauri::command]
pub fn rename_node(root: String, path: String, name: String) -> WfResult<()> {
    workspace::rename(Path::new(&root), &path, &name)
}

#[tauri::command]
pub fn delete_node(root: String, path: String) -> WfResult<()> {
    workspace::delete(Path::new(&root), &path)
}

#[tauri::command]
pub fn load_request_file(root: String, path: String) -> WfResult<RequestFile> {
    workspace::load_request_file(Path::new(&root), &path)
}

#[tauri::command]
pub fn save_request_file(root: String, path: String, request: RequestFile) -> WfResult<()> {
    workspace::save_request_file(Path::new(&root), &path, &request)
}
