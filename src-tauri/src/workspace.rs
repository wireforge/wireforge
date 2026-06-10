//! Workspace loading and collection CRUD over the on-disk layout
//! (`collections/main/requests/...` with `_folder.wf.json` and `collection.json`
//! order arrays). Paths in this API are relative to the requests root.

use crate::canonical::{repair_order, to_canonical_json};
use crate::error::{WfError, WfResult};
use crate::id::new_id;
use crate::model::{Collection, Folder, HttpMethod, RequestFile, Workspace};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const REQUEST_EXT: &str = ".wf.json";
const FOLDER_FILE: &str = "_folder.wf.json";

/// A node in the collection tree, as sent to the frontend sidebar.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Node {
    Folder {
        id: String,
        name: String,
        path: String,
        children: Vec<Node>,
    },
    Request {
        id: String,
        name: String,
        method: HttpMethod,
        path: String,
    },
}

pub(crate) fn collection_dir(workspace: &Path) -> PathBuf {
    workspace.join("collections").join("main")
}

fn requests_root(workspace: &Path) -> PathBuf {
    collection_dir(workspace).join("requests")
}

fn rel(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn parent_relpath(relpath: &str) -> String {
    match relpath.rfind('/') {
        Some(i) => relpath[..i].to_string(),
        None => String::new(),
    }
}

pub(crate) fn slugify(name: &str) -> String {
    let mut s: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "untitled".to_string()
    } else {
        s
    }
}

pub(crate) fn unique_path(dir: &Path, base: &str, ext: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{base}{ext}"));
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{base}-{n}{ext}"));
        n += 1;
    }
    candidate
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> WfResult<T> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| WfError::new("WF_STORE_FILE_NOT_FOUND", e.to_string()))?;
    let value = serde_json::from_str(&text)
        .map_err(|e| WfError::new("WF_STORE_VALIDATION_FAILED", e.to_string()))?;
    Ok(value)
}

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> WfResult<()> {
    let json =
        to_canonical_json(value).map_err(|e| WfError::new("WF_SERIALIZE_FAILED", e.to_string()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
    }
    let tmp = path.with_extension("wf-tmp");
    std::fs::write(&tmp, json.as_bytes())
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
    Ok(())
}

/// Create the minimal workspace/collection files if they do not exist yet.
fn ensure_collection(workspace: &Path) -> WfResult<()> {
    std::fs::create_dir_all(requests_root(workspace))
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;

    let col = collection_dir(workspace).join("collection.json");
    if !col.exists() {
        write_json(
            &col,
            &Collection {
                format: "wireforge.collection".to_string(),
                version: 1,
                id: new_id("col"),
                name: "Main".to_string(),
                variables: Default::default(),
                order: vec![],
            },
        )?;
    }

    let ws = workspace.join("wireforge.json");
    if !ws.exists() {
        write_json(
            &ws,
            &Workspace {
                format: "wireforge.workspace".to_string(),
                version: 1,
                id: Some(new_id("ws")),
                name: "Workspace".to_string(),
                default_collection_id: None,
                default_environment_id: None,
                variables: Default::default(),
            },
        )?;
    }
    Ok(())
}

/// Return the workspace's stable id, generating and persisting one if the
/// workspace file lacks it. Used to namespace keychain entries.
pub fn ensure_workspace_id(workspace: &Path) -> WfResult<String> {
    ensure_collection(workspace)?;
    let path = workspace.join("wireforge.json");
    let mut ws: Workspace = read_json(&path)?;
    if let Some(id) = &ws.id {
        return Ok(id.clone());
    }
    let id = new_id("ws");
    ws.id = Some(id.clone());
    write_json(&path, &ws)?;
    Ok(id)
}

/// Read the child-order array for a folder (root order lives in collection.json).
fn read_children_order(workspace: &Path, folder_relpath: &str) -> WfResult<Vec<String>> {
    if folder_relpath.is_empty() {
        let col = collection_dir(workspace).join("collection.json");
        if col.exists() {
            let c: Collection = read_json(&col)?;
            return Ok(c.order);
        }
        Ok(vec![])
    } else {
        let ff = requests_root(workspace)
            .join(folder_relpath)
            .join(FOLDER_FILE);
        if ff.exists() {
            let f: Folder = read_json(&ff)?;
            return Ok(f.order);
        }
        Ok(vec![])
    }
}

fn write_children_order(
    workspace: &Path,
    folder_relpath: &str,
    order: Vec<String>,
) -> WfResult<()> {
    if folder_relpath.is_empty() {
        let col = collection_dir(workspace).join("collection.json");
        let mut c: Collection = read_json(&col)?;
        c.order = order;
        write_json(&col, &c)
    } else {
        let ff = requests_root(workspace)
            .join(folder_relpath)
            .join(FOLDER_FILE);
        let mut f: Folder = read_json(&ff)?;
        f.order = order;
        write_json(&ff, &f)
    }
}

fn load_dir(base: &Path, dir: &Path, order: &[String]) -> WfResult<Vec<Node>> {
    let mut by_id: HashMap<String, Node> = HashMap::new();
    let mut present: Vec<String> = vec![];

    let entries = std::fs::read_dir(dir)
        .map_err(|e| WfError::new("WF_STORE_FILE_NOT_FOUND", e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| WfError::new("WF_STORE_FILE_NOT_FOUND", e.to_string()))?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            let ffile = path.join(FOLDER_FILE);
            let (fid, fname, forder) = if ffile.exists() {
                let f: Folder = read_json(&ffile)?;
                (f.id, f.name, f.order)
            } else {
                (new_id("folder"), file_name.clone(), vec![])
            };
            let children = load_dir(base, &path, &forder)?;
            present.push(fid.clone());
            by_id.insert(
                fid.clone(),
                Node::Folder {
                    id: fid,
                    name: fname,
                    path: rel(base, &path),
                    children,
                },
            );
        } else if file_name.ends_with(REQUEST_EXT) && file_name != FOLDER_FILE {
            let req: RequestFile = read_json(&path)?;
            present.push(req.id.clone());
            by_id.insert(
                req.id.clone(),
                Node::Request {
                    id: req.id.clone(),
                    name: req.name,
                    method: req.method,
                    path: rel(base, &path),
                },
            );
        }
    }

    let ordered = repair_order(order, &present);
    Ok(ordered
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect())
}

/// Load the collection tree for the workspace. Returns an empty tree if the
/// workspace has no collection yet.
pub fn load_tree(workspace: &Path) -> WfResult<Vec<Node>> {
    let root = requests_root(workspace);
    if !root.exists() {
        return Ok(vec![]);
    }
    let order = read_children_order(workspace, "")?;
    load_dir(&root, &root, &order)
}

/// Create a new request under `folder_relpath` (empty string = root). Returns
/// the new request's relative path.
pub fn create_request(workspace: &Path, folder_relpath: &str, name: &str) -> WfResult<String> {
    ensure_collection(workspace)?;
    let dir = if folder_relpath.is_empty() {
        requests_root(workspace)
    } else {
        requests_root(workspace).join(folder_relpath)
    };
    std::fs::create_dir_all(&dir)
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;

    let id = new_id("req");
    let path = unique_path(&dir, &slugify(name), REQUEST_EXT);
    let request = RequestFile {
        format: "wireforge.request".to_string(),
        version: 1,
        id: id.clone(),
        name: name.to_string(),
        description: None,
        method: HttpMethod::Get,
        url: String::new(),
        params: vec![],
        headers: vec![],
        auth: Default::default(),
        body: Default::default(),
    };
    write_json(&path, &request)?;

    let mut order = read_children_order(workspace, folder_relpath)?;
    order.push(id);
    write_children_order(workspace, folder_relpath, order)?;

    Ok(rel(&requests_root(workspace), &path))
}

/// Create a new folder under `parent_relpath` (empty string = root). Returns the
/// new folder's relative path.
pub fn create_folder(workspace: &Path, parent_relpath: &str, name: &str) -> WfResult<String> {
    ensure_collection(workspace)?;
    let parent = if parent_relpath.is_empty() {
        requests_root(workspace)
    } else {
        requests_root(workspace).join(parent_relpath)
    };
    let dir = unique_path(&parent, &slugify(name), "");
    std::fs::create_dir_all(&dir)
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;

    let id = new_id("folder");
    write_json(
        &dir.join(FOLDER_FILE),
        &Folder {
            format: "wireforge.folder".to_string(),
            version: 1,
            id: id.clone(),
            name: name.to_string(),
            order: vec![],
        },
    )?;

    let mut order = read_children_order(workspace, parent_relpath)?;
    order.push(id);
    write_children_order(workspace, parent_relpath, order)?;

    Ok(rel(&requests_root(workspace), &dir))
}

/// Rename a request or folder by updating its `name` field (the file/dir name is
/// kept stable so references and Git history are preserved).
pub fn rename(workspace: &Path, relpath: &str, new_name: &str) -> WfResult<()> {
    let full = requests_root(workspace).join(relpath);
    if full.is_dir() {
        let ff = full.join(FOLDER_FILE);
        let mut f: Folder = read_json(&ff)?;
        f.name = new_name.to_string();
        write_json(&ff, &f)
    } else {
        let mut req: RequestFile = read_json(&full)?;
        req.name = new_name.to_string();
        write_json(&full, &req)
    }
}

/// Delete a request or folder and remove it from its parent's order.
pub fn delete(workspace: &Path, relpath: &str) -> WfResult<()> {
    let full = requests_root(workspace).join(relpath);
    let id = if full.is_dir() {
        let f: Folder = read_json(&full.join(FOLDER_FILE))?;
        std::fs::remove_dir_all(&full)
            .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
        f.id
    } else {
        let req: RequestFile = read_json(&full)?;
        std::fs::remove_file(&full)
            .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
        req.id
    };
    let parent = parent_relpath(relpath);
    let order = read_children_order(workspace, &parent)?
        .into_iter()
        .filter(|x| *x != id)
        .collect();
    write_children_order(workspace, &parent, order)
}

pub fn load_request_file(workspace: &Path, relpath: &str) -> WfResult<RequestFile> {
    read_json(&requests_root(workspace).join(relpath))
}

/// Merge variables into the main collection's `variables` map, keeping existing
/// keys. Returns how many were added.
pub fn merge_collection_variables(workspace: &Path, vars: &[(String, String)]) -> WfResult<u32> {
    if vars.is_empty() {
        return Ok(0);
    }
    ensure_collection(workspace)?;
    let col_path = collection_dir(workspace).join("collection.json");
    let mut c: Collection = read_json(&col_path)?;
    let mut added = 0;
    for (k, v) in vars {
        if !c.variables.contains_key(k) {
            c.variables.insert(k.clone(), v.clone());
            added += 1;
        }
    }
    if added > 0 {
        write_json(&col_path, &c)?;
    }
    Ok(added)
}

pub fn save_request_file(workspace: &Path, relpath: &str, request: &RequestFile) -> WfResult<()> {
    write_json(&requests_root(workspace).join(relpath), request)
}

fn node_id(path: &Path) -> WfResult<String> {
    if path.is_dir() {
        let f: Folder = read_json(&path.join(FOLDER_FILE))?;
        Ok(f.id)
    } else {
        let r: RequestFile = read_json(path)?;
        Ok(r.id)
    }
}

fn split_name(file_name: &str) -> (String, String) {
    if let Some(base) = file_name.strip_suffix(REQUEST_EXT) {
        (base.to_string(), REQUEST_EXT.to_string())
    } else {
        (file_name.to_string(), String::new())
    }
}

/// Move a request or folder into `dest_folder_relpath` (empty = root), updating
/// both parents' order arrays. Returns the new relative path.
pub fn move_node(
    workspace: &Path,
    src_relpath: &str,
    dest_folder_relpath: &str,
) -> WfResult<String> {
    let root = requests_root(workspace);
    let src = root.join(src_relpath);
    if !src.exists() {
        return Err(Box::new(WfError::new(
            "WF_STORE_FILE_NOT_FOUND",
            "source not found",
        )));
    }
    let dest_dir = if dest_folder_relpath.is_empty() {
        root.clone()
    } else {
        root.join(dest_folder_relpath)
    };

    if src.is_dir() {
        let src_c = src.canonicalize().unwrap_or_else(|_| src.clone());
        let dest_c = dest_dir.canonicalize().unwrap_or_else(|_| dest_dir.clone());
        if dest_c.starts_with(&src_c) {
            return Err(Box::new(WfError::new(
                "WF_STORE_WRITE_FAILED",
                "cannot move a folder into itself",
            )));
        }
    }

    let id = node_id(&src)?;
    let src_parent = parent_relpath(src_relpath);

    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
    let file_name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| WfError::new("WF_STORE_WRITE_FAILED", "invalid source name"))?;
    let mut new_path = dest_dir.join(&file_name);
    if new_path.exists() && new_path != src {
        let (base, ext) = split_name(&file_name);
        new_path = unique_path(&dest_dir, &base, &ext);
    }
    std::fs::rename(&src, &new_path)
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;

    if src_parent != dest_folder_relpath {
        let src_order: Vec<String> = read_children_order(workspace, &src_parent)?
            .into_iter()
            .filter(|x| *x != id)
            .collect();
        write_children_order(workspace, &src_parent, src_order)?;
        let mut dest_order = read_children_order(workspace, dest_folder_relpath)?;
        if !dest_order.contains(&id) {
            dest_order.push(id);
        }
        write_children_order(workspace, dest_folder_relpath, dest_order)?;
    }
    Ok(rel(&root, &new_path))
}

/// Duplicate a request as a sibling with a new id and a " copy" name.
pub fn duplicate_request(workspace: &Path, relpath: &str) -> WfResult<String> {
    let root = requests_root(workspace);
    let src = root.join(relpath);
    let mut req: RequestFile = read_json(&src)?;
    let dir = src
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| WfError::new("WF_STORE_WRITE_FAILED", "no parent directory"))?;

    req.id = new_id("req");
    req.name = format!("{} copy", req.name);
    let path = unique_path(&dir, &slugify(&req.name), REQUEST_EXT);
    write_json(&path, &req)?;

    let parent = parent_relpath(relpath);
    let mut order = read_children_order(workspace, &parent)?;
    order.push(req.id.clone());
    write_children_order(workspace, &parent, order)?;

    Ok(rel(&root, &path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_ws() -> PathBuf {
        std::env::temp_dir().join(new_id("wf_ws"))
    }

    #[test]
    fn empty_workspace_loads_empty_tree() {
        let ws = temp_ws();
        assert!(load_tree(&ws).unwrap().is_empty());
    }

    #[test]
    fn create_and_load_tree_with_order() {
        let ws = temp_ws();
        let folder = create_folder(&ws, "", "Users").unwrap();
        create_request(&ws, &folder, "List users").unwrap();
        create_request(&ws, "", "Ping").unwrap();

        let tree = load_tree(&ws).unwrap();
        assert_eq!(tree.len(), 2); // Users folder, then Ping (creation order)
        match &tree[0] {
            Node::Folder { name, children, .. } => {
                assert_eq!(name, "Users");
                assert_eq!(children.len(), 1);
                match &children[0] {
                    Node::Request { name, .. } => assert_eq!(name, "List users"),
                    _ => panic!("expected request child"),
                }
            }
            _ => panic!("expected folder first"),
        }
        match &tree[1] {
            Node::Request { name, .. } => assert_eq!(name, "Ping"),
            _ => panic!("expected request second"),
        }

        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn rename_and_delete() {
        let ws = temp_ws();
        let path = create_request(&ws, "", "Old name").unwrap();
        rename(&ws, &path, "New name").unwrap();
        assert_eq!(load_request_file(&ws, &path).unwrap().name, "New name");

        delete(&ws, &path).unwrap();
        assert!(load_tree(&ws).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn move_request_between_folders() {
        let ws = temp_ws();
        let a = create_folder(&ws, "", "A").unwrap();
        let b = create_folder(&ws, "", "B").unwrap();
        let req = create_request(&ws, &a, "Endpoint").unwrap();

        let moved = move_node(&ws, &req, &b).unwrap();
        assert!(moved.starts_with(&format!("{b}/")));
        assert_eq!(load_request_file(&ws, &moved).unwrap().name, "Endpoint");

        let tree = load_tree(&ws).unwrap();
        for n in &tree {
            if let Node::Folder { name, children, .. } = n {
                match name.as_str() {
                    "A" => assert_eq!(children.len(), 0, "source folder should be empty"),
                    "B" => assert_eq!(children.len(), 1, "dest folder should hold the request"),
                    _ => {}
                }
            }
        }

        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn duplicate_makes_a_copy() {
        let ws = temp_ws();
        let req = create_request(&ws, "", "Ping").unwrap();

        let dup = duplicate_request(&ws, &req).unwrap();
        assert_ne!(dup, req);
        assert_eq!(load_request_file(&ws, &dup).unwrap().name, "Ping copy");
        assert_ne!(
            load_request_file(&ws, &req).unwrap().id,
            load_request_file(&ws, &dup).unwrap().id
        );
        assert_eq!(load_tree(&ws).unwrap().len(), 2);

        let _ = std::fs::remove_dir_all(&ws);
    }
}
