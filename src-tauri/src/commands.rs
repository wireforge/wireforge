//! Tauri command surface — a thin layer over the backend modules.

use crate::environments::{self, EnvSummary};
use crate::error::{WfError, WfResult};
use crate::http_engine::{HttpEngine, ReqwestEngine};
use crate::model::{Environment, RequestFile, UnifiedRequest, UnifiedResponse};
use crate::postman::{self, ImportPreview, ImportResult};
use crate::variable_resolver::{resolve, ResolveOutcome};
use crate::workspace::{self, Node};
use std::collections::BTreeMap;
use std::path::Path;

fn read_import_file(path: &str) -> WfResult<String> {
    std::fs::read_to_string(path).map_err(|e| {
        Box::new(WfError::new(
            "WF_IMPORT_PARSE_FAILED",
            format!("could not read file: {e}"),
        ))
    })
}

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
pub fn move_node(root: String, path: String, dest: String) -> WfResult<String> {
    workspace::move_node(Path::new(&root), &path, &dest)
}

#[tauri::command]
pub fn duplicate_request(root: String, path: String) -> WfResult<String> {
    workspace::duplicate_request(Path::new(&root), &path)
}

#[tauri::command]
pub fn load_request_file(root: String, path: String) -> WfResult<RequestFile> {
    workspace::load_request_file(Path::new(&root), &path)
}

#[tauri::command]
pub fn save_request_file(root: String, path: String, request: RequestFile) -> WfResult<()> {
    workspace::save_request_file(Path::new(&root), &path, &request)
}

#[tauri::command]
pub fn import_preview(path: String) -> WfResult<ImportPreview> {
    postman::preview(&read_import_file(&path)?)
}

#[tauri::command]
pub fn import_apply(root: String, path: String) -> WfResult<ImportResult> {
    postman::apply(Path::new(&root), &read_import_file(&path)?)
}

#[tauri::command]
pub fn list_environments(root: String) -> WfResult<Vec<EnvSummary>> {
    environments::list_environments(Path::new(&root))
}

#[tauri::command]
pub fn create_environment(root: String, name: String) -> WfResult<String> {
    environments::create_environment(Path::new(&root), &name)
}

#[tauri::command]
pub fn load_environment(root: String, slug: String) -> WfResult<Environment> {
    environments::load_environment(Path::new(&root), &slug)
}

#[tauri::command]
pub fn save_environment(root: String, slug: String, environment: Environment) -> WfResult<()> {
    environments::save_environment(Path::new(&root), &slug, &environment)
}

/// Resolve `input` for previewing in the UI. Secret values are always redacted.
#[tauri::command]
pub fn resolve_preview(
    root: String,
    environment: Option<String>,
    input: String,
) -> WfResult<ResolveOutcome> {
    let (scopes, secrets) = environments::build_scopes(Path::new(&root), environment.as_deref())?;
    resolve(&input, &scopes, &secrets, &BTreeMap::new(), true)
}
