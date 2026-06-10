//! VcsProvider: repository and collaboration operations.
//! Target: `git2` plus a GitHub API client (v0.6+). Kept behind a trait so a
//! Forgejo/Tangled provider can be added later.
//!
//! v0.5 implements local read-only status (branch, ahead/behind, per-file
//! state). Diff and commit land in later v0.5 chunks; push/pull in v0.6.

use crate::error::{WfError, WfResult};
use git2::{
    build::CheckoutBuilder, Cred, DiffFormat, DiffOptions, FetchOptions, PushOptions,
    RemoteCallbacks, Repository, Status, StatusOptions,
};
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

fn not_a_repo() -> Box<WfError> {
    Box::new(WfError::new(
        "WF_GIT_NOT_A_REPO",
        "the workspace is not inside a Git repository",
    ))
}

/// Map a workspace-relative path to a repository-relative one.
fn repo_path(prefix: &str, ws_relative: &str) -> String {
    if prefix.is_empty() {
        ws_relative.to_string()
    } else {
        format!("{prefix}/{ws_relative}")
    }
}

/// Unified diff of the working tree (index + workdir) against HEAD. `path` is a
/// workspace-relative path to scope the diff; `None` diffs everything. Untracked
/// files appear as fully added. Returns empty when not in a repository.
pub fn diff(workspace: &Path, path: Option<&str>) -> WfResult<String> {
    let repo = match Repository::discover(workspace) {
        Ok(r) => r,
        Err(_) => return Ok(String::new()),
    };
    let prefix = workspace_prefix(&repo, workspace);

    let mut opts = DiffOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true);
    if let Some(p) = path {
        opts.pathspec(repo_path(&prefix, p));
    }

    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let diff = repo
        .diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))
        .map_err(git_err)?;

    let mut out = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        if matches!(line.origin(), '+' | '-' | ' ') {
            out.push(line.origin());
        }
        out.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
        true
    })
    .map_err(git_err)?;
    Ok(out)
}

/// Stage the given workspace-relative paths (added/modified are staged, missing
/// ones are removed) and create a commit. A missing Git identity is reported so
/// the user can configure it.
pub fn commit(workspace: &Path, message: &str, paths: &[String]) -> WfResult<()> {
    let repo = Repository::discover(workspace).map_err(|_| not_a_repo())?;
    let prefix = workspace_prefix(&repo, workspace);
    let workdir = repo.workdir().ok_or_else(not_a_repo)?.to_path_buf();

    let mut index = repo.index().map_err(git_err)?;
    for p in paths {
        let rel = repo_path(&prefix, p);
        if workdir.join(&rel).exists() {
            index.add_path(Path::new(&rel)).map_err(git_err)?;
        } else {
            index.remove_path(Path::new(&rel)).map_err(git_err)?;
        }
    }
    index.write().map_err(git_err)?;
    let tree = repo
        .find_tree(index.write_tree().map_err(git_err)?)
        .map_err(git_err)?;

    let sig = repo.signature().map_err(|_| {
        Box::new(WfError::new(
            "WF_GIT_OPERATION_FAILED",
            "configure git user.name and user.email to commit",
        ))
    })?;

    // Parents: HEAD, plus MERGE_HEAD when finishing a merge (so the resolution
    // becomes a proper merge commit).
    let mut parents: Vec<git2::Commit> = vec![];
    if let Some(c) = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok())
    {
        parents.push(c);
    }
    if let Ok(merge_head) = repo.find_reference("MERGE_HEAD") {
        if let Some(c) = merge_head
            .target()
            .and_then(|oid| repo.find_commit(oid).ok())
        {
            parents.push(c);
        }
    }
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .map_err(git_err)?;
    // Clear any in-progress merge state now that it is committed.
    let _ = repo.cleanup_state();
    Ok(())
}

/// Extract the host from an HTTPS remote URL (for token lookup). Errors when the
/// remote is not HTTPS, since token-over-HTTPS is the supported transport.
fn https_host(url: &str) -> WfResult<String> {
    let rest = url.strip_prefix("https://").ok_or_else(|| {
        Box::new(WfError::new(
            "WF_GITHUB_REQUEST_FAILED",
            "the origin remote must use an HTTPS URL for token authentication",
        ))
    })?;
    let authority = rest.split('/').next().unwrap_or("");
    // Drop any userinfo (user@host).
    let host = authority.rsplit('@').next().unwrap_or(authority);
    Ok(host.to_string())
}

/// Build remote callbacks that supply the GitHub token for HTTPS remotes. Local
/// (file path) remotes need no credentials, so the token is optional.
fn token_for(url: &str) -> WfResult<Option<String>> {
    if url.starts_with("https://") {
        crate::github::load_token(&https_host(url)?)
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullOutcome {
    /// "upToDate" | "fastForward" | "merged"
    pub status: String,
}

/// Fetch `origin` and integrate the current branch's upstream: fast-forward when
/// possible, otherwise a merge commit. Conflicts leave the merge in progress and
/// return `WF_GIT_MERGE_CONFLICT` for the resolution UI.
pub fn pull(workspace: &Path) -> WfResult<PullOutcome> {
    let repo = Repository::discover(workspace).map_err(|_| not_a_repo())?;
    let head = repo.head().map_err(git_err)?;
    let branch = head
        .shorthand()
        .ok_or_else(|| git_err(git2::Error::from_str("no current branch")))?
        .to_string();
    let mut remote = repo.find_remote("origin").map_err(|_| {
        Box::new(WfError::new(
            "WF_GITHUB_REQUEST_FAILED",
            "no 'origin' remote is configured",
        ))
    })?;
    let url = remote.url().unwrap_or("").to_string();
    let token = token_for(&url)?;

    {
        let mut cb = RemoteCallbacks::new();
        if let Some(tok) = &token {
            cb.credentials(move |_url, _user, _allowed| {
                Cred::userpass_plaintext("x-access-token", tok)
            });
        }
        let mut fo = FetchOptions::new();
        fo.remote_callbacks(cb);
        remote
            .fetch(&[&branch], Some(&mut fo), None)
            .map_err(git_err)?;
    }

    let fetch_head = repo.find_reference("FETCH_HEAD").map_err(git_err)?;
    let their = repo
        .reference_to_annotated_commit(&fetch_head)
        .map_err(git_err)?;
    let (analysis, _) = repo.merge_analysis(&[&their]).map_err(git_err)?;

    if analysis.is_up_to_date() {
        return Ok(PullOutcome {
            status: "upToDate".to_string(),
        });
    }

    if analysis.is_fast_forward() {
        let refname = format!("refs/heads/{branch}");
        match repo.find_reference(&refname) {
            Ok(mut r) => {
                r.set_target(their.id(), "pull: fast-forward")
                    .map_err(git_err)?;
            }
            Err(_) => {
                repo.reference(&refname, their.id(), true, "pull: create")
                    .map_err(git_err)?;
            }
        }
        repo.set_head(&refname).map_err(git_err)?;
        repo.checkout_head(Some(CheckoutBuilder::new().force()))
            .map_err(git_err)?;
        return Ok(PullOutcome {
            status: "fastForward".to_string(),
        });
    }

    // Normal merge.
    repo.merge(&[&their], None, None).map_err(git_err)?;
    let mut index = repo.index().map_err(git_err)?;
    if index.has_conflicts() {
        return Err(Box::new(
            WfError::new("WF_GIT_MERGE_CONFLICT", "merge produced conflicts")
                .with_suggested_action("resolveConflict"),
        ));
    }
    let tree = repo
        .find_tree(index.write_tree().map_err(git_err)?)
        .map_err(git_err)?;
    let sig = repo.signature().map_err(|_| {
        Box::new(WfError::new(
            "WF_GIT_OPERATION_FAILED",
            "configure git user.name and user.email to merge",
        ))
    })?;
    let head_commit = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|o| repo.find_commit(o).ok())
        .ok_or_else(|| git_err(git2::Error::from_str("no HEAD commit")))?;
    let their_commit = repo.find_commit(their.id()).map_err(git_err)?;
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Merge remote-tracking branch 'origin'",
        &tree,
        &[&head_commit, &their_commit],
    )
    .map_err(git_err)?;
    repo.cleanup_state().map_err(git_err)?;
    Ok(PullOutcome {
        status: "merged".to_string(),
    })
}

/// Push the current branch to `origin`. A non-fast-forward rejection is reported
/// as `WF_GITHUB_PUSH_REJECTED` so the user pulls first.
pub fn push(workspace: &Path) -> WfResult<()> {
    let repo = Repository::discover(workspace).map_err(|_| not_a_repo())?;
    let head = repo.head().map_err(git_err)?;
    let branch = head
        .shorthand()
        .ok_or_else(|| git_err(git2::Error::from_str("no current branch")))?
        .to_string();
    let mut remote = repo.find_remote("origin").map_err(|_| {
        Box::new(WfError::new(
            "WF_GITHUB_REQUEST_FAILED",
            "no 'origin' remote is configured",
        ))
    })?;
    let url = remote.url().unwrap_or("").to_string();
    let token = token_for(&url)?;

    let rejected = std::cell::RefCell::new(None::<String>);
    {
        let mut cb = RemoteCallbacks::new();
        if let Some(tok) = &token {
            cb.credentials(move |_url, _user, _allowed| {
                Cred::userpass_plaintext("x-access-token", tok)
            });
        }
        cb.push_update_reference(|_refname, status| {
            if let Some(msg) = status {
                *rejected.borrow_mut() = Some(msg.to_string());
            }
            Ok(())
        });
        let mut po = PushOptions::new();
        po.remote_callbacks(cb);
        let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
        remote.push(&[&refspec], Some(&mut po)).map_err(git_err)?;
    }

    if let Some(msg) = rejected.borrow().clone() {
        return Err(Box::new(
            WfError::new("WF_GITHUB_PUSH_REJECTED", format!("push rejected: {msg}"))
                .with_suggested_action("pullThenRetry"),
        ));
    }
    Ok(())
}

/// The three sides of a conflicted file, read from the index conflict stages.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictSides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ours: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theirs: Option<String>,
}

fn blob_text(repo: &Repository, oid: git2::Oid) -> Option<String> {
    repo.find_blob(oid)
        .ok()
        .map(|b| String::from_utf8_lossy(b.content()).to_string())
}

fn entry_path_matches(entry: &Option<git2::IndexEntry>, rel: &str) -> bool {
    entry
        .as_ref()
        .and_then(|e| std::str::from_utf8(&e.path).ok())
        .map(|p| p == rel)
        .unwrap_or(false)
}

/// Read the base/ours/theirs contents of a conflicted file for side-by-side
/// display.
pub fn conflict_sides(workspace: &Path, path: &str) -> WfResult<ConflictSides> {
    let repo = Repository::discover(workspace).map_err(|_| not_a_repo())?;
    let prefix = workspace_prefix(&repo, workspace);
    let rel = repo_path(&prefix, path);
    let index = repo.index().map_err(git_err)?;

    let mut sides = ConflictSides::default();
    for c in index.conflicts().map_err(git_err)? {
        let c = c.map_err(git_err)?;
        if entry_path_matches(&c.ancestor, &rel)
            || entry_path_matches(&c.our, &rel)
            || entry_path_matches(&c.their, &rel)
        {
            sides.base = c.ancestor.and_then(|e| blob_text(&repo, e.id));
            sides.ours = c.our.and_then(|e| blob_text(&repo, e.id));
            sides.theirs = c.their.and_then(|e| blob_text(&repo, e.id));
            break;
        }
    }
    Ok(sides)
}

fn conflict_oids(
    index: &git2::Index,
    rel: &str,
) -> WfResult<(Option<git2::Oid>, Option<git2::Oid>)> {
    for c in index.conflicts().map_err(git_err)? {
        let c = c.map_err(git_err)?;
        if entry_path_matches(&c.ancestor, rel)
            || entry_path_matches(&c.our, rel)
            || entry_path_matches(&c.their, rel)
        {
            return Ok((c.our.map(|e| e.id), c.their.map(|e| e.id)));
        }
    }
    Ok((None, None))
}

/// Resolve a conflicted file by keeping one side ("mine" = ours, "theirs"),
/// writing the resolved content to the file, preserving the losing side as a
/// visible `<path>.<side>.conflict` backup, and staging the resolution.
pub fn resolve_conflict(workspace: &Path, path: &str, choice: &str) -> WfResult<()> {
    let repo = Repository::discover(workspace).map_err(|_| not_a_repo())?;
    let prefix = workspace_prefix(&repo, workspace);
    let rel = repo_path(&prefix, path);
    let workdir = repo.workdir().ok_or_else(not_a_repo)?.to_path_buf();

    let mut index = repo.index().map_err(git_err)?;
    let (ours, theirs) = conflict_oids(&index, &rel)?;

    let (chosen, losing, losing_label) = if choice == "theirs" {
        (theirs, ours, "ours")
    } else {
        (ours, theirs, "theirs")
    };

    let target = workdir.join(&rel);
    match chosen {
        Some(oid) => {
            let blob = repo.find_blob(oid).map_err(git_err)?;
            std::fs::write(&target, blob.content())
                .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
        }
        None => {
            // The chosen side deleted the file.
            let _ = std::fs::remove_file(&target);
        }
    }

    if let Some(oid) = losing {
        if let Ok(blob) = repo.find_blob(oid) {
            let backup = workdir.join(format!("{rel}.{losing_label}.conflict"));
            std::fs::write(&backup, blob.content())
                .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
        }
    }

    // Adding (or removing) the path replaces the conflict stages with a single
    // stage-0 entry, marking the conflict resolved.
    if target.exists() {
        index.add_path(Path::new(&rel)).map_err(git_err)?;
    } else {
        let _ = index.remove_path(Path::new(&rel));
    }
    index.write().map_err(git_err)?;
    Ok(())
}

/// Mark a conflict resolved using the current working-tree content (for files
/// edited by hand or in an external editor).
pub fn mark_resolved(workspace: &Path, path: &str) -> WfResult<()> {
    let repo = Repository::discover(workspace).map_err(|_| not_a_repo())?;
    let prefix = workspace_prefix(&repo, workspace);
    let rel = repo_path(&prefix, path);
    let workdir = repo.workdir().ok_or_else(not_a_repo)?.to_path_buf();

    let mut index = repo.index().map_err(git_err)?;
    if workdir.join(&rel).exists() {
        index.add_path(Path::new(&rel)).map_err(git_err)?;
    } else {
        let _ = index.remove_path(Path::new(&rel));
    }
    index.write().map_err(git_err)?;
    Ok(())
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

    fn init_repo_with_commit(dir: &Path) -> Repository {
        std::fs::create_dir_all(dir).unwrap();
        let repo = Repository::init(dir).unwrap();
        {
            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "t").unwrap();
            cfg.set_str("user.email", "t@t").unwrap();
        }
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("a.txt")).unwrap();
            index.write().unwrap();
            let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
            let sig = Signature::now("t", "t@t").unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }
        repo
    }

    #[test]
    fn diff_shows_modifications() {
        let dir = temp_dir();
        let _repo = init_repo_with_commit(&dir);
        std::fs::write(dir.join("a.txt"), "two\n").unwrap();

        let d = diff(&dir, None).unwrap();
        assert!(d.contains("a.txt"), "diff names the file: {d}");
        assert!(d.contains("-one"), "diff has the removed line: {d}");
        assert!(d.contains("+two"), "diff has the added line: {d}");

        // Scoping to an unrelated path yields nothing.
        assert!(diff(&dir, Some("nope.txt")).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_stages_and_writes_history() {
        let dir = temp_dir();
        let repo = init_repo_with_commit(&dir);

        std::fs::write(dir.join("b.txt"), "hi\n").unwrap();
        commit(&dir, "add b", &["b.txt".to_string()]).unwrap();

        let st = repo_status(&dir).unwrap();
        assert!(
            !st.files.iter().any(|f| f.path == "b.txt"),
            "b.txt should be committed: {:?}",
            st.files
        );

        let msg = repo
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .message()
            .unwrap()
            .to_string();
        assert_eq!(msg, "add b");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A portable `file://` URL for a local directory (works on Windows and Unix).
    fn file_url(p: &Path) -> String {
        let s = p
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let s = s.trim_start_matches("//?/");
        if s.starts_with('/') {
            format!("file://{s}")
        } else {
            format!("file:///{s}")
        }
    }

    #[test]
    fn https_host_parsing() {
        assert_eq!(
            https_host("https://github.com/org/repo.git").unwrap(),
            "github.com"
        );
        assert_eq!(
            https_host("https://x-access-token@ghe.corp/org/r.git").unwrap(),
            "ghe.corp"
        );
        assert!(https_host("git@github.com:org/repo.git").is_err());
    }

    #[test]
    fn push_updates_a_bare_remote() {
        let work = temp_dir();
        let repo = init_repo_with_commit(&work);
        let origin_dir = temp_dir();
        let origin = Repository::init_bare(&origin_dir).unwrap();
        repo.remote("origin", &file_url(&origin_dir)).unwrap();

        push(&work).unwrap();

        let local_head = repo.head().unwrap().target().unwrap();
        let remote_head = origin
            .find_reference("refs/heads/master")
            .unwrap()
            .target()
            .unwrap();
        assert_eq!(local_head, remote_head);

        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&origin_dir);
    }

    #[test]
    fn pull_fast_forwards() {
        let origin_dir = temp_dir();
        let origin = init_repo_with_commit(&origin_dir); // commit A, a.txt="one\n"

        // Local repo tracking origin at A.
        let local_dir = temp_dir();
        std::fs::create_dir_all(&local_dir).unwrap();
        let local = Repository::init(&local_dir).unwrap();
        local.remote("origin", &file_url(&origin_dir)).unwrap();
        local
            .find_remote("origin")
            .unwrap()
            .fetch(&["master"], None, None)
            .unwrap();
        let a = local
            .find_commit(
                local
                    .find_reference("FETCH_HEAD")
                    .unwrap()
                    .target()
                    .unwrap(),
            )
            .unwrap();
        local.branch("master", &a, true).unwrap();
        local.set_head("refs/heads/master").unwrap();
        local
            .checkout_head(Some(CheckoutBuilder::new().force()))
            .unwrap();
        // Normalize line endings: Git autocrlf may rewrite LF on checkout (Windows).
        let read =
            |p: std::path::PathBuf| std::fs::read_to_string(p).unwrap().replace("\r\n", "\n");
        assert_eq!(read(local_dir.join("a.txt")), "one\n");

        // origin advances to B.
        std::fs::write(origin_dir.join("a.txt"), "two\n").unwrap();
        {
            let mut idx = origin.index().unwrap();
            idx.add_path(Path::new("a.txt")).unwrap();
            idx.write().unwrap();
            let tree = origin.find_tree(idx.write_tree().unwrap()).unwrap();
            let sig = Signature::now("t", "t@t").unwrap();
            let parent = origin.head().unwrap().peel_to_commit().unwrap();
            origin
                .commit(Some("HEAD"), &sig, &sig, "B", &tree, &[&parent])
                .unwrap();
        }

        let out = pull(&local_dir).unwrap();
        assert_eq!(out.status, "fastForward");
        assert_eq!(read(local_dir.join("a.txt")), "two\n");

        let _ = std::fs::remove_dir_all(&origin_dir);
        let _ = std::fs::remove_dir_all(&local_dir);
    }

    #[test]
    fn resolve_conflict_keeps_a_side_and_backs_up_the_other() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Repository::init(&dir).unwrap();
        {
            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "t").unwrap();
            cfg.set_str("user.email", "t@t").unwrap();
        }
        let sig = || Signature::now("t", "t@t").unwrap();
        let commit_file = |contents: &str, msg: &str, parents: &[&git2::Commit]| {
            std::fs::write(dir.join("f.txt"), contents).unwrap();
            let mut idx = repo.index().unwrap();
            idx.add_path(Path::new("f.txt")).unwrap();
            idx.write().unwrap();
            let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
            repo.commit(Some("HEAD"), &sig(), &sig(), msg, &tree, parents)
                .unwrap()
        };

        let a = repo.find_commit(commit_file("base\n", "A", &[])).unwrap();
        commit_file("ours\n", "ours", &[&a]); // master diverges

        repo.branch("theirs", &a, true).unwrap();
        repo.set_head("refs/heads/theirs").unwrap();
        repo.checkout_head(Some(CheckoutBuilder::new().force()))
            .unwrap();
        let t = commit_file("theirs\n", "theirs", &[&a]);

        repo.set_head("refs/heads/master").unwrap();
        repo.checkout_head(Some(CheckoutBuilder::new().force()))
            .unwrap();
        let their_ann = repo.find_annotated_commit(t).unwrap();
        repo.merge(&[&their_ann], None, None).unwrap();
        assert!(repo.index().unwrap().has_conflicts());

        let sides = conflict_sides(&dir, "f.txt").unwrap();
        assert_eq!(sides.ours.as_deref().map(str::trim), Some("ours"));
        assert_eq!(sides.theirs.as_deref().map(str::trim), Some("theirs"));

        resolve_conflict(&dir, "f.txt", "mine").unwrap();
        // resolve_conflict wrote the index on disk via its own handle; reload.
        let mut reloaded = repo.index().unwrap();
        reloaded.read(true).unwrap();
        assert!(!reloaded.has_conflicts());
        let read =
            |p: std::path::PathBuf| std::fs::read_to_string(p).unwrap().replace("\r\n", "\n");
        assert_eq!(read(dir.join("f.txt")), "ours\n");
        assert_eq!(read(dir.join("f.txt.theirs.conflict")), "theirs\n");

        commit(&dir, "merge theirs", &["f.txt".to_string()]).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_count(), 2, "merge commit has two parents");
        assert_eq!(repo.state(), git2::RepositoryState::Clean);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
