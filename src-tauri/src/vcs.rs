//! VcsProvider: repository and collaboration operations.
//! Target: `git2` plus a GitHub API client (v0.5+). Kept behind a trait so a
//! Forgejo/Tangled provider can be added later.

use crate::error::WfResult;

pub trait VcsProvider {
    fn current_branch(&self) -> WfResult<String>;
    fn status(&self) -> WfResult<()>;
    fn commit(&self, message: &str) -> WfResult<()>;
    fn push(&self) -> WfResult<()>;
    fn pull(&self) -> WfResult<()>;
}
