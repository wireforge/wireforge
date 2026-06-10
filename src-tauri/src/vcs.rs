//! VcsProvider: repository and collaboration operations.
//! Target: `git2` plus a GitHub API client (v0.6+). Kept behind a trait so a
//! Forgejo/Tangled provider can be added later.
//!
//! v0.5 implements local read-only status (branch, ahead/behind, per-file
//! state). Diff and commit land in later v0.5 chunks; push/pull in v0.6.

use crate::error::{WfError, WfResult};
use git2::{Repository, Status, StatusOptions};
use serde::Serialize;
use std::path::Path;

pub trait VcsProvider {
    fn current_branch(&self) -> WfResult<String>;
    fn status(&self) -> WfResult<()>;
    fn commit(&self, message: &str) -> WfResult<()>;
    fn push(&self) -> WfResult<()>;
    fn pull(&self) -> WfResult<()>;
}

/// The working-tree state of a single path, collapsed to what the sidebar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Clean,
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
    Conflicted,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// Path relative to the workspace root, forward slashes.
    pub path: String,
    pub status: NodeStatus,
}

/// A repository status snapshot for one workspace. `isRepo` is false (not an
/// error) when the workspace is not inside a Git repository.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub is_repo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub dirty: bool,
    pub files: Vec<FileEntry>,
}

fn git_err(e: git2::Error) -> Box<WfError> {
    Box::new(WfError::new(
        "WF_GIT_OPERATION_FAILED",
        format!("git error: {e}"),
    ))
}

fn map_status(s: Status) -> NodeStatus {
    if s.is_conflicted() {
        NodeStatus::Conflicted
    } else if s.is_index_deleted() || s.is_wt_deleted() {
        NodeStatus::Deleted
    } else if s.is_index_renamed() || s.is_wt_renamed() {
        NodeStatus::Renamed
    } else if s.is_index_new() {
        NodeStatus::Added
    } else if s.is_wt_new() {
        NodeStatus::Untracked
    } else if s.is_index_modified()
        || s.is_wt_modified()
        || s.is_index_typechange()
        || s.is_wt_typechange()
    {
        NodeStatus::Modified
    } else {
        NodeStatus::Clean
    }
}

/// The workspace path expressed relative to the repository work directory, with
/// forward slashes (empty when they are the same directory).
fn workspace_prefix(repo: &Repository, workspace: &Path) -> String {
    let Some(workdir) = repo.workdir() else {
        return String::new();
    };
    let work_c = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());
    let ws_c = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    match ws_c.strip_prefix(&work_c) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => String::new(),
    }
}

/// Re-root a repo-relative path under the workspace, returning None when the
/// path is outside the workspace subtree.
fn under_workspace(path: &str, prefix: &str) -> Option<String> {
    if prefix.is_empty() {
        return Some(path.to_string());
    }
    if path == prefix {
        return Some(String::new());
    }
    path.strip_prefix(&format!("{prefix}/")).map(str::to_string)
}

/// Compute ahead/behind counts of HEAD against its upstream, if any.
fn ahead_behind(repo: &Repository) -> (usize, usize) {
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return (0, 0),
    };
    let Some(local_oid) = head.target() else {
        return (0, 0);
    };
    let Some(name) = head.shorthand() else {
        return (0, 0);
    };
    let Ok(branch) = repo.find_branch(name, git2::BranchType::Local) else {
        return (0, 0);
    };
    let Ok(upstream) = branch.upstream() else {
        return (0, 0);
    };
    let Some(up_oid) = upstream.get().target() else {
        return (0, 0);
    };
    repo.graph_ahead_behind(local_oid, up_oid).unwrap_or((0, 0))
}

/// Read the workspace's Git status. Returns `is_repo: false` when the workspace
/// is not inside a repository.
pub fn repo_status(workspace: &Path) -> WfResult<RepoStatus> {
    let repo = match Repository::discover(workspace) {
        Ok(r) => r,
        Err(_) => return Ok(RepoStatus::default()),
    };

    let branch = match repo.head() {
        Ok(h) => h.shorthand().map(str::to_string),
        Err(_) => None, // unborn branch (no commits yet)
    };
    let (ahead, behind) = ahead_behind(&repo);
    let prefix = workspace_prefix(&repo, workspace);

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = repo.statuses(Some(&mut opts)).map_err(git_err)?;

    let mut files = vec![];
    let mut dirty = false;
    for entry in statuses.iter() {
        let status = entry.status();
        if status.is_ignored() {
            continue;
        }
        dirty = true;
        let Some(path) = entry.path() else { continue };
        if let Some(rel) = under_workspace(path, &prefix) {
            files.push(FileEntry {
                path: rel,
                status: map_status(status),
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(RepoStatus {
        is_repo: true,
        branch,
        ahead,
        behind,
        dirty,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;

    fn temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(crate::id::new_id("wf_git"))
    }

    #[test]
    fn non_repo_reports_not_a_repo() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let st = repo_status(&dir).unwrap();
        assert!(!st.is_repo);
        assert!(st.branch.is_none());
        assert!(st.files.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_modified_and_untracked_files() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Repository::init(&dir).unwrap();

        // Commit one file so a baseline tree exists.
        std::fs::write(dir.join("a.txt"), "one").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("a.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("t", "t@t").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();

        // Modify the tracked file and add an untracked one.
        std::fs::write(dir.join("a.txt"), "two").unwrap();
        std::fs::write(dir.join("b.txt"), "new").unwrap();

        let st = repo_status(&dir).unwrap();
        assert!(st.is_repo);
        assert!(st.dirty);
        assert!(st.branch.is_some());
        assert_eq!(st.ahead, 0);
        assert_eq!(st.behind, 0);

        let find = |p: &str| st.files.iter().find(|f| f.path == p).map(|f| f.status);
        assert_eq!(find("a.txt"), Some(NodeStatus::Modified));
        assert_eq!(find("b.txt"), Some(NodeStatus::Untracked));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_subdir_paths_are_rerooted() {
        // Repo at root; workspace is a subdirectory. Status paths should come
        // back relative to the workspace, and files outside it are excluded.
        let root = temp_dir();
        let ws = root.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        let _repo = Repository::init(&root).unwrap();

        std::fs::write(root.join("outside.txt"), "x").unwrap();
        std::fs::create_dir_all(ws.join("collections")).unwrap();
        std::fs::write(ws.join("collections").join("c.txt"), "y").unwrap();

        let st = repo_status(&ws).unwrap();
        assert!(st.is_repo);
        assert!(st.dirty); // repo has changes (incl. outside.txt)
        assert_eq!(st.files.len(), 1, "only workspace files are listed");
        assert_eq!(st.files[0].path, "collections/c.txt");

        let _ = std::fs::remove_dir_all(&root);
    }
}
