//! Tauri command surface — a thin layer over the backend modules.

use crate::environments::{self, EnvSummary};
use crate::error::{WfError, WfResult};
use crate::github::{self, AuthStatus, DeviceStart, PollOutcome};
use crate::http_engine::{HttpEngine, ReqwestEngine};
use crate::model::{Environment, RequestFile, UnifiedRequest, UnifiedResponse};
use crate::postman::{self, ImportPreview, ImportResult};
use crate::secret_resolver::{self, SecretStatus};
use crate::variable_resolver::{resolve, ResolveOutcome};
use crate::vcs::{self, ConflictSides, PullOutcome, RepoStatus};
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

/// Send a request. When `root` is provided, all `{{variables}}` are resolved
/// against the workspace (and optional active `environment`) on the backend —
/// secrets included — and the send is rejected before going out if a variable
/// is unresolved or a required secret is missing. Without `root`, the request
/// is sent as-is.
#[tauri::command]
pub async fn send_request(
    request: UnifiedRequest,
    root: Option<String>,
    environment: Option<String>,
) -> WfResult<UnifiedResponse> {
    let resolved = match &root {
        Some(r) => {
            secret_resolver::resolve_request(Path::new(r), environment.as_deref(), &request)?
        }
        None => request,
    };
    ReqwestEngine::new().send(resolved).await
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
pub fn import_curl(text: String) -> WfResult<RequestFile> {
    crate::curl::parse(&text)
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

#[tauri::command]
pub fn secret_status(root: String, environment: Option<String>) -> WfResult<Vec<SecretStatus>> {
    secret_resolver::secret_status(Path::new(&root), environment.as_deref())
}

#[tauri::command]
pub fn set_secret(root: String, environment: String, name: String, value: String) -> WfResult<()> {
    secret_resolver::set_secret(Path::new(&root), &environment, &name, &value)
}

#[tauri::command]
pub fn delete_secret(root: String, environment: String, name: String) -> WfResult<()> {
    secret_resolver::delete_secret(Path::new(&root), &environment, &name)
}

#[tauri::command]
pub fn export_docs(root: String) -> WfResult<String> {
    workspace::export_markdown(Path::new(&root))
}

/// Write text to an arbitrary path the user chose via a save dialog.
#[tauri::command]
pub fn save_text(path: String, content: String) -> WfResult<()> {
    std::fs::write(&path, content)
        .map_err(|e| Box::new(WfError::new("WF_STORE_WRITE_FAILED", e.to_string())))
}

#[tauri::command]
pub fn git_status(root: String) -> WfResult<RepoStatus> {
    vcs::repo_status(Path::new(&root))
}

#[tauri::command]
pub fn git_diff(root: String, path: Option<String>) -> WfResult<String> {
    vcs::diff(Path::new(&root), path.as_deref())
}

#[tauri::command]
pub fn git_commit(root: String, message: String, paths: Vec<String>) -> WfResult<()> {
    vcs::commit(Path::new(&root), &message, &paths)
}

#[tauri::command]
pub fn git_push(root: String) -> WfResult<()> {
    vcs::push(Path::new(&root))
}

#[tauri::command]
pub fn git_pull(root: String) -> WfResult<PullOutcome> {
    vcs::pull(Path::new(&root))
}

#[tauri::command]
pub fn git_conflict_sides(root: String, path: String) -> WfResult<ConflictSides> {
    vcs::conflict_sides(Path::new(&root), &path)
}

#[tauri::command]
pub fn git_resolve_conflict(root: String, path: String, choice: String) -> WfResult<()> {
    vcs::resolve_conflict(Path::new(&root), &path, &choice)
}

#[tauri::command]
pub fn git_mark_resolved(root: String, path: String) -> WfResult<()> {
    vcs::mark_resolved(Path::new(&root), &path)
}

#[tauri::command]
pub async fn github_device_start(host: String, client_id: String) -> WfResult<DeviceStart> {
    github::device_start(&host, &client_id).await
}

#[tauri::command]
pub async fn github_device_poll(
    host: String,
    client_id: String,
    device_code: String,
) -> WfResult<PollOutcome> {
    github::device_poll(&host, &client_id, &device_code).await
}

#[tauri::command]
pub async fn github_auth_status(host: String) -> WfResult<AuthStatus> {
    github::auth_status(&host).await
}

#[tauri::command]
pub fn github_logout(host: String) -> WfResult<()> {
    github::logout(&host)
}
